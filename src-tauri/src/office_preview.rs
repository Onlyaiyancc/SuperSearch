use quick_xml::{events::Event, Reader, XmlVersion};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Read;

const MAX_ENTRIES: usize = 10_000;
const MAX_EXPANDED: u64 = 64 * 1024 * 1024;
const MAX_XML: u64 = 16 * 1024 * 1024;
const MAX_TEXT: usize = 1024 * 1024;
const MAX_SHARED: usize = 2 * 1024 * 1024;
const MAX_CELL: usize = 64 * 1024;
const MAX_ROWS: usize = 200;
const MAX_COLUMNS: usize = 40;

#[derive(Debug, Serialize)]
pub struct OfficePreview {
    pub sections: Vec<OfficeSection>,
    pub truncated: bool,
}

#[derive(Debug, Serialize)]
pub struct OfficeSection {
    pub title: String,
    pub text: String,
    pub rows: Vec<Vec<String>>,
}

struct Package {
    zip: zip::ZipArchive<File>,
    remaining: u64,
}

impl Package {
    fn open(file: File) -> Result<Self, String> {
        let mut zip = zip::ZipArchive::new(file).map_err(|_| "Office 文件损坏或格式不受支持")?;
        if zip.len() > MAX_ENTRIES {
            return Err("Office 文件包含过多压缩条目".into());
        }
        let mut total = 0u64;
        let mut names = HashSet::new();
        for index in 0..zip.len() {
            let entry = zip
                .by_index(index)
                .map_err(|_| "无法读取 Office 压缩条目")?;
            total = total
                .checked_add(entry.size())
                .ok_or("Office 解压内容超过限制")?;
            let name = entry.name().map_err(|_| "Office 压缩条目名称无效")?.into_owned();
            if total > MAX_EXPANDED || !names.insert(name) {
                return Err("Office 解压内容超过限制或包含重复条目".into());
            }
        }
        let mut package = Self {
            zip,
            remaining: MAX_EXPANDED,
        };
        // 所有关系文件均检查，防止遗漏未被正文引用的外部内容。
        let relationships: Vec<_> = names
            .into_iter()
            .filter(|name| name.ends_with(".rels"))
            .collect();
        for name in relationships {
            relationships_from_xml(&package.xml(&name)?)?;
        }
        Ok(package)
    }

    fn xml(&mut self, name: &str) -> Result<String, String> {
        let entry = self
            .zip
            .by_name(name)
            .map_err(|_| "Office 文件缺少必要内容")?;
        if entry.size() > MAX_XML || entry.size() > self.remaining {
            return Err("Office XML 内容超过安全读取限制".into());
        }
        let mut bytes = Vec::new();
        entry
            .take(MAX_XML.min(self.remaining) + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "无法解压 Office 内容")?;
        if bytes.len() as u64 > MAX_XML || bytes.len() as u64 > self.remaining {
            return Err("Office XML 内容超过安全读取限制".into());
        }
        self.remaining -= bytes.len() as u64;
        String::from_utf8(bytes).map_err(|_| "Office XML 编码不受支持".into())
    }
}

enum Token {
    Start(String, HashMap<String, String>),
    End(String),
    Text(String),
}

fn valid_xml_character(character: char) -> bool {
    matches!(character, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')
}

fn valid_xml_name(name: &str) -> bool {
    let mut pieces = name.split(':');
    let mut valid_piece = |piece: &str| {
        let mut chars = piece.chars();
        let start = |character| {
            matches!(character, 'A'..='Z' | '_' | 'a'..='z'
            | '\u{c0}'..='\u{d6}' | '\u{d8}'..='\u{f6}' | '\u{f8}'..='\u{2ff}'
            | '\u{370}'..='\u{37d}' | '\u{37f}'..='\u{1fff}' | '\u{200c}'..='\u{200d}'
            | '\u{2070}'..='\u{218f}' | '\u{2c00}'..='\u{2fef}' | '\u{3001}'..='\u{d7ff}'
            | '\u{f900}'..='\u{fdcf}' | '\u{fdf0}'..='\u{fffd}' | '\u{10000}'..='\u{effff}')
        };
        chars.next().is_some_and(start)
            && chars.all(|character| start(character) || matches!(character, '-' | '.' | '0'..='9' | '\u{b7}' | '\u{300}'..='\u{36f}' | '\u{203f}'..='\u{2040}'))
    };
    valid_piece(pieces.next().unwrap_or_default())
        && pieces.next().is_none_or(&mut valid_piece)
        && pieces.next().is_none()
}

// 流式处理有界 XML；不建立 DOM、不解析 DTD，也不解析自定义实体。
fn walk_xml(xml: &str, mut visit: impl FnMut(Token) -> Result<(), String>) -> Result<(), String> {
    if !xml.chars().all(valid_xml_character) {
        return Err("Office XML 包含无效字符".into());
    }
    let mut reader = Reader::from_str(xml.trim_start_matches('\u{feff}'));
    reader.config_mut().enable_all_checks(true);
    let mut stack = Vec::new();
    let mut roots = 0;
    let mut events = 0;
    let mut declared = false;
    loop {
        events += 1;
        if events > 1_000_000 {
            return Err("Office XML 节点过多".into());
        }
        let event = reader.read_event().map_err(|_| "Office XML 内容损坏")?;
        let empty = matches!(&event, Event::Empty(_));
        match event {
            Event::Start(start) | Event::Empty(start) => {
                if stack.is_empty() {
                    roots += 1;
                }
                if roots > 1 || stack.len() >= 128 {
                    return Err("Office XML 结构无效或嵌套过深".into());
                }
                if !valid_xml_name(
                    std::str::from_utf8(start.name().as_ref())
                        .map_err(|_| "Office XML 名称无效")?,
                ) {
                    return Err("Office XML 名称无效".into());
                }
                let name = std::str::from_utf8(start.local_name().as_ref())
                    .map_err(|_| "Office XML 名称无效")?
                    .to_owned();
                let mut attrs = HashMap::new();
                for attribute in start.attributes() {
                    let attribute = attribute.map_err(|_| "Office XML 属性无效")?;
                    let key = std::str::from_utf8(attribute.key.as_ref())
                        .map_err(|_| "Office XML 属性无效")?
                        .to_owned();
                    if !valid_xml_name(&key) || attribute.value.contains(&b'<') {
                        return Err("Office XML 属性无效".into());
                    }
                    let value = attribute
                        .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
                        .map_err(|_| "Office XML 属性或实体无效")?
                        .into_owned();
                    if !value.chars().all(valid_xml_character) {
                        return Err("Office XML 属性包含无效字符".into());
                    }
                    attrs.insert(key, value);
                }
                visit(Token::Start(name.clone(), attrs))?;
                if empty {
                    visit(Token::End(name))?;
                } else {
                    stack.push(start.name().as_ref().to_vec());
                }
            }
            Event::End(end) => {
                if stack.pop().as_deref() != Some(end.name().as_ref()) {
                    return Err("Office XML 标签不匹配".into());
                }
                let name = std::str::from_utf8(end.local_name().as_ref())
                    .map_err(|_| "Office XML 名称无效")?
                    .to_owned();
                visit(Token::End(name))?;
            }
            Event::Text(text) => {
                let text =
                    std::str::from_utf8(text.as_ref()).map_err(|_| "Office XML 文字编码无效")?;
                if stack.is_empty() && !text.trim().is_empty() {
                    return Err("Office XML 正文结构无效".into());
                }
                visit(Token::Text(text.to_owned()))?;
            }
            Event::CData(text) => {
                if stack.is_empty() {
                    return Err("Office XML 正文结构无效".into());
                }
                visit(Token::Text(
                    std::str::from_utf8(text.as_ref())
                        .map_err(|_| "Office XML 文字编码无效")?
                        .to_owned(),
                ))?;
            }
            Event::GeneralRef(reference) => {
                if stack.is_empty() {
                    return Err("Office XML 实体结构无效".into());
                }
                let name =
                    std::str::from_utf8(reference.as_ref()).map_err(|_| "Office XML 实体无效")?;
                let value = match name {
                    "amp" => '&',
                    "lt" => '<',
                    "gt" => '>',
                    "apos" => '\'',
                    "quot" => '"',
                    _ => reference
                        .resolve_char_ref()
                        .map_err(|_| "Office XML 实体无效")?
                        .ok_or("不支持 Office 自定义实体")?,
                };
                if !valid_xml_character(value) {
                    return Err("Office XML 实体包含无效字符".into());
                }
                visit(Token::Text(value.to_string()))?;
            }
            Event::DocType(_) => return Err("不支持包含 DTD 的 Office 文件".into()),
            Event::Decl(declaration) => {
                if declared
                    || roots != 0
                    || declaration
                        .version()
                        .map_err(|_| "Office XML 声明无效")?
                        .as_ref()
                        != b"1.0"
                {
                    return Err("Office XML 声明无效".into());
                }
                if let Some(encoding) = declaration.encoding() {
                    if !encoding
                        .map_err(|_| "Office XML 编码无效")?
                        .eq_ignore_ascii_case(b"UTF-8")
                    {
                        return Err("Office XML 编码不受支持".into());
                    }
                }
                declared = true;
            }
            Event::Eof => {
                if roots != 1 || !stack.is_empty() {
                    return Err("Office XML 内容不完整".into());
                }
                return Ok(());
            }
            _ => {}
        }
    }
}

fn relationships_from_xml(xml: &str) -> Result<HashMap<String, String>, String> {
    let mut relationships = HashMap::new();
    walk_xml(xml, |token| {
        if let Token::Start(name, attrs) = token {
            if name == "Relationship" {
                let target = attrs.get("Target").ok_or("Office 关系缺少目标")?;
                if attrs
                    .get("TargetMode")
                    .is_some_and(|mode| mode != "Internal")
                    || target.contains(':')
                    || target.contains('\\')
                    || target.starts_with("//")
                {
                    return Err("不支持包含外部关系的 Office 文件".into());
                }
                let id = attrs.get("Id").ok_or("Office 关系缺少标识")?;
                if relationships.insert(id.clone(), target.clone()).is_some() {
                    return Err("Office 关系标识重复".into());
                }
            }
        }
        Ok(())
    })?;
    Ok(relationships)
}

fn part_target(base: &str, target: &str) -> Result<String, String> {
    let path = if target.starts_with('/') {
        target.trim_start_matches('/').to_owned()
    } else {
        format!("{base}/{target}")
    };
    let mut components = Vec::new();
    for component in path.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                components.pop().ok_or("Office 关系目标越界")?;
            }
            other => components.push(other),
        }
    }
    Ok(components.join("/"))
}

fn append_bounded(output: &mut String, text: &str, remaining: &mut usize, truncated: &mut bool) {
    let mut end = text.len().min(*remaining);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    output.push_str(&text[..end]);
    *remaining -= end;
    *truncated |= end < text.len();
}

fn document_text(xml: &str, remaining: &mut usize, truncated: &mut bool) -> Result<String, String> {
    let mut text = String::new();
    let mut in_text = false;
    walk_xml(xml, |token| {
        match token {
            Token::Start(name, _) if name == "t" => in_text = true,
            Token::Start(name, _) if name == "tab" => {
                append_bounded(&mut text, "\t", remaining, truncated)
            }
            Token::Start(name, _) if name == "br" => {
                append_bounded(&mut text, "\n", remaining, truncated)
            }
            Token::End(name) if name == "t" => in_text = false,
            Token::End(name) if name == "p" || name == "tr" => {
                append_bounded(&mut text, "\n", remaining, truncated)
            }
            Token::End(name) if name == "tc" => {
                append_bounded(&mut text, "\t", remaining, truncated)
            }
            Token::Text(value) if in_text => {
                append_bounded(&mut text, &value, remaining, truncated)
            }
            _ => {}
        }
        Ok(())
    })?;
    Ok(text)
}

pub fn read_office(file: File, extension: &str) -> Result<OfficePreview, String> {
    if !matches!(extension, "docx" | "xlsx" | "pptx") {
        return Err("不支持此 Office 文件格式".into());
    }
    let mut package = Package::open(file)?;
    let mut result = OfficePreview {
        sections: Vec::new(),
        truncated: false,
    };
    let mut remaining = MAX_TEXT;
    match extension {
        "docx" => {
            let text = document_text(
                &package.xml("word/document.xml")?,
                &mut remaining,
                &mut result.truncated,
            )?;
            result.sections.push(OfficeSection {
                title: "正文".into(),
                text,
                rows: Vec::new(),
            });
        }
        "pptx" => {
            let mut ids = Vec::new();
            walk_xml(&package.xml("ppt/presentation.xml")?, |token| {
                if let Token::Start(name, attrs) = token {
                    if name == "sldId" {
                        let id = attrs.get("r:id").ok_or("幻灯片缺少关系标识")?;
                        if ids.len() < 100 {
                            ids.push(id.clone());
                        } else {
                            result.truncated = true;
                        }
                    }
                }
                Ok(())
            })?;
            let relationships =
                relationships_from_xml(&package.xml("ppt/_rels/presentation.xml.rels")?)?;
            for (index, id) in ids.iter().enumerate() {
                if remaining == 0 {
                    result.truncated = true;
                    break;
                }
                let path = part_target("ppt", relationships.get(id).ok_or("幻灯片关系不存在")?)?;
                let text =
                    document_text(&package.xml(&path)?, &mut remaining, &mut result.truncated)?;
                result.sections.push(OfficeSection {
                    title: format!("幻灯片 {}", index + 1),
                    text,
                    rows: Vec::new(),
                });
            }
        }
        "xlsx" => read_workbook(&mut package, &mut result, &mut remaining)?,
        _ => unreachable!(),
    }
    Ok(result)
}

fn shared_strings(xml: &str, truncated: &mut bool) -> Result<Vec<String>, String> {
    let mut strings = Vec::new();
    let mut text = String::new();
    let mut in_text = false;
    let mut remaining = MAX_SHARED;
    let mut cell_remaining = MAX_CELL;
    walk_xml(xml, |token| {
        match token {
            Token::Start(name, _) if name == "si" => {
                text.clear();
                cell_remaining = MAX_CELL.min(remaining);
            }
            Token::Start(name, _) if name == "t" => in_text = true,
            Token::Text(value) if in_text && strings.len() < 100_000 => {
                append_bounded(&mut text, &value, &mut cell_remaining, truncated)
            }
            Token::End(name) if name == "t" => in_text = false,
            Token::End(name) if name == "si" => {
                if strings.len() < 100_000 && remaining > 0 {
                    remaining -= text.len();
                    strings.push(std::mem::take(&mut text));
                } else {
                    *truncated = true;
                }
            }
            _ => {}
        }
        Ok(())
    })?;
    Ok(strings)
}

fn cell_position(reference: &str) -> Result<(usize, usize), String> {
    let letters = reference
        .bytes()
        .take_while(u8::is_ascii_alphabetic)
        .count();
    if letters == 0 || letters == reference.len() {
        return Err("工作表单元格位置无效".into());
    }
    let mut column = 0usize;
    for letter in reference[..letters].bytes() {
        column = column
            .checked_mul(26)
            .and_then(|value| value.checked_add((letter.to_ascii_uppercase() - b'A' + 1) as usize))
            .ok_or("工作表单元格位置无效")?;
    }
    let row = reference[letters..]
        .parse::<usize>()
        .map_err(|_| "工作表单元格位置无效")?;
    if row == 0 {
        return Err("工作表单元格位置无效".into());
    }
    Ok((row - 1, column - 1))
}

fn read_workbook(
    package: &mut Package,
    result: &mut OfficePreview,
    remaining: &mut usize,
) -> Result<(), String> {
    let mut sheets = Vec::new();
    walk_xml(&package.xml("xl/workbook.xml")?, |token| {
        if let Token::Start(name, attrs) = token {
            if name == "sheet" {
                if sheets.len() < 10 {
                    let id = attrs.get("r:id").ok_or("工作表缺少关系标识")?.clone();
                    let mut title = String::new();
                    let mut title_limit = 256;
                    append_bounded(
                        &mut title,
                        attrs.get("name").ok_or("工作表缺少名称")?,
                        &mut title_limit,
                        &mut result.truncated,
                    );
                    sheets.push((title, id));
                } else {
                    result.truncated = true;
                }
            }
        }
        Ok(())
    })?;
    let relationships = relationships_from_xml(&package.xml("xl/_rels/workbook.xml.rels")?)?;
    let shared = if package
        .zip
        .file_names()
        .any(|name| name.is_ok_and(|name| name == "xl/sharedStrings.xml"))
    {
        shared_strings(&package.xml("xl/sharedStrings.xml")?, &mut result.truncated)?
    } else {
        Vec::new()
    };
    for (title, id) in sheets {
        if *remaining == 0 {
            result.truncated = true;
            break;
        }
        let path = part_target("xl", relationships.get(&id).ok_or("工作表关系不存在")?)?;
        let rows = worksheet(
            &package.xml(&path)?,
            &shared,
            remaining,
            &mut result.truncated,
        )?;
        result.sections.push(OfficeSection {
            title,
            text: String::new(),
            rows,
        });
    }
    Ok(())
}

fn worksheet(
    xml: &str,
    shared: &[String],
    remaining: &mut usize,
    truncated: &mut bool,
) -> Result<Vec<Vec<String>>, String> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row = 0usize;
    let mut column = 0usize;
    let mut kind = String::new();
    let mut value = String::new();
    let mut in_cell = false;
    let mut capture = false;
    let mut cell_remaining = MAX_CELL;
    walk_xml(xml, |token| {
        match token {
            Token::Start(name, attrs) if name == "row" => {
                row = match attrs.get("r") {
                    Some(value) => value
                        .parse::<usize>()
                        .ok()
                        .and_then(|value| value.checked_sub(1))
                        .ok_or("工作表行位置无效")?,
                    None => row,
                };
                column = 0;
            }
            Token::Start(name, attrs) if name == "c" => {
                if let Some(reference) = attrs.get("r") {
                    (row, column) = cell_position(reference)?;
                }
                kind = attrs.get("t").cloned().unwrap_or_default();
                value.clear();
                in_cell = true;
                cell_remaining = MAX_CELL;
            }
            Token::Start(name, _) if in_cell && (name == "v" || name == "t") => capture = true,
            Token::Text(text) if in_cell && capture => {
                append_bounded(&mut value, &text, &mut cell_remaining, truncated)
            }
            Token::End(name) if name == "v" || name == "t" => capture = false,
            Token::End(name) if name == "c" => {
                in_cell = false;
                if row < MAX_ROWS && column < MAX_COLUMNS {
                    let resolved = match kind.as_str() {
                        "s" => value
                            .trim()
                            .parse::<usize>()
                            .ok()
                            .and_then(|index| shared.get(index))
                            .map(String::as_str)
                            .unwrap_or_else(|| {
                                *truncated = true;
                                ""
                            }),
                        "b" => {
                            if value.trim() == "1" {
                                "TRUE"
                            } else {
                                "FALSE"
                            }
                        }
                        _ => &value,
                    };
                    let mut cell = String::new();
                    append_bounded(&mut cell, resolved, remaining, truncated);
                    if rows.len() <= row {
                        rows.resize_with(row + 1, Vec::new);
                    }
                    if rows[row].len() <= column {
                        rows[row].resize(column + 1, String::new());
                    }
                    rows[row][column] = cell;
                } else {
                    *truncated = true;
                }
                column = column.saturating_add(1);
            }
            Token::End(name) if name == "row" => row = row.saturating_add(1),
            _ => {}
        }
        Ok(())
    })?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};
    use zip::write::SimpleFileOptions;

    fn preview(parts: &[(&str, &str)], extension: &str) -> Result<OfficePreview, String> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "supersearch-office-{}-{}.zip",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let file = File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        for (name, xml) in parts {
            zip.start_file(
                *name,
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
            )
            .unwrap();
            zip.write_all(xml.as_bytes()).unwrap();
        }
        drop(zip.finish().unwrap());
        let result = read_office(File::open(&path).unwrap(), extension);
        std::fs::remove_file(path).unwrap();
        result
    }

    #[test]
    fn docx_extracts_paragraphs_tables_and_xml_entities() {
        let result = preview(&[("word/document.xml", "<w:document xmlns:w='w'><w:body><w:p><w:r><w:t>Hello &amp; 世界</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>")], "docx").unwrap();
        assert!(result.sections[0].text.contains("Hello & 世界"));
        assert!(result.sections[0].text.contains("Cell"));
        assert!(!result.truncated);
    }

    #[test]
    fn xlsx_uses_sheet_order_names_and_sparse_cell_positions() {
        let result = preview(&[
            ("xl/workbook.xml", "<workbook xmlns:r='r'><sheets><sheet name='Summary' r:id='b'/><sheet name='Data' r:id='a'/></sheets></workbook>"),
            ("xl/_rels/workbook.xml.rels", "<Relationships><Relationship Id='a' Target='worksheets/sheet1.xml'/><Relationship Id='b' Target='worksheets/sheet2.xml'/></Relationships>"),
            ("xl/sharedStrings.xml", "<sst><si><r><t>Shared </t></r><r><t>value</t></r></si></sst>"),
            ("xl/worksheets/sheet1.xml", "<worksheet><sheetData><row r='1'><c r='A1'><v>42</v></c></row></sheetData></worksheet>"),
            ("xl/worksheets/sheet2.xml", "<worksheet><sheetData><row r='3'><c r='B3' t='s'><v>0</v></c><c r='D3' t='inlineStr'><is><t>Inline</t></is></c><c r='E3' t='b'><v>1</v></c></row></sheetData></worksheet>"),
        ], "xlsx").unwrap();
        assert_eq!(result.sections[0].title, "Summary");
        assert_eq!(result.sections[1].title, "Data");
        assert_eq!(
            result.sections[0].rows[2],
            ["", "Shared value", "", "Inline", "TRUE"]
        );
        assert!(result.sections[0].rows[0].is_empty());
        assert_eq!(result.sections[1].rows[0][0], "42");
    }

    #[test]
    fn pptx_follows_presentation_order() {
        let result = preview(&[
            ("ppt/presentation.xml", "<p:presentation xmlns:p='p' xmlns:r='r'><p:sldIdLst><p:sldId r:id='b'/><p:sldId r:id='a'/></p:sldIdLst></p:presentation>"),
            ("ppt/_rels/presentation.xml.rels", "<Relationships><Relationship Id='a' Target='slides/slide1.xml'/><Relationship Id='b' Target='slides/slide2.xml'/></Relationships>"),
            ("ppt/slides/slide1.xml", "<p:sld xmlns:p='p' xmlns:a='a'><a:p><a:r><a:t>First file</a:t></a:r></a:p></p:sld>"),
            ("ppt/slides/slide2.xml", "<p:sld xmlns:p='p' xmlns:a='a'><a:p><a:r><a:t>Second file</a:t></a:r></a:p></p:sld>"),
        ], "pptx").unwrap();
        assert_eq!(result.sections[0].title, "幻灯片 1");
        assert_eq!(result.sections[0].text.trim(), "Second file");
        assert_eq!(result.sections[1].text.trim(), "First file");
    }

    #[test]
    fn rejects_malformed_xml_dtd_entities_and_external_relationships() {
        for xml in [
            "<document><t>bad</document>",
            "<!DOCTYPE d [<!ENTITY x 'bad'>]><d>&x;</d>",
            "<d><t>&custom;</t></d>",
            "<d/><d/>",
            "<d>",
            "<d><t>&#1;</t></d>",
            "<d a='&#1;'/>",
            "<d>\u{1}</d>",
            "<1/>",
            "<d a='<'/>",
            "<d><?xml version='1.0'?></d>",
        ] {
            assert!(
                preview(&[("word/document.xml", xml)], "docx").is_err(),
                "{xml}"
            );
        }
        assert!(preview(&[("word/document.xml", "<document/>"), ("word/_rels/document.xml.rels", "<Relationships><Relationship Id='x' Target='https://example.test' TargetMode='External'/></Relationships>")], "docx").is_err());
    }

    #[test]
    fn enforces_zip_entry_and_aggregate_expansion_limits() {
        let parts: Vec<_> = (0..=MAX_ENTRIES)
            .map(|index| (format!("part{index}"), "x"))
            .collect();
        let borrowed: Vec<_> = parts
            .iter()
            .map(|(name, value)| (name.as_str(), *value))
            .collect();
        assert!(preview(&borrowed, "docx").unwrap_err().contains("过多"));
        let huge = "x".repeat(MAX_EXPANDED as usize);
        assert!(preview(
            &[("word/document.xml", "<document/>"), ("unused.bin", &huge)],
            "docx"
        )
        .unwrap_err()
        .contains("解压"));
    }

    #[test]
    fn limits_sheet_count_and_shared_string_storage() {
        let mut workbook = String::from("<workbook xmlns:r='r'><sheets>");
        let mut relationships = String::from("<Relationships>");
        for index in 0..12 {
            workbook.push_str(&format!("<sheet name='Sheet {index}' r:id='s{index}'/>"));
            relationships.push_str(&format!(
                "<Relationship Id='s{index}' Target='worksheets/data.xml'/>"
            ));
        }
        workbook.push_str("</sheets></workbook>");
        relationships.push_str("</Relationships>");
        let shared = format!(
            "<sst><si><t>{}</t></si><si><t>Tail</t></si></sst>",
            "a".repeat(MAX_CELL + 10)
        );
        let result = preview(&[
            ("xl/workbook.xml", &workbook), ("xl/_rels/workbook.xml.rels", &relationships), ("xl/sharedStrings.xml", &shared),
            ("xl/worksheets/data.xml", "<worksheet><sheetData><row><c t='s'><v>0</v></c><c t='s'><v>1</v></c></row></sheetData></worksheet>"),
        ], "xlsx").unwrap();
        assert_eq!(result.sections.len(), 10);
        assert_eq!(result.sections[0].rows[0][0].len(), MAX_CELL);
        assert_eq!(result.sections[0].rows[0][1], "Tail");
        assert!(result.truncated);
    }

    #[test]
    fn rejects_deep_xml_and_escaping_internal_targets() {
        let nested = format!("{}{}", "<d>".repeat(130), "</d>".repeat(130));
        assert!(preview(&[("word/document.xml", &nested)], "docx").is_err());
        assert!(preview(&[
            ("xl/workbook.xml", "<workbook xmlns:r='r'><sheets><sheet name='Bad' r:id='a'/></sheets></workbook>"),
            ("xl/_rels/workbook.xml.rels", "<Relationships><Relationship Id='a' Target='../../outside.xml'/></Relationships>"),
        ], "xlsx").is_err());
    }

    #[test]
    fn bounds_xml_bombs_output_and_worksheet_dimensions() {
        let large = format!(
            "<document><t>{}</t></document>",
            "a".repeat(16 * 1024 * 1024)
        );
        assert!(preview(&[("word/document.xml", &large)], "docx").is_err());
        let text = format!("<document><t>{}</t></document>", "世".repeat(400_000));
        let result = preview(&[("word/document.xml", &text)], "docx").unwrap();
        assert!(result.truncated);
        assert!(result.sections[0].text.len() <= 1024 * 1024);
        let result = preview(&[
            ("xl/workbook.xml", "<workbook xmlns:r='r'><sheets><sheet name='Bounded' r:id='a'/></sheets></workbook>"),
            ("xl/_rels/workbook.xml.rels", "<Relationships><Relationship Id='a' Target='worksheets/sheet1.xml'/></Relationships>"),
            ("xl/worksheets/sheet1.xml", "<worksheet><sheetData><row r='1'><c r='AN1'><v>40</v></c><c r='AO1'><v>41</v></c></row><row r='201'><c r='A201'><v>hidden</v></c></row></sheetData></worksheet>"),
        ], "xlsx").unwrap();
        assert_eq!(result.sections[0].rows[0].len(), 40);
        assert_eq!(result.sections[0].rows.len(), 1);
        assert!(result.truncated);
    }
}
