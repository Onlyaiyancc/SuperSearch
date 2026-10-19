use super::*;
use std::time::Instant;

#[path = "ranking_baseline.rs"]
mod baseline;

fn entry(title: &str, kind: &str, directory: &str) -> Entry {
    let locator = format!(r"{directory}\{title}");
    Entry {
        entity_id: locator.clone(), title: title.into(), kind: kind.into(),
        subtitle: String::new(), launch_kind: "path".into(), aliases: vec![],
        launch_key: locator.clone(), locator,
    }
}

// 合成应用目录和人工指定的相关结果；不是实际用户日志或线上相关性评估。
const APPS: &[(&str, &str, &str, &str)] = &[
    ("微信", "weixin", "wx", "wechat"),
    ("钉钉", "dingding", "dd", "dingtalk"),
    ("飞书", "feishu", "fs", "lark"),
    ("企业微信", "qiyeweixin", "qywx", "wecom"),
    ("腾讯会议", "tengxunhuiyi", "txhy", "meeting"),
    ("网易云音乐", "wangyiyunyinyue", "wyyyy", "cloudmusic"),
    ("音乐播放器", "yinyuebofangqi", "yybfq", "musicplayer"),
    ("重庆银行", "chongqingyinhang", "cqyh", "cqbank"),
    ("厦门应用", "xiamenyingyong", "xmyy", "xiamenapp"),
    ("重启工具", "chongqigongju", "cqgj", "reboot"),
    ("重复文件", "chongfuwenjian", "cfwj", "deduplicate"),
    ("乐器调音", "yueqitiaoyin", "yqty", "tuner"),
    ("记事本", "jishiben", "jsb", "notepad"),
    ("计算器", "jisuanqi", "jsq", "calculator"),
    ("截图工具", "jietugongju", "jtgj", "screenshot"),
    ("画图", "huatu", "ht", "paint"),
    ("文件管理器", "wenjianguanliqi", "wjglq", "explorer"),
    ("任务管理器", "renwuguanliqi", "rwglq", "taskmanager"),
    ("系统设置", "xitongshezhi", "xtsz", "settings"),
    ("百度网盘", "baiduwangpan", "bdwp", "baidudrive"),
    ("阿里云盘", "aliyunpan", "alyp", "alidrive"),
    ("有道词典", "youdaocidian", "ydcd", "youdao"),
    ("金山词霸", "jinshanciba", "jscb", "kingsoft"),
    ("搜狗输入法", "sougoushurufa", "sgsrf", "sogou"),
    ("剪映", "jianying", "jy", "capcut"),
    ("哔哩哔哩", "bilibili", "blbl", "bilibili"),
    ("腾讯视频", "tengxunshipin", "txsp", "tencentvideo"),
    ("远程桌面", "yuanchengzhuomian", "yczm", "remotedesktop"),
];

#[test]
fn synthetic_quality_corpus_reports_top_one_and_three() {
    let mut catalog: Vec<_> = APPS.iter().map(|(title, _, _, alias)| {
        let mut e = entry(title, "application", r"C:\Apps");
        e.aliases.push((*alias).into());
        e
    }).collect();
    for (title, alias) in [("Visual Studio Code", "vscode"), ("Google Chrome", "browser"),
        ("Windows Terminal", "terminal"), ("Microsoft PowerToys", "powertoys")] {
        let mut e = entry(title, "application", r"C:\Apps");
        e.aliases.push(alias.into());
        catalog.push(e);
    }
    let filenames = ["报告.pdf", "报告计划.pdf", "年度报告.pdf", "Annual Report.txt", "会议纪要.docx",
        "预算.xlsx", "设计稿.png", "项目说明.md", "合同.pdf", "Release Notes.txt"];
    catalog.extend(filenames.iter().map(|name| entry(name, "file", r"D:\工作\项目")));
    catalog.extend([entry("报告.pdf", "file", r"D:\工作\项目\归档"),
        entry("报告.pdf", "file", r"D:\工作\项目备份"),
        entry("报告归档", "folder", r"D:\工作"), entry("项目", "folder", r"D:\工作")]);
    let mut duplicate = entry("报告.pdf", "file", r"D:\工作\项目");
    duplicate.locator = duplicate.locator.replace('\\', "/").to_lowercase();
    duplicate.entity_id = "duplicate-path".into();
    catalog.push(duplicate);
    let mut cases: Vec<(String, &str, String, bool, bool)> = Vec::new();
    for (title, full, initials, alias) in APPS {
        let target = format!(r"C:\Apps\{title}");
        for query in [*title, *full, *initials, *alias] {
            cases.push((query.into(), "apps", target.clone(), false, false));
        }
    }
    for (query, target) in [("vsc", "Visual Studio Code"), ("vscode", "Visual Studio Code"),
        ("ＶＳＣＯＤＥ", "Visual Studio Code"), ("studio code", "Visual Studio Code"),
        ("gc", "Google Chrome"), ("browser", "Google Chrome"),
        ("wt", "Windows Terminal"), ("terminal", "Windows Terminal"),
        ("mpt", "Microsoft PowerToys"), ("powertoys", "Microsoft PowerToys")] {
        cases.push((query.into(), "apps", format!(r"C:\Apps\{target}"), false, false));
    }
    for (query, scope, target) in [
        ("报告", "files", r"D:\工作\项目\报告.pdf"),
        ("file:报告 ext:pdf", "all", r"D:\工作\项目\报告.pdf"),
        ("报告 ext:pdf;txt", "all", r"D:\工作\项目\报告.pdf"),
        ("report annual", "files", r"D:\工作\项目\Annual Report.txt"),
        ("annual report ext:txt", "files", r"D:\工作\项目\Annual Report.txt"),
        ("会议纪要", "files", r"D:\工作\项目\会议纪要.docx"),
        ("预算 ext:xlsx", "files", r"D:\工作\项目\预算.xlsx"),
        ("设计稿 ext:png", "all", r"D:\工作\项目\设计稿.png"),
        ("项目说明 ext:md", "files", r"D:\工作\项目\项目说明.md"),
        ("合同 ext:pdf", "files", r"D:\工作\项目\合同.pdf"),
        ("notes release", "files", r"D:\工作\项目\Release Notes.txt"),
        ("folder:报告", "all", r"D:\工作\报告归档"),
        ("项目", "folders", r"D:\工作\项目"),
        ("项目/预算", "all", r"D:\工作\项目\预算.xlsx"),
        ("项目/归档/报告", "all", r"D:\工作\项目\归档\报告.pdf"),
        (r"报告 in:D:\工作\项目备份", "files", r"D:\工作\项目备份\报告.pdf"),
        (r"file:报告 in:D:\工作\项目 ext:pdf", "all", r"D:\工作\项目\报告.pdf"),
    ] {
        cases.push((query.into(), scope, target.into(), false, false));
    }
    // 历史优先、同级置顶，以及无匹配结果也是明确的人工标签。
    cases.push(("报告".into(), "files", r"D:\工作\项目\年度报告.pdf".into(), true, false));
    cases.push(("报告".into(), "files", r"D:\工作\项目备份\报告.pdf".into(), false, true));
    for (query, scope) in [("报告 missing", "files"), ("报告 ext:zip", "files"),
        ("weixin", "files"), ("budgetunknown", "apps"), ("项目备份/预算", "all"),
        ("微信", "folders"), ("不存在的工具", "apps")] {
        cases.push((query.into(), scope, String::new(), false, false));
    }
    let mut top1 = 0;
    let mut top3 = 0;
    let mut baseline_top1 = 0;
    let mut baseline_top3 = 0;
    let mut previous_top1 = 0;
    let mut previous_top3 = 0;
    for (query, scope, target, historical, pinned) in &cases {
        let usage = if *historical { HashMap::from([(target.clone(), UsageStat { success_count: 1, last_success_at: u64::MAX })]) } else { HashMap::new() };
        let pins = if *pinned { HashSet::from([target.clone()]) } else { HashSet::new() };
        let spec = crate::query::parse(query, scope).unwrap();
        let ranked = rank_entries(catalog.clone(), &spec, &usage, &pins);
        let before = baseline::rank_entries(catalog.clone(), &spec, &usage, &pins);
        let previous = super::experience_baseline::rank_entries(catalog.clone(), &spec, &usage, &pins);
        // 未指定历史、置顶或归档目录时，同名精确 PDF 的不同合法路径均相关。
        let relevant = |e: &Entry| e.entity_id == *target || (!historical && !pinned && target == r"D:\工作\项目\报告.pdf" && e.title == "报告.pdf");
        let one = if target.is_empty() { ranked.is_empty() } else { ranked.first().is_some_and(|(e, _)| relevant(e)) };
        let three = if target.is_empty() { ranked.is_empty() } else { ranked.iter().take(3).any(|(e, _)| relevant(e)) };
        top1 += usize::from(one);
        top3 += usize::from(three);
        baseline_top1 += usize::from(if target.is_empty() { before.is_empty() } else { before.first().is_some_and(|(e, _)| relevant(e)) });
        baseline_top3 += usize::from(if target.is_empty() { before.is_empty() } else { before.iter().take(3).any(|(e, _)| relevant(e)) });
        previous_top1 += usize::from(if target.is_empty() { previous.is_empty() } else { previous.first().is_some_and(|(e, _)| relevant(e)) });
        previous_top3 += usize::from(if target.is_empty() { previous.is_empty() } else { previous.iter().take(3).any(|(e, _)| relevant(e)) });
        if !one { println!("Synthetic top1 miss: query={query:?}, target={target:?}, actual={:?}", ranked.iter().take(3).map(|(e, _)| &e.title).collect::<Vec<_>>()); }
        assert!(three, "Top3 regression: {query}: {target}");
        if !["yueqitiaoyin", "yqty"].contains(&query.as_str()) {
            if spec.scope == "apps" {
                // 本轮有意细化应用同级顺序；候选及公开等级保持原有召回语义。
                let membership = |items: &[(Entry, u8)]| {
                    let mut members: Vec<_> = items.iter().map(|(e, tier)| (e.entity_id.clone(), *tier)).collect();
                    members.sort();
                    members
                };
                assert_eq!(membership(&ranked), membership(&before), "Changed candidate membership: {query}");
            } else {
                assert_eq!(ranked, before, "Changed file ranking policy: {query}");
            }
        }
        let paths: Vec<_> = ranked.iter().filter(|(e, _)| e.kind == "file").map(|(e, _)| e.locator.replace('/', "\\").to_lowercase()).collect();
        assert_eq!(paths.iter().collect::<HashSet<_>>().len(), paths.len(), "Duplicate path: {query}");
    }
    assert!(cases.len() >= 100);
    println!("Synthetic corpus: queries={}, candidates={}, baselineTop1={}/{}, baselineTop3={}/{}, optimizedTop1={}/{}, optimizedTop3={}/{}", cases.len(), catalog.len(), baseline_top1, cases.len(), baseline_top3, cases.len(), top1, cases.len(), top3, cases.len());
    println!("Experience quality: previousTop1={previous_top1}/{} previousTop3={previous_top3}/{} afterTop1={top1}/{} afterTop3={top3}/{} expectedEmpty=7", cases.len(), cases.len(), cases.len(), cases.len());
    assert_eq!((baseline_top1, baseline_top3, top1, top3), (143, 146, 148, 148));
    assert_eq!((previous_top1, previous_top3), (145, 148));
}

#[test]
fn tuning_application_uses_tiao_polyphonic_reading() {
    let e = entry("乐器调音", "application", r"C:\Apps");
    for query in ["yueqitiaoyin", "yqty"] {
        assert_eq!(baseline::match_tier(&e, &crate::query::parse(query, "apps").unwrap()), None);
        assert_eq!(match_tier(&e, &crate::query::parse(query, "apps").unwrap()), Some(3), "{query}");
    }
}

#[test]
fn derived_app_keys_follow_content_and_alias_changes_immediately() {
    let mut e = entry("微信内容变更", "application", r"C:\Apps");
    let old = crate::query::parse("weixin", "apps").unwrap();
    assert_eq!(match_tier(&e, &old), Some(3));
    e.title = "钉钉内容变更".into();
    assert_eq!(match_tier(&e, &old), None);
    assert_eq!(match_tier(&e, &crate::query::parse("dingding", "apps").unwrap()), Some(3));
    e.aliases = vec!["chat-alias".into()];
    let alias = crate::query::parse("chat-alias", "apps").unwrap();
    assert_eq!(match_tier(&e, &alias), Some(0));
    e.aliases.clear();
    assert_eq!(match_tier(&e, &alias), None);
}

#[test]
fn app_key_cache_has_hard_capacity_and_excludes_oversized_content() {
    let mut cache = ApplicationKeyCache::default();
    let keys = Arc::new(ApplicationKeys { full: String::new(), initials: String::new(), acronym: String::new() });
    for i in 0..APPLICATION_KEY_CAPACITY + 20 {
        let title = format!("应用-{i}");
        cache.insert(&title, Arc::clone(&keys));
        assert!(cache.entries.len() <= APPLICATION_KEY_CAPACITY);
        assert_eq!(cache.entries.len(), cache.order.len());
    }
    assert!(!cache.entries.contains_key("应用-0"));
    let title = "超长标题".repeat(APPLICATION_KEY_MAX_BYTES);
    let first = application_keys(&title);
    let second = application_keys(&title);
    assert!(!Arc::ptr_eq(&first, &second));
    cache.insert(&title, first);
    assert_eq!(cache.entries.len(), APPLICATION_KEY_CAPACITY);
    assert!(!cache.entries.contains_key(&title));
}

#[test]
fn app_keys_are_shared_across_query_worker_threads() {
    let workers: Vec<_> = (0..8).map(|_| std::thread::spawn(|| application_keys("跨线程缓存验证应用"))).collect();
    let keys: Vec<_> = workers.into_iter().map(|worker| worker.join().unwrap()).collect();
    assert!(keys.iter().all(|key| Arc::ptr_eq(key, &keys[0])));
    assert_eq!(keys[0].full, "kuaxianchenghuancunyanzhengyingyong");
}

#[test]
fn repeated_application_queries_reuse_pinyin_conversion() {
    let e = entry("微信缓存回归独立标题", "application", r"C:\Apps");
    let spec = crate::query::parse("unmatchedprobe", "apps").unwrap();
    PINYIN_CONVERSIONS.with(|count| count.set(0));
    assert_eq!(match_tier(&e, &spec), None);
    assert_eq!(match_tier(&e, &spec), None);
    assert_eq!(PINYIN_CONVERSIONS.with(|count| count.get()), 1, "Repeated queries must reuse derived app keys");
}

#[test]
fn application_matching_workload_reports_repeated_query_cost() {
    // 固定合成目录覆盖直接、拼音、多词和无匹配；计时包含克隆与排序，避免只测缓存命中。
    let catalog: Vec<_> = (0..256).map(|i| {
        let (title, _, _, alias) = APPS[i % APPS.len()];
        let mut e = entry(&format!("{title} {i:03}"), "application", r"C:\Apps");
        e.aliases.push(format!("{alias}{i:03}"));
        e
    }).collect();
    let specs: Vec<_> = ["weixin", "cqyh", "yybfq", "studio missing", "noresult", "微信", "银行", "jsq"]
        .iter().map(|q| crate::query::parse(q, "apps").unwrap()).collect();
    let mut samples = [Vec::new(), Vec::new()];
    let mut count = [0, 0];
    for round in 0..5 {
        // 两种实现交替先后执行，使用同一进程和同一输入减少系统负载偏差。
        for implementation in if round % 2 == 0 { [0, 1] } else { [1, 0] } {
            let started = Instant::now();
            for _ in 0..10 {
                for spec in &specs {
                    let ranked = if implementation == 0 { baseline::rank_entries(catalog.clone(), spec, &HashMap::new(), &HashSet::new()) }
                        else { rank_entries(catalog.clone(), spec, &HashMap::new(), &HashSet::new()) };
                    count[implementation] += std::hint::black_box(ranked).len();
                }
            }
            samples[implementation].push(started.elapsed().as_secs_f64() * 1000.0);
        }
    }
    assert!(count[0] > 0);
    assert_eq!(count[0], count[1]);
    for (i, label) in ["baseline", "optimized"].into_iter().enumerate() {
        samples[i].sort_by(f64::total_cmp);
        println!("Synthetic app workload ({label}): 5 samples x 80 queries x 256 candidates, medianMs={:.3}, minMs={:.3}, maxMs={:.3}, checksum={}", samples[i][2], samples[i][0], samples[i][4], count[i]);
    }
}
