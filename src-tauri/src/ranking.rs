use crate::{
    model::{Entry, UsageStat},
    query::QuerySpec,
};
use pinyin::ToPinyin;
use std::{collections::{HashMap, HashSet, VecDeque}, sync::{Arc, Mutex, OnceLock}};
use unicode_normalization::UnicodeNormalization;

pub fn normalize(text: &str) -> String {
    text.nfkc().flat_map(char::to_lowercase).collect()
}

pub fn match_tier(entry: &Entry, spec: &QuerySpec) -> Option<u8> {
    let terms: Vec<_> = spec.terms.iter().map(|term| normalize(term)).collect();
    match_normalized(entry, spec, &terms, &terms.join(" "), &normalize(&entry.title))
}

fn match_normalized(entry: &Entry, spec: &QuerySpec, terms: &[String], joined: &str, title: &str) -> Option<u8> {
    let app = entry.kind == "application";
    if (spec.scope == "apps" && !app)
        || (spec.scope == "files" && entry.kind != "file")
        || (spec.scope == "folders" && entry.kind != "folder")
    {
        return None;
    }
    if let Some(directory) = &spec.directory {
        if !crate::query::directory_matches(&entry.locator, directory) {
            return None;
        }
    }
    if !spec.extensions.is_empty() {
        let ext = entry
            .title
            .rsplit_once('.')
            .map(|(_, e)| e.to_ascii_lowercase())
            .unwrap_or_default();
        if entry.kind != "file" || !spec.extensions.contains(&ext) {
            return None;
        }
    }
    if spec.terms.is_empty() {
        return Some(2);
    }
    let exact_stem = entry.kind == "file" && title.rsplit_once('.').is_some_and(|(stem, _)| !stem.is_empty() && stem == joined);
    if title == joined || exact_stem || (app && entry.aliases.iter().any(|a| normalize(a) == joined)) {
        return Some(0);
    }
    if title.starts_with(joined) {
        return Some(1);
    }
    if terms.iter().all(|t| title.contains(t)) {
        return Some(2);
    }
    if !app {
        return None;
    }
    if entry
        .aliases
        .iter()
        .any(|a| terms.iter().all(|t| normalize(a).contains(t)))
    {
        return Some(3);
    }
    let keys = application_keys(title);
    if terms
        .iter()
        .all(|t| keys.full.contains(t) || keys.initials.contains(t))
    {
        return Some(3);
    }
    if terms
        .iter()
        .all(|t| keys.acronym.contains(t) || (t.chars().count() >= 2 && subsequence(t, title)))
    {
        return Some(4);
    }
    None
}

const APPLICATION_KEY_CAPACITY: usize = 512;
const APPLICATION_KEY_MAX_BYTES: usize = 512;

struct ApplicationKeys {
    full: String,
    initials: String,
    acronym: String,
}

#[derive(Default)]
struct ApplicationKeyCache {
    entries: HashMap<String, Arc<ApplicationKeys>>,
    order: VecDeque<String>,
}

impl ApplicationKeyCache {
    fn insert(&mut self, title: &str, keys: Arc<ApplicationKeys>) -> Arc<ApplicationKeys> {
        if let Some(existing) = self.entries.get(title) { return Arc::clone(existing); }
        if title.len() > APPLICATION_KEY_MAX_BYTES { return keys; }
        if self.entries.len() == APPLICATION_KEY_CAPACITY {
            if let Some(oldest) = self.order.pop_front() { self.entries.remove(&oldest); }
        }
        self.order.push_back(title.to_owned());
        self.entries.insert(title.to_owned(), Arc::clone(&keys));
        keys
    }
}

fn application_keys(title: &str) -> Arc<ApplicationKeys> {
    // 多个查询工作线程共享固定容量；只保存应用名称衍生键，不保存路径、别名、查询或使用记录。
    static CACHE: OnceLock<Mutex<ApplicationKeyCache>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(ApplicationKeyCache::default()));
    if let Ok(cache) = cache.lock() {
        if let Some(keys) = cache.entries.get(title) { return Arc::clone(keys); }
    }
    // 拼音计算不占用缓存锁；并发首次插入再核对内容键，名称变更自然重新计算。
    let (full, initials) = pinyin_keys(title);
    let keys = Arc::new(ApplicationKeys {
        full, initials,
        acronym: title.split(|c: char| !c.is_alphanumeric()).filter_map(|w| w.chars().next()).collect(),
    });
    cache.lock().map(|mut cache| cache.insert(title, Arc::clone(&keys))).unwrap_or(keys)
}

fn pinyin_keys(title: &str) -> (String, String) {
    #[cfg(test)]
    PINYIN_CONVERSIONS.with(|count| count.set(count.get() + 1));
    // 常见多音词优先采用固定读音，其余按字转换；特殊应用可由用户别名覆盖。
    let common = [
        ("重庆", ["chong", "qing"]),
        ("银行", ["yin", "hang"]),
        ("音乐", ["yin", "yue"]),
        ("乐器", ["yue", "qi"]),
        ("厦门", ["xia", "men"]),
        ("重启", ["chong", "qi"]),
        ("重复", ["chong", "fu"]),
        ("调音", ["tiao", "yin"]),
    ];
    let chars: Vec<char> = title.chars().collect();
    let mut full = String::new();
    let mut initials = String::new();
    let mut i = 0;
    while i < chars.len() {
        if i + 1 < chars.len() {
            let pair: String = chars[i..i + 2].iter().collect();
            if let Some((_, syllables)) = common.iter().find(|(word, _)| *word == pair) {
                for syllable in syllables {
                    full.push_str(syllable);
                    initials.push(syllable.chars().next().unwrap());
                }
                i += 2;
                continue;
            }
        }
        let c = chars[i];
        if let Some(py) = c.to_pinyin() {
            full.push_str(py.plain());
            if let Some(first) = py.plain().chars().next() {
                initials.push(first);
            }
        } else {
            full.push(c);
            if c.is_alphanumeric() {
                initials.push(c);
            }
        }
        i += 1;
    }
    (full, initials)
}

fn subsequence(needle: &str, haystack: &str) -> bool {
    let mut chars = haystack.chars();
    needle.chars().all(|c| chars.by_ref().any(|h| h == c))
}

pub fn rank_entries(
    entries: Vec<Entry>,
    spec: &QuerySpec,
    usage: &HashMap<String, UsageStat>,
    pins: &HashSet<String>,
) -> Vec<(Entry, u8)> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut unique = HashSet::new();
    // 查询词仅规范化一次；排序键在每个候选上计算一次，比较器不再重复分配字符串。
    let terms: Vec<_> = spec.terms.iter().map(|term| normalize(term)).collect();
    let joined = terms.join(" ");
    let mut ranked: Vec<_> = entries
        .into_iter()
        .filter_map(|e| {
            let key = if e.kind == "application" {
                format!("app:{}", e.launch_key)
            } else {
                format!("{}:{}", e.kind, e.locator.replace('/', "\\").to_lowercase())
            };
            if !unique.insert(key) {
                return None;
            }
            let title = normalize(&e.title);
            let tier = match_normalized(&e, spec, &terms, &joined, &title)?;
            let used = usage.get(&e.entity_id).is_some_and(|stat| stat.success_count > 0);
            let pinned = pins.contains(&e.entity_id);
            let score = usage_score(usage.get(&e.entity_id), now);
            Some((e, tier, used, pinned, score, title))
        })
        .collect();
    ranked.sort_by(|(a, at, au, ap, ascore, an), (b, bt, bu, bp, bscore, bn)| {
        bu.cmp(au)
            .then_with(|| at.cmp(bt))
            .then_with(|| bp.cmp(ap))
            .then_with(|| bscore.total_cmp(ascore))
            .then_with(|| an.cmp(bn))
            .then_with(|| a.entity_id.cmp(&b.entity_id))
    });
    ranked.into_iter().map(|(entry, tier, ..)| (entry, tier)).collect()
}

fn usage_score(stat: Option<&UsageStat>, now: u64) -> f64 {
    stat.filter(|s| s.success_count > 0).map(|s| {
        let age = now.saturating_sub(s.last_success_at);
        (2.0 * (1.0 + s.success_count as f64).ln()).min(12.0)
            + if age < 86400 {
                6.0
            } else if age < 7 * 86400 {
                3.0
            } else if age < 30 * 86400 {
                1.0
            } else {
                0.0
            }
    })
    .unwrap_or(0.0)
}

#[cfg(test)]
thread_local! {
    static PINYIN_CONVERSIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
#[path = "ranking_corpus.rs"]
mod corpus;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_quality_queries_keep_expected_results_in_top_three() {
        let catalog = vec![entry("微信", "application"), entry("Visual Studio Code", "application"),
            entry("重庆银行", "application"), entry("报告.pdf", "file"), entry("报告计划.pdf", "file"),
            entry("年度报告.pdf", "file"), entry("Annual Report.txt", "file"), entry("报告归档", "folder")];
        for (raw, scope, target) in [("微信", "apps", "微信"), ("weixin", "apps", "微信"),
            ("wx", "apps", "微信"), ("vsc", "apps", "Visual Studio Code"),
            ("cqyh", "apps", "重庆银行"), ("报告", "files", "报告.pdf"),
            ("report annual", "files", "Annual Report.txt"), ("报告", "folders", "报告归档"),
            ("报告 ext:pdf", "all", "报告.pdf")] {
            let ranked = rank_entries(catalog.clone(), &crate::query::parse(raw, scope).unwrap(), &HashMap::new(), &HashSet::new());
            assert!(ranked.iter().take(3).any(|(entry, _)| entry.title == target), "{raw}: {target}");
        }
    }

    #[test]
    fn ranking_workload_retains_deterministic_order_and_reports_elapsed_time() {
        let entries: Vec<_> = (0..1000).rev().map(|index| entry(&format!("报告-{index:04}.pdf"), "file")).collect();
        let spec = crate::query::parse("报告", "files").unwrap();
        let started = std::time::Instant::now();
        for _ in 0..50 {
            let ranked = rank_entries(entries.clone(), &spec, &HashMap::new(), &HashSet::new());
            assert_eq!(ranked.len(), 1000);
            assert_eq!(ranked[0].0.title, "报告-0000.pdf");
            assert_eq!(ranked[999].0.title, "报告-0999.pdf");
        }
        println!("Ranking workload: 50 x 1000 file candidates, elapsedMs={:.3}", started.elapsed().as_secs_f64() * 1000.0);
    }

    #[test]
    fn windows_path_variants_are_one_result_but_launch_arguments_remain_distinct() {
        let original = entry("报告.txt", "file");
        let mut duplicate = original.clone();
        duplicate.entity_id = "alternate-id".into();
        duplicate.locator = original.locator.replace('\\', "/").to_lowercase();
        let spec = crate::query::parse("报告", "all").unwrap();
        let ranked = rank_entries(vec![original.clone(), duplicate], &spec, &HashMap::new(), &HashSet::new());
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].0.entity_id, original.entity_id);
        let mut app = entry("报告工具", "application");
        app.launch_key = "tool.exe --profile Work".into();
        let mut other = app.clone();
        other.entity_id = "other-profile".into();
        other.launch_key = "tool.exe --profile work".into();
        assert_eq!(rank_entries(vec![app, other], &spec, &HashMap::new(), &HashSet::new()).len(), 2);
    }

    #[test]
    fn filename_stem_exact_precedes_prefix_substring_and_pins() {
        let spec = crate::query::parse("报告", "files").unwrap();
        let names = ["年度报告.pdf", "报告计划.pdf", "报告.pdf"];
        let pins = HashSet::from(["报告计划.pdf".into()]);
        let ranked = rank_entries(names.iter().map(|name| entry(name, "file")).collect(), &spec, &HashMap::new(), &pins);
        assert_eq!(ranked.iter().map(|(e, _)| e.title.as_str()).collect::<Vec<_>>(), ["报告.pdf", "报告计划.pdf", "年度报告.pdf"]);
        assert_eq!(ranked.iter().map(|(_, tier)| *tier).collect::<Vec<_>>(), [0, 1, 2]);
        assert_eq!(match_tier(&entry("REPORT.PDF", "file"), &crate::query::parse("report", "files").unwrap()), Some(0));
        assert_eq!(match_tier(&entry("report.pdf", "folder"), &crate::query::parse("report", "folders").unwrap()), Some(1));
    }
    #[test]
    fn named_folder_query_matches_nested_files_without_sibling_prefixes() {
        let spec = crate::query::parse("项目/报告", "all").unwrap();
        let mut file = entry("报告.txt", "file");
        file.locator = r"D:\工作\项目\子目录\报告.txt".into();
        assert_eq!(match_tier(&file, &spec), Some(0));
        file.locator = r"D:\工作\项目备份\报告.txt".into();
        assert_eq!(match_tier(&file, &spec), None);
    }
    fn entry(title: &str, kind: &str) -> Entry {
        Entry {
            entity_id: title.into(),
            title: title.into(),
            kind: kind.into(),
            subtitle: String::new(),
            locator: format!(r"C:\Apps\{title}"),
            launch_kind: "path".into(),
            aliases: vec![],
            launch_key: title.into(),
        }
    }
    #[test]
    fn direct_chinese_pinyin_initials_alias_and_subsequence_tiers() {
        let mut e = entry("微信", "application");
        assert_eq!(
            match_tier(&e, &crate::query::parse("微信", "all").unwrap()),
            Some(0)
        );
        assert_eq!(
            match_tier(&e, &crate::query::parse("weixin", "all").unwrap()),
            Some(3)
        );
        assert_eq!(
            match_tier(&e, &crate::query::parse("wx", "all").unwrap()),
            Some(3)
        );
        e.aliases.push("chat".into());
        assert_eq!(
            match_tier(&e, &crate::query::parse("chat", "all").unwrap()),
            Some(0)
        );
        assert_eq!(normalize("ＶＳＣＯＤＥ"), "vscode");
        assert_eq!(
            match_tier(
                &entry("Visual Studio Code", "application"),
                &crate::query::parse("vsc", "all").unwrap()
            ),
            Some(4)
        );
        assert_eq!(
            match_tier(
                &entry("Visual Studio Code", "file"),
                &crate::query::parse("vsc", "all").unwrap()
            ),
            None
        );
    }
    #[test]
    fn successful_history_beats_unused_exact_match_and_terms_are_and() {
        let exact = entry("Report", "file");
        let prefix = entry("Report Notes", "file");
        let usage = HashMap::from([(
            prefix.entity_id.clone(),
            UsageStat {
                success_count: u64::MAX,
                last_success_at: u64::MAX,
            },
        )]);
        let pins = HashSet::from([prefix.entity_id.clone()]);
        let ranked = rank_entries(
            vec![prefix, exact],
            &crate::query::parse("report", "all").unwrap(),
            &usage,
            &pins,
        );
        assert_eq!(ranked[0].0.title, "Report Notes");
        assert_eq!(
            match_tier(
                &entry("Annual Report", "file"),
                &crate::query::parse("report annual", "all").unwrap()
            ),
            Some(2)
        );
        assert_eq!(
            match_tier(
                &entry("Annual Report", "file"),
                &crate::query::parse("report missing", "all").unwrap()
            ),
            None
        );
    }
    #[test]
    fn common_polyphonic_application_names_match_full_and_initial_pinyin() {
        for (title, queries) in [
            ("重庆银行", ["chongqingyinhang", "cqyh"]),
            ("音乐播放器", ["yinyuebofangqi", "yybfq"]),
            ("厦门应用", ["xiamenyingyong", "xmyy"]),
        ] {
            for q in queries {
                assert_eq!(
                    match_tier(
                        &entry(title, "application"),
                        &crate::query::parse(q, "all").unwrap()
                    ),
                    Some(3),
                    "{title}: {q}"
                );
            }
        }
    }
}
