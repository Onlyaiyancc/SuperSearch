use super::*;

use super::experience_baseline as before;

fn app(title: &str) -> Entry {
    Entry { entity_id: title.into(), title: title.into(), kind: "application".into(),
        subtitle: String::new(), locator: format!(r"C:\Apps\{title}.exe"),
        launch_kind: "path".into(), aliases: vec![], launch_key: title.into() }
}

#[test]
fn complete_derived_keys_precede_partial_matches_without_history() {
    for (query, titles, expected) in [
        ("weixin", ["企业微信", "微信"], "微信"),
        ("wx", ["企业微信", "微信"], "微信"),
        ("wt", ["Microsoft PowerToys", "Windows Terminal"], "Windows Terminal"),
        ("weixin", ["微信助手", "微信"], "微信"),
    ] {
        let spec = crate::query::parse(query, "apps").unwrap();
        let entries: Vec<_> = titles.into_iter().map(app).collect();
        let ranked = rank_entries(entries, &spec, &HashMap::new(), &HashSet::new());
        assert_eq!(ranked[0].0.title, expected, "Complete key must win: {query}");
        assert_eq!(ranked.len(), 2, "Partial matches remain discoverable: {query}");
    }
}

#[test]
fn derived_key_precision_preserves_history_and_explicit_alias_policy() {
    let entries = vec![app("企业微信"), app("微信")];
    let usage = HashMap::from([("企业微信".into(), UsageStat { success_count: 1, last_success_at: u64::MAX })]);
    let spec = crate::query::parse("weixin", "apps").unwrap();
    assert_eq!(rank_entries(entries.clone(), &spec, &usage, &HashSet::new())[0].0.title, "企业微信");
    let mut aliased = app("聊天工具");
    aliased.aliases.push("weixin".into());
    let ranked = rank_entries(vec![app("微信"), aliased], &spec, &HashMap::new(), &HashSet::new());
    assert_eq!(ranked[0].0.title, "聊天工具");
    let pins = HashSet::from(["企业微信".into()]);
    assert_eq!(rank_entries(entries, &spec, &HashMap::new(), &pins)[0].0.title, "微信");
}

#[test]
fn precise_derived_matching_keeps_public_tiers_and_unicode_queries() {
    for (query, title, tier) in [("ＷＥＩＸＩＮ", "微信", 3), ("wx", "微信", 3),
        ("wt", "Windows Terminal", 4), ("studio code", "Visual Studio Code", 2)] {
        let spec = crate::query::parse(query, "apps").unwrap();
        assert_eq!(match_tier(&app(title), &spec), Some(tier));
        assert_eq!(before::match_tier(&app(title), &spec), Some(tier));
    }
}

#[test]
fn irrelevant_duplicates_do_not_mask_a_matching_candidate() {
    let mut unrelated = app("Unrelated label");
    unrelated.launch_key = "shared.exe".into();
    let mut matching = app("微信");
    matching.launch_key = "shared.exe".into();
    let ranked = rank_entries(vec![unrelated, matching], &crate::query::parse("微信", "apps").unwrap(), &HashMap::new(), &HashSet::new());
    assert_eq!(ranked.len(), 1);
    assert_eq!(ranked[0].0.title, "微信");
}

#[test]
fn release_same_input_comparison_reports_matching_workloads() {
    let apps: Vec<_> = (0..256).map(|index| app(&format!("{} {index:03}", ["微信", "企业微信", "重庆银行", "音乐播放器"][index % 4]))).collect();
    let files: Vec<_> = (0..1000).map(|index| {
        let mut entry = app(&format!("项目报告-{index:04}.pdf"));
        entry.kind = "file".into();
        entry.locator = format!(r"D:\工作\项目\{}", entry.title);
        entry
    }).collect();
    for (label, catalog, queries, scope) in [
        ("apps", apps.clone(), vec!["weixin", "wx", "chongqingyinhang", "yybfq", "noresult", "微信"], "apps"),
        ("files", files, vec!["项目", "报告", "报告 missing", "0001", "不存在", "ext:pdf"], "files"),
        ("excluded-apps", apps, vec!["微信", "wx", "报告", "noresult", "重庆", "音乐"], "files"),
    ] {
        let specs: Vec<_> = queries.iter().map(|q| crate::query::parse(q, scope).unwrap()).collect();
        let mut samples = [Vec::new(), Vec::new()];
        let mut totals = [0usize, 0];
        for round in 0..7 {
            for implementation in if round % 2 == 0 { [0, 1] } else { [1, 0] } {
                let started = std::time::Instant::now();
                for _ in 0..10 {
                    for spec in &specs {
                        let ranked = if implementation == 0 { before::rank_entries(catalog.clone(), spec, &HashMap::new(), &HashSet::new()) }
                            else { rank_entries(catalog.clone(), spec, &HashMap::new(), &HashSet::new()) };
                        totals[implementation] += std::hint::black_box(ranked).len();
                    }
                }
                samples[implementation].push(started.elapsed().as_secs_f64() * 1000.0);
            }
        }
        assert_eq!(totals[0], totals[1], "Candidate counts changed: {label}");
        for (index, name) in ["before", "after"].into_iter().enumerate() {
            samples[index].sort_by(f64::total_cmp);
            println!("Experience workload {label} ({name}): samples=7 queriesPerSample=60 candidates={} medianMs={:.3} minMs={:.3} maxMs={:.3} resultCount={}", catalog.len(), samples[index][3], samples[index][0], samples[index][6], totals[index]);
        }
    }
}
