use crate::model::{Entry, FolderCompletion, Settings};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuerySpec {
    pub terms: Vec<String>,
    pub scope: String,
    pub extensions: Vec<String>,
    pub directory: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderPrefix {
    pub directory: String,
    pub prefix: String,
}

pub fn compile_exact_candidates(spec: &QuerySpec, settings: &Settings) -> Option<String> {
    if spec.terms.is_empty() || !matches!(spec.scope.as_str(), "all" | "files") { return None; }
    let name = spec.terms.join(" ");
    let filters = QuerySpec { terms: spec.terms.clone(), scope: "files".into(), extensions: spec.extensions.clone(), directory: spec.directory.clone() };
    // 先按原关键词缩小候选再核对完整名称；避免仅按目录和通配符扫描整个索引。
    // 输入已拒绝通配符和引号，只生成扩展名通配符。
    Some(format!("count:50 {} <nopath:wfn:\"{name}\"|nopath:wfn:\"{name}.*\">", compile_everything(&filters, settings)))
}

pub fn folder_prefix(raw: &str) -> Option<FolderPrefix> {
    let input = raw.trim();
    let input = if input.starts_with('"') && input.ends_with('"') && input.len() > 1 {
        &input[1..input.len() - 1]
    } else {
        if input.rsplit(['\\', '/']).next().is_some_and(|leaf| leaf.chars().any(char::is_whitespace)) { return None; }
        input
    };
    if input.chars().count() > 512 || input.chars().any(|c| "\"!*?<>|()".contains(c) || c.is_control()) { return None; }
    let path = input.replace('/', "\\");
    let split = path.rfind('\\')?;
    let prefix = path[split + 1..].to_string();
    let directory = if path.as_bytes().get(1) == Some(&b':') {
        local_directory(&path[..=split]).ok()?
    } else {
        if path.starts_with('\\') || path.contains(':') { return None; }
        let parent = path[..split].trim_end_matches('\\');
        if parent.is_empty() || parent.split('\\').any(|c| c.is_empty() || c == "." || c == ".." || c.ends_with('.') || c.ends_with(' ')) { return None; }
        format!("\\{parent}")
    };
    if prefix == "." || prefix == ".." || prefix.ends_with('.') || prefix.ends_with(' ') || prefix.contains(':') { return None; }
    Some(FolderPrefix { directory, prefix })
}

pub fn compile_folder_completion(prefix: &FolderPrefix, settings: &Settings) -> String {
    let mut expression = compile_everything(&QuerySpec { terms: vec![], scope: "folders".into(), extensions: vec![], directory: Some(prefix.directory.clone()) }, settings);
    // 绝对目录在索引查询中限制直接子项，避免深层目录耗尽单页候选预算。
    if !prefix.directory.starts_with('\\') { expression.push_str(&format!(" parent:\"{}\"", prefix.directory)); }
    if !prefix.prefix.is_empty() { expression.push_str(&format!(" startwith:\"{}\"", prefix.prefix)); }
    expression
}

pub fn folder_completions(prefix: &FolderPrefix, entries: Vec<Entry>, settings: &Settings) -> Vec<FolderCompletion> {
    let directory = crate::ranking::normalize(&prefix.directory.replace('/', "\\"));
    let term = crate::ranking::normalize(&prefix.prefix);
    let mut candidates: Vec<_> = entries.into_iter().filter_map(|entry| {
        if entry.kind != "folder" { return None; }
        let path = entry.locator.replace('/', "\\").trim_end_matches('\\').to_string();
        // 仅采用索引返回的本地完整目录；直接父目录边界阻止递归和相似名称穿透。
        local_directory(&path).ok()?;
        let (parent, title) = path.rsplit_once('\\')?;
        let parent = if parent.ends_with(':') { format!("{parent}\\") } else { parent.to_string() };
        let normalized_parent = crate::ranking::normalize(&parent);
        let matches_parent = if directory.starts_with('\\') { normalized_parent.ends_with(&directory) } else { normalized_parent == directory };
        if !matches_parent || !crate::ranking::normalize(title).starts_with(&term) { return None; }
        if !settings.include_directories.is_empty() && !settings.include_directories.iter().any(|d| directory_matches(&path, d)) { return None; }
        if settings.exclude_directories.iter().any(|d| path.eq_ignore_ascii_case(&d.replace('/', "\\").trim_end_matches('\\')) || directory_matches(&path, d)) { return None; }
        Some(FolderCompletion { title: title.into(), query: format!("{path}\\"), path })
    }).collect();
    candidates.sort_by(|a, b| crate::ranking::normalize(&a.path).cmp(&crate::ranking::normalize(&b.path)).then_with(|| a.path.cmp(&b.path)));
    candidates.dedup_by(|a, b| a.path.eq_ignore_ascii_case(&b.path));
    candidates.truncate(5);
    candidates
}

pub fn parse(raw: &str, scope: &str) -> Result<QuerySpec, String> {
    if !["all", "apps", "files", "folders"].contains(&scope) || raw.chars().count() > 512 {
        return Err("Invalid search query".into());
    }
    let (raw, direct_directory) = direct_folder_query(raw)?;
    let direct = direct_directory.is_some();
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut quoted = false;
    for c in raw.chars() {
        if c == '"' {
            quoted = !quoted;
        } else if c.is_control() && !c.is_whitespace() {
            return Err("Invalid search query".into());
        } else if c.is_whitespace() && !quoted {
            if !token.is_empty() {
                tokens.push(std::mem::take(&mut token));
            }
        } else {
            token.push(c);
        }
    }
    if quoted {
        return Err("Invalid search query".into());
    }
    if !token.is_empty() {
        tokens.push(token);
    }
    let mut spec = QuerySpec {
        terms: vec![],
        scope: scope.into(),
        extensions: vec![],
        directory: direct_directory,
    };
    let mut prefix_seen = false;
    for token in tokens {
        if let Some((prefix, value)) = token.split_once(':') {
            match prefix.to_ascii_lowercase().as_str() {
                "app" | "file" | "folder" => {
                    let selected = match prefix.to_ascii_lowercase().as_str() {
                        "app" => "apps",
                        "file" => "files",
                        _ => "folders",
                    };
                    if prefix_seen || (scope != "all" && scope != selected) {
                        return Err("Invalid search filters".into());
                    }
                    spec.scope = selected.into();
                    prefix_seen = true;
                    if !value.is_empty() {
                        push_term(&mut spec, value)?;
                    }
                }
                "ext" => {
                    if value.is_empty() || !spec.extensions.is_empty() {
                        return Err("Invalid search filters".into());
                    }
                    for ext in value.split(';') {
                        let ext = ext.trim_start_matches('.').to_ascii_lowercase();
                        if ext.is_empty()
                            || ext.len() > 32
                            || !ext
                                .chars()
                                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                        {
                            return Err("Invalid search filters".into());
                        }
                        if !spec.extensions.contains(&ext) {
                            spec.extensions.push(ext);
                        }
                    }
                    if spec.extensions.len() > 8 {
                        return Err("Invalid search filters".into());
                    }
                }
                "in" => {
                    if spec.directory.is_some() {
                        return Err("Invalid search filters".into());
                    }
                    spec.directory = Some(local_directory(value)?);
                }
                _ => return Err("Invalid search query".into()),
            }
        } else {
            push_term(&mut spec, &token)?;
        }
    }
    if spec.scope == "apps" && (!spec.extensions.is_empty() || spec.directory.is_some()) {
        return Err("Invalid search filters".into());
    }
    if direct && spec.scope == "all" { spec.scope = "files".into(); }
    Ok(spec)
}

fn direct_folder_query(raw: &str) -> Result<(&str, Option<String>), String> {
    let input = raw.trim();
    let drive_path = |path: &str| {
        let bytes = path.as_bytes();
        bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && matches!(bytes[2], b'\\' | b'/')
    };
    if let Some(quoted) = input.strip_prefix('"') {
        if let Some(end) = quoted.find('"') {
            let path = &quoted[..end];
            if drive_path(path) {
                let rest = &quoted[end + 1..];
                if !rest.is_empty() && !rest.starts_with(char::is_whitespace) { return Err("Invalid search query".into()); }
                if rest.trim().is_empty() && !is_local_directory(path) {
                    if let Some(query) = joined_folder_query(path)? { return Ok(query); }
                }
                return Ok((rest.trim_start(), Some(local_directory(path)?)));
            }
        }
        return Ok((raw, None));
    }
    if !drive_path(input) {
        if let Some(query) = joined_folder_query(input)? { return Ok(query); }
        return Ok((raw, None));
    }
    let first = input.char_indices().find(|(_, c)| c.is_whitespace()).map(|(index, _)| index).unwrap_or(input.len());
    let fallback = local_directory(&input[..first])?;
    // 含空格的粘贴路径按最长真实目录识别；只检查本地卷，避免触发网络目录等待。
    let root: Vec<u16> = fallback[..3].encode_utf16().chain(Some(0)).collect();
    if matches!(unsafe { GetDriveTypeW(root.as_ptr()) }, 2 | 3) {
        let mut boundaries: Vec<_> = input.char_indices().filter(|(_, c)| c.is_whitespace()).map(|(index, _)| index).take(32).collect();
        boundaries.push(input.len());
        for end in boundaries.into_iter().rev() {
            if let Ok(path) = local_directory(input[..end].trim_end()) {
                if std::path::Path::new(&path).is_dir() {
                    return Ok((input[end..].trim_start(), Some(path)));
                }
            }
        }
    }
    if let Some(query) = joined_folder_query(input)? { return Ok(query); }
    Ok((input[first..].trim_start(), Some(fallback)))
}

fn joined_folder_query(input: &str) -> Result<Option<(&str, Option<String>)>, String> {
    let Some(split) = input.rfind(['\\', '/']) else { return Ok(None); };
    let directory = &input[..=split];
    let filename = &input[split + 1..];
    let absolute = input.as_bytes().get(1) == Some(&b':');
    if absolute {
        // 空格分隔输入仍优先按目录和关键词识别；连接写法允许真实父目录含空格。
        if let Some(first_space) = input.find(char::is_whitespace) {
            if first_space > split || !is_local_directory(directory) { return Ok(None); }
        }
        return Ok(Some((filename, Some(local_directory(directory)?))));
    }
    // 文件夹名也可直接限定索引路径；前后分隔符保证不会匹配相似名称的文件夹。
    if directory.contains(':') { return Ok(None); }
    if input.starts_with(['\\', '/']) { return Err("A local folder query is required".into()); }
    for component in directory.trim_end_matches(['\\', '/']).split(['\\', '/']) {
        if component.is_empty() || component == "." || component == ".." { return Err("Invalid folder query".into()); }
        let mut validated = QuerySpec { terms: vec![], scope: "files".into(), extensions: vec![], directory: None };
        push_term(&mut validated, component)?;
    }
    Ok(Some((filename, Some(format!("\\{}", directory.replace('/', "\\").trim_end_matches('\\'))))))
}

fn is_local_directory(path: &str) -> bool {
    let Ok(validated) = local_directory(path) else { return false; };
    let root: Vec<u16> = validated[..3].encode_utf16().chain(Some(0)).collect();
    matches!(unsafe { GetDriveTypeW(root.as_ptr()) }, 2 | 3) && std::path::Path::new(&validated).is_dir()
}

#[link(name = "kernel32")]
extern "system" { fn GetDriveTypeW(root: *const u16) -> u32; }

fn push_term(spec: &mut QuerySpec, value: &str) -> Result<(), String> {
    // 保留字符在首版明确拒绝，防止进入 Everything 宏、通配符或路径模式。
    if value
        .chars()
        .any(|c| "!*?<>|():\\/\"".contains(c) || c.is_control())
        || spec.terms.len() >= 8
    {
        return Err("Unsupported search characters".into());
    }
    spec.terms.push(value.into());
    Ok(())
}

pub fn local_directory(raw: &str) -> Result<String, String> {
    let path = raw.replace('/', "\\");
    let b = path.as_bytes();
    if b.len() < 3
        || !b[0].is_ascii_alphabetic()
        || b[1] != b':'
        || b[2] != b'\\'
        || path.encode_utf16().count() > 32767
        || path
            .chars()
            .any(|c| "\"*?<>|!".contains(c) || c.is_control())
        || path[2..].contains(':')
        || path[3..]
            .split('\\')
            .any(|c| c == "." || c == ".." || c.ends_with('.') || c.ends_with(' '))
    {
        return Err("A local absolute directory is required".into());
    }
    if path.len() == 3 {
        Ok(path)
    } else {
        Ok(path.trim_end_matches('\\').into())
    }
}

pub fn compile_everything(spec: &QuerySpec, settings: &Settings) -> String {
    let mut parts: Vec<String> = spec.terms.iter().map(|t| format!("\"{t}\"")).collect();
    if spec.scope == "files" {
        parts.push("file:".into());
    }
    if spec.scope == "folders" {
        parts.push("folder:".into());
    }
    if !spec.extensions.is_empty() {
        parts.push(format!("ext:{}", spec.extensions.join(";")));
    }
    if let Some(directory) = &spec.directory {
        parts.push(directory_filter(directory));
    }
    if !settings.include_directories.is_empty() {
        parts.push(format!(
            "<{}>",
            settings
                .include_directories
                .iter()
                .map(|p| directory_filter(p))
                .collect::<Vec<_>>()
                .join("|")
        ));
    }
    for directory in &settings.exclude_directories {
        parts.push(format!("!{}", directory_filter(directory)));
    }
    if settings.hide_hidden {
        parts.push("!attrib:H".into());
    }
    if settings.hide_system {
        parts.push("!attrib:S".into());
    }
    parts.join(" ")
}

fn directory_filter(path: &str) -> String {
    format!("\"{}\\\"", path.replace('/', "\\").trim_end_matches('\\'))
}

pub fn directory_matches(locator: &str, directory: &str) -> bool {
    let boundary = format!("{}\\", directory.replace('/', "\\").trim_end_matches('\\')).to_lowercase();
    let path = locator.replace('/', "\\").to_lowercase();
    if directory.starts_with('\\') { path.contains(&boundary) } else { path.starts_with(&boundary) }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn folder(path: &str) -> crate::model::Entry {
        crate::model::Entry { entity_id: path.into(), kind: "folder".into(), title: path.rsplit('\\').next().unwrap().into(), subtitle: String::new(), locator: path.into(), launch_kind: "path".into(), aliases: vec![], launch_key: path.into() }
    }
    #[test]
    fn exact_candidate_expression_keeps_metacharacters_literal_and_preserves_search_filters() {
        let spec = parse(r#"file: "报告[草稿]+v1.2$^{版}" ext:pdf in:D:\项目"#, "all").unwrap();
        let settings = Settings { include_directories: vec![r"D:\项目".into()], exclude_directories: vec![r"D:\项目\私密".into()], ..Settings::default() };
        let expression = compile_exact_candidates(&spec, &settings).unwrap();
        assert!(expression.contains(r#"<nopath:wfn:"报告[草稿]+v1.2$^{版}"|nopath:wfn:"报告[草稿]+v1.2$^{版}.*">"#), "{expression}");
        assert!(!expression.contains("regex:"));
        for filter in ["file:", "ext:pdf", "!attrib:H", "!attrib:S", r#""D:\项目\""#, r#"!"D:\项目\私密\""#, "count:50"] { assert!(expression.contains(filter), "{filter}: {expression}"); }
        assert!(compile_exact_candidates(&parse("report", "apps").unwrap(), &settings).is_none());
        assert!(compile_exact_candidates(&parse(r"D:\项目\", "files").unwrap(), &settings).is_none());
    }
    #[test]
    fn completions_accept_only_plain_path_prefixes() {
        for (raw, parent, prefix) in [(r"D:\项", r"D:\", "项"), (r"D:\项目\子", r"D:\项目", "子"), ("项目/子", r"\项目", "子"), (r"D:\项目\", r"D:\项目", ""), (r"D:\Work Files\Sub", r"D:\Work Files", "Sub"), ("Work Files/Sub", r"\Work Files", "Sub")] {
            let parsed = folder_prefix(raw).unwrap();
            assert_eq!(parsed.directory, parent, "{raw}");
            assert_eq!(parsed.prefix, prefix, "{raw}");
        }
        for raw in ["report", r"D:\项目 报告", r"D:\项目 ext:pdf", r"in:D:\项目", "folder: 项目/子", r"D:\项目\..\子", r"\\server\share", r"D:\项*", r"D:\项|备份", "项目/子 名"] {
            assert!(folder_prefix(raw).is_none(), "{raw}");
        }
    }
    #[test]
    fn completion_candidates_use_direct_children_full_queries_filters_and_five_limit() {
        let prefix = folder_prefix(r"D:\项目\子").unwrap();
        let settings = Settings { include_directories: vec![r"D:\项目".into()], exclude_directories: vec![r"D:\项目\子私密".into()], ..Settings::default() };
        let mut entries = vec![folder(r"D:\项目\子私密"), folder(r"D:\项目备份\子错误"), folder(r"D:\项目\子目录\子孙"), folder(r"D:\项目\其他子"), folder(r"E:\项目\子错误")];
        entries.extend((0..8).rev().map(|i| folder(&format!(r"D:\项目\子{i}"))));
        entries.push(folder(r"D:\项目\子0"));
        let candidates = folder_completions(&prefix, entries, &settings);
        assert_eq!(candidates.len(), 5);
        assert_eq!(candidates[0].path, r"D:\项目\子0");
        assert_eq!(candidates[0].query, "D:\\项目\\子0\\");
        assert_eq!(candidates[0].title, "子0");
        assert!(candidates.iter().all(|c| !c.path.contains("私密") && !c.path.contains("错误") && !c.path.contains("子孙")));
        let expression = compile_folder_completion(&prefix, &settings);
        assert!(expression.contains(r#"parent:"D:\项目""#) && expression.contains(r#"startwith:"子""#));
        assert!(expression.contains("folder:") && expression.contains("!attrib:H") && expression.contains("!attrib:S"));
        assert!(expression.contains(r#""D:\项目\""#) && expression.contains(r#"!"D:\项目\子私密\""#));
        let relative = folder_prefix("项目/子").unwrap();
        let candidates = folder_completions(&relative, vec![folder(r"D:\工作\项目\子目录"), folder(r"D:\项目\子目录\子孙"), folder(r"D:\项目备份\子错误")], &Settings::default());
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].path, r"D:\工作\项目\子目录");
    }
    #[test]
    fn folder_slash_filename_supports_absolute_and_named_folder_boundaries() {
        let folder = std::env::temp_dir().join(format!("SuperSearch-joined-query-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let input = folder.join("报告").to_string_lossy().into_owned();
        for raw in [input.clone(), input.replace('\\', "/")] {
            let q = parse(&raw, "all").unwrap();
            assert_eq!(q.directory.as_deref(), Some(folder.to_string_lossy().as_ref()));
            assert_eq!(q.terms, ["报告"]);
        }
        let quoted = parse(&format!("\"{}\"", folder.display()), "all").unwrap();
        assert_eq!(quoted.directory.as_deref(), Some(folder.to_string_lossy().as_ref()));
        assert!(quoted.terms.is_empty());
        std::fs::remove_dir(&folder).unwrap();
        let q = parse("项目/报告", "all").unwrap();
        assert_eq!(q.directory.as_deref(), Some(r"\项目"));
        assert_eq!(q.terms, ["报告"]);
        assert!(compile_everything(&q, &Settings::default()).contains(r#""\项目\""#));
        assert!(parse("项目/", "all").unwrap().terms.is_empty());
        for raw in ["../报告", "/项目/报告", "项目/*", "项目/../报告", "https://site/file"] {
            assert!(parse(raw, "all").is_err(), "{raw}");
        }
    }
    #[test]
    fn direct_folder_input_keeps_keywords_inside_the_directory_boundary() {
        let q = parse(r"D:\项目 报告", "all").unwrap();
        assert_eq!(q.directory.as_deref(), Some(r"D:\项目"));
        assert_eq!(q.terms, ["报告"]);
        assert_eq!(q.scope, "files");
        let compiled = compile_everything(&q, &Settings::default());
        assert!(compiled.contains(r#""D:\项目\""#));
        let root = parse(r"D:\", "all").unwrap();
        assert_eq!(root.directory.as_deref(), Some(r"D:\"));
        assert!(root.terms.is_empty());
    }
    #[test]
    fn direct_folder_input_supports_quotes_filters_and_rejects_conflicts() {
        let q = parse(r#""D:\Work Files" "年度 报告" ext:pdf"#, "files").unwrap();
        assert_eq!(q.directory.as_deref(), Some(r"D:\Work Files"));
        assert_eq!(q.terms, ["年度 报告"]);
        assert_eq!(q.extensions, ["pdf"]);
        assert_eq!(parse(r"D:/Work/ 项目", "folders").unwrap().scope, "folders");
        for (raw, scope) in [(r"D:\Work report", "apps"), (r"D:\Work app: report", "all"), (r"D:\Work report in:D:\Other", "all"), (r"D:\Work\..\Other report", "all"), (r"D:\Work? report", "all"), (r"\\server\share report", "all")] {
            assert!(parse(raw, scope).is_err(), "{raw}");
        }
    }
    #[test]
    fn unquoted_existing_directory_with_spaces_is_not_split_into_keywords() {
        let folder = std::env::temp_dir().join(format!("SuperSearch folder path {}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let raw = format!("{} 报告", folder.display());
        let q = parse(&raw, "all");
        let quoted = parse(&format!("\"{}\"", folder.display()), "all").unwrap();
        assert_eq!(quoted.directory.as_deref(), Some(folder.to_string_lossy().as_ref()));
        assert!(quoted.terms.is_empty());
        std::fs::remove_dir(&folder).unwrap();
        let q = q.unwrap();
        assert_eq!(q.directory.as_deref(), Some(folder.to_string_lossy().as_ref()));
        assert_eq!(q.terms, ["报告"]);
    }
    #[test]
    fn parses_phrase_extensions_and_local_directory() {
        let q = parse(
            r#"file: "年度 报告" ext:.PDF;docx in:"D:\Work Files""#,
            "all",
        )
        .unwrap();
        assert_eq!(q.terms, ["年度 报告"]);
        assert_eq!(q.extensions, ["pdf", "docx"]);
        assert_eq!(q.directory.as_deref(), Some(r"D:\Work Files"));
        assert_eq!(q.scope, "files");
    }
    #[test]
    fn rejects_engine_syntax_and_conflicting_filters() {
        for raw in [
            "content:secret",
            "regex:.*",
            "foo|bar",
            "*.pdf",
            "!report",
            "<report>",
            "app: x ext:pdf",
            "app: file: x",
            "\"unfinished",
            r"in:\\server\share",
        ] {
            assert!(parse(raw, "all").is_err(), "{raw}");
        }
        assert!(parse("app: x", "files").is_err());
        assert!(parse(&"字".repeat(513), "all").is_err());
        assert!(parse("a b c d e f g h i", "all").is_err());
        assert!(parse(r"in:C:\Work\..\Other", "all").is_err());
    }
    #[test]
    fn compiles_only_trusted_filters_with_directory_boundary() {
        let q = parse(r#"report in:"D:\Work" ext:pdf"#, "files").unwrap();
        let s = compile_everything(&q, &Settings::default());
        assert!(s.contains(r#""D:\Work\""#), "{s}");
        assert!(s.contains("file:") && s.contains("ext:pdf"));
        assert!(!s.contains("content:") && s.contains("report"));
    }
    #[test]
    fn stored_directory_filters_normalize_slashes_without_path_text_changes() {
        let settings = Settings {
            include_directories: vec!["D:/Work Files/".into()],
            exclude_directories: vec!["D:/Work Files/Private".into()],
            ..Settings::default()
        };
        let compiled = compile_everything(&parse("report", "all").unwrap(), &settings);
        assert!(compiled.contains(r#"<"D:\Work Files\">"#), "{compiled}");
        assert!(
            compiled.contains(r#"!"D:\Work Files\Private\""#),
            "{compiled}"
        );
    }
}
