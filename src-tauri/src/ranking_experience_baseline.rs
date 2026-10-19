// 本轮改动前的 0.1.6 排序快照，仅用于同进程对比测试。
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
