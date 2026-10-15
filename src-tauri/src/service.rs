use crate::{everything::EverythingClient, model::*, platform, query, ranking, session::RequestGate, store::Store};
use serde_json::{json, Value};
use std::{collections::{HashMap, HashSet}, path::Path, sync::{atomic::{AtomicBool, Ordering}, Mutex}, time::{Instant, SystemTime, UNIX_EPOCH}};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::GlobalShortcutExt;

#[derive(Default)]
pub struct SearchCache {
    gate: RequestGate,
    request_id: String,
    generation: u64,
    revision: u64,
    spec: Option<query::QuerySpec>,
    folder_prefix: Option<query::FolderPrefix>,
    folder_completions: Vec<FolderCompletion>,
    entries: Vec<Entry>,
    results: HashMap<String, Entry>,
    result_ids: HashMap<String, String>,
    providers: Vec<ProviderStatus>,
    offset: u32,
    total: u32,
    loading_more: bool,
}

pub struct AppState {
    pub store: Mutex<Store>,
    pub cache: Mutex<SearchCache>,
    pub everything: EverythingClient,
    pub refreshing: AtomicBool,
    pub apps_error: Mutex<Option<String>>,
    pub shortcut_error: Mutex<Option<String>>,
    pub last_refresh: Mutex<Instant>,
    pub window_position_initialized: AtomicBool,
}

fn lock<T>(mutex: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>, String> {
    mutex.lock().map_err(|_| "Application state is unavailable. Restart SuperSearch.".into())
}

fn apps_provider_status(refreshing: bool, error: Option<&str>, count: usize) -> ProviderStatus {
    let (status, message) = if refreshing {
        ("loading", format!("Refreshing applications; {count} applications cached."))
    } else if let Some(error) = error {
        ("unavailable", format!("Application refresh failed: {error}. {count} applications retained. Try Refresh apps in Settings."))
    } else {
        ("ready", format!("{count} applications"))
    };
    ProviderStatus { id: "apps".into(), status: status.into(), message }
}

pub fn invalidate(app: &AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut cache) = state.cache.lock() {
            cache.gate.invalidate();
            cache.entries.clear();
            cache.results.clear();
            cache.spec = None;
            cache.folder_prefix = None;
            cache.folder_completions.clear();
            cache.loading_more = false;
        }
    }
}

fn apply_layout(window: &tauri::WebviewWindow, layout: crate::layout::WindowLayout, rows: u32, initial_position: bool) -> Result<(), String> {
    let cursor_monitor = if initial_position {
        unsafe {
            let mut point = windows::Win32::Foundation::POINT::default();
            if windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut point).is_ok() {
                window.monitor_from_point(point.x as f64, point.y as f64).ok().flatten()
            } else { None }
        }
    } else { None };
    let monitor = cursor_monitor.or_else(|| window.current_monitor().ok().flatten()).or_else(|| window.primary_monitor().ok().flatten()).ok_or("No monitor is available")?;
    let scale = monitor.scale_factor();
    let work = monitor.work_area();
    let anchor = if initial_position { None } else { window.outer_position().ok() };
    let width = if initial_position { 680.0 } else { window.inner_size().map(|size| size.width as f64 / window.scale_factor().unwrap_or(scale)).unwrap_or(680.0) };
    let geometry = crate::layout::fit(layout, rows, width, work, scale, anchor);
    // 物理尺寸按目标屏幕 DPI 计算，跨屏移动时不依赖尚未更新的旧窗口 DPI。
    window.set_position(geometry.position).map_err(|e| e.to_string())?;
    window.set_min_size(Some(tauri::PhysicalSize::new((360.0*scale).round().min(work.size.width as f64) as u32, (76.0*scale).round().min(work.size.height as f64) as u32))).map_err(|e| e.to_string())?;
    window.set_size(geometry.size).map_err(|e| e.to_string())?;
    window.set_position(geometry.position).map_err(|e| e.to_string())
}

fn may_expand(cache: &SearchCache) -> bool {
    cache.gate.current(&cache.request_id, cache.generation) && cache.spec.as_ref().is_some_and(|spec| !blank_query(spec)) && (!cache.results.is_empty() || !cache.folder_completions.is_empty())
}

fn results_layout_rows(cache: &SearchCache, rows: u32, request_id: Option<&str>) -> Option<u32> {
    (request_id == Some(cache.request_id.as_str()) && may_expand(cache)).then_some(rows.min(6).min((cache.results.len() + cache.folder_completions.len()) as u32))
}

#[tauri::command]
pub fn sync_window_layout(app: AppHandle, layout: crate::layout::WindowLayout, rows: u32, request_id: Option<String>) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("Search window is unavailable")?;
    if !window.is_visible().unwrap_or(false) { return Ok(()); }
    let state = app.state::<AppState>();
    let rows = if matches!(layout, crate::layout::WindowLayout::Results) {
        let cache = lock(&state.cache)?;
        let Some(rows) = results_layout_rows(&cache, rows, request_id.as_deref()) else { return Ok(()); };
        rows
    } else { rows };
    apply_layout(&window, layout, rows, false)
}

pub fn show(app: &AppHandle, settings: bool) {
    // 原生唤起先使旧搜索失效并收起，再显示；迟到结果不能恢复旧面板。
    invalidate(app);
    if let Some(window) = app.get_webview_window("main") {
        let layout = if settings { crate::layout::WindowLayout::Settings } else { crate::layout::WindowLayout::Compact };
        let state = app.state::<AppState>();
        // 同一原生窗口只在首次成功定位时使用默认位置；后续呼出保留拖动后的坐标。
        let initial_position = !state.window_position_initialized.load(Ordering::Relaxed);
        if apply_layout(&window, layout, 0, initial_position).is_ok() {
            state.window_position_initialized.store(true, Ordering::Relaxed);
        }
        let _ = window.show();
        // 长时间诊断保持可见但不反复抢占用户焦点；日常呼出和短基准仍正常聚焦。
        let soak = app.try_state::<crate::ui_benchmark::BenchmarkState>().is_some_and(|state| state.options.soak_seconds.is_some());
        if !soak { let _ = window.set_focus(); }
        let _ = window.emit(if settings { "settings_requested" } else { "show_search" }, ());
    }
    if let Some(state) = app.try_state::<AppState>() {
        let due = state.last_refresh.lock().map(|t| t.elapsed().as_secs() > 300).unwrap_or(false);
        if due { refresh_in_background(app.clone()); }
    }
}

pub fn toggle(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            invalidate(app);
            let _ = window.hide();
            let _ = window.emit("hide_search", ());
        } else { show(app, false); }
    }
}

fn snapshot(cache: &mut SearchCache, store: &Store, complete: bool) -> Result<SearchSnapshot, String> {
    let pins: HashSet<_> = store.pinned_entries()?.iter().map(|e| e.entity_id.clone()).collect();
    let spec = cache.spec.as_ref().ok_or("No current search")?;
    let settings=store.settings();
    let empty = blank_query(spec);
    // 新文件页已由 Everything 做属性筛选；只有少量固定/最近项需要检查本地元数据。
    let entries=if empty { vec![] } else { cache.entries.clone() };
    let usage = if settings.record_history { store.usage()? } else { HashMap::new() };
    let ranked = ranking::rank_entries(entries, spec, &usage, &pins);
    let limit = 50;
    let truncated = ranked.len() > limit || cache.total > cache.offset;
    cache.revision += 1;
    cache.results.clear();
    let items = ranked.into_iter().take(limit).map(|(entry, tier)| {
        // ID 绑定实体而非行号；迟到来源重新排序时旧按钮不会指向其他目标。
        let next_id = format!("r-{}-{}", cache.generation, cache.result_ids.len());
        let id = cache.result_ids.entry(entry.entity_id.clone()).or_insert(next_id).clone();
        let available = !empty || entry.launch_kind == "shell" || Path::new(&entry.locator).exists();
        let mut actions = vec!["pin".into()];
        if available { actions.insert(0, "open".into()); }
        if entry.launch_kind != "shell" { actions.extend(["reveal".into(), "copyPath".into()]); }
        let subtitle = if matches!(entry.kind.as_str(), "file" | "folder") { Path::new(&entry.locator).parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| entry.subtitle.clone()) } else { entry.subtitle.clone() };
        let path = if matches!(entry.kind.as_str(), "file" | "folder") { entry.locator.clone() } else { String::new() };
        let item = ResultItem { result_id: id.clone(), entity_id: entry.entity_id.clone(), kind: entry.kind.clone(), title: entry.title.clone(), subtitle, path, match_tier: tier, pinned: pins.contains(&entry.entity_id), available, actions };
        cache.results.insert(id, entry);
        item
    }).collect();
    let current = cache.gate.current(&cache.request_id, cache.generation);
    Ok(SearchSnapshot { request_id: cache.request_id.clone(), snapshot_revision: cache.revision, scope: spec.scope.clone(), terms: if current { spec.terms.clone() } else { vec![] }, directory: if current { spec.directory.clone() } else { None }, folder_completions: if current { cache.folder_completions.clone() } else { vec![] }, complete, items, providers: cache.providers.clone(), truncated, has_more: cache.offset < cache.total && cache.offset < 1000 })
}

fn blank_query(spec: &query::QuerySpec) -> bool {
    spec.terms.is_empty() && spec.extensions.is_empty() && spec.directory.is_none()
}

fn initial_entries(store: &Store, spec: &query::QuerySpec) -> Result<Vec<Entry>, String> {
    if blank_query(spec) { return Ok(vec![]); }
    let settings = store.settings();
    let pinned = store.pinned_entries()?;
    let pins = pinned.iter().map(|e| e.entity_id.clone()).collect();
    let usage = if settings.record_history { store.usage()? } else { HashMap::new() };
    let mut entries: Vec<_> = ranking::rank_entries(store.apps(), spec, &usage, &pins).into_iter().take(50).map(|(e,_)|e).collect();
    if spec.scope != "apps" {
        let mut historical: Vec<_> = pinned.into_iter().filter(|e| e.kind != "application").collect();
        if settings.record_history {
            historical.extend(store.usage_entries()?.into_iter().filter(|(e,s)| e.kind != "application" && s.success_count > 0).map(|(e,_)|e));
        }
        // 先按查询、目录和历史排序，再检查最多 50 项，避免逐个访问整份历史的文件系统。
        historical.retain(|e| visible_directory(e, &settings));
        entries.extend(ranking::rank_entries(historical, spec, &usage, &pins).into_iter().take(50).filter_map(|(e,_)| {
            let metadata = std::fs::metadata(&e.locator).ok()?;
            if (e.kind == "file" && !metadata.is_file()) || (e.kind == "folder" && !metadata.is_dir()) || !visible_attributes(&metadata, &settings) { return None; }
            Some(e)
        }));
    }
    Ok(entries)
}

fn visible_directory(entry: &Entry, settings: &Settings) -> bool {
    if entry.kind == "application" { return true; }
    let locator=entry.locator.replace('/',"\\").to_lowercase();
    let inside=|directory:&String| {
        let path=directory.replace('/',"\\").trim_end_matches('\\').to_lowercase();
        locator.starts_with(&format!("{path}\\"))
    };
    if !settings.include_directories.is_empty() && !settings.include_directories.iter().any(inside) { return false; }
    if settings.exclude_directories.iter().any(inside) { return false; }
    true
}

fn visible_attributes(metadata: &std::fs::Metadata, settings: &Settings) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        let attributes=metadata.file_attributes();
        if (settings.hide_hidden && attributes & 2 != 0) || (settings.hide_system && attributes & 4 != 0) { return false; }
    }
    true
}

#[cfg(test)]
fn visible_with_settings(entry: &Entry, settings: &Settings) -> bool {
    visible_directory(entry, settings) && (entry.kind == "application" || std::fs::metadata(&entry.locator).map(|m| visible_attributes(&m,settings)).unwrap_or(true))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn provider_probe_does_not_execute_blocking_ipc_on_its_calling_thread() {
        let caller = std::thread::current().id();
        let status = tauri::async_runtime::block_on(run_provider_probe(move || {
            assert_ne!(std::thread::current().id(), caller, "Blocking provider IPC must leave the command caller thread");
            Ok(vec![apps_provider_status(false, None, 3)])
        })).unwrap();
        assert_eq!(status[0].status, "ready");
        assert!(status[0].message.contains("3 applications"));
        assert_eq!(tauri::async_runtime::block_on(run_provider_probe(|| Err("Probe failed".into()))).unwrap_err(), "Probe failed");
    }
    use std::{cell::RefCell, fs, path::PathBuf, sync::atomic::AtomicU64};
    static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!("supersearch-service-{}-{}", std::process::id(), FIXTURE_ID.fetch_add(1, Ordering::Relaxed))))
        }
        fn store(&self) -> Store { Store::new(&self.0).unwrap() }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            // 清理前确认测试目录仍在指定临时目录下。
            assert!(self.0.starts_with(std::env::temp_dir()));
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn app_entry(id: &str) -> Entry {
        Entry { entity_id: id.into(), kind: "application".into(), title: id.into(), subtitle: String::new(), locator: format!(r"C:\Apps\{id}.exe"), launch_kind: "path".into(), aliases: vec![], launch_key: id.into() }
    }
    fn search_cache(raw: &str, scope: &str, entries: Vec<Entry>) -> SearchCache {
        SearchCache { spec: Some(query::parse(raw, scope).unwrap()), entries, ..SearchCache::default() }
    }
    #[test]
    fn snapshot_exposes_parsed_directory_and_current_candidates_without_file_results() {
        let fixture = Fixture::new();
        let store = fixture.store();
        let mut cache = search_cache("项目/子", "all", vec![]);
        cache.request_id = "current".into();
        cache.generation = cache.gate.begin("current").unwrap();
        cache.folder_completions = vec![FolderCompletion { title: "子目录".into(), path: r"D:\项目\子目录".into(), query: "D:\\项目\\子目录\\".into() }];
        let result = snapshot(&mut cache, &store, true).unwrap();
        assert_eq!(result.directory.as_deref(), Some(r"\项目"));
        assert_eq!(result.terms, ["子"]);
        assert_eq!(result.folder_completions.len(), 1);
        assert!(result.items.is_empty());
        assert_eq!(results_layout_rows(&cache, 6, Some("current")), Some(1));
        assert_eq!(results_layout_rows(&cache, 6, Some("previous")), None);
        cache.gate.invalidate();
        assert_eq!(results_layout_rows(&cache, 6, Some("current")), None);
        let result = snapshot(&mut cache, &store, true).unwrap();
        assert!(result.directory.is_none() && result.folder_completions.is_empty());
        assert!(result.terms.is_empty());
    }
    fn page(entries: Vec<Entry>) -> crate::everything::FilePage {
        crate::everything::FilePage { total: entries.len() as u32, entries, status: ProviderStatus { id: "everything".into(), status: "ready".into(), message: "Fixture index is ready".into() } }
    }
    #[test]
    fn file_query_submission_is_atomic_but_waiting_allows_new_input() {
        let mut initial = search_cache("report", "files", vec![]);
        initial.request_id = "current".into();
        initial.generation = initial.gate.begin("current").unwrap();
        let generation = initial.generation;
        let cache = Mutex::new(initial);
        let reply = query_current(&cache, "current", generation, || {
            assert!(cache.try_lock().is_err(), "Submission must protect the request generation");
            Ok(())
        }, |_| {
            let mut cache = cache.try_lock().expect("Reply waiting must release the search lock");
            cache.gate.begin("newer").unwrap();
            Ok(page(vec![file(r"D:\Work\report.txt", "file")]))
        }).unwrap();
        let mut cache = cache.lock().unwrap();
        assert!(!apply_file_page(&mut cache, "current", generation, 0, reply));
        assert!(cache.entries.is_empty());
    }

    #[test]
    fn stale_file_queries_never_replace_current_sdk_work() {
        let cache = Mutex::new(search_cache("report", "files", vec![]));
        assert!(query_current(&cache, "stale", 1, || -> Result<(), String> { panic!("Stale query must not submit") }, |_| -> Result<crate::everything::FilePage, String> { panic!("Stale query must not wait") }).is_none());
    }

    #[test]
    fn file_page_preserves_main_results_when_completion_fails_and_skips_stale_work() {
        let mut cache = search_cache(r"D:\项目\子", "files", vec![]);
        cache.request_id = "current".into();
        cache.generation = cache.gate.begin("current").unwrap();
        cache.folder_prefix = query::folder_prefix(r"D:\项目\子");
        let generation = cache.generation;
        assert!(apply_file_page(&mut cache, "current", generation, 0, Ok(page(vec![file(r"D:\项目\子报告.txt", "file")]))));
        apply_folder_page(&mut cache, "current", generation, &Settings::default(), Err("Completion failed".into()));
        assert_eq!(cache.entries.len(), 1);
        assert_eq!(cache.providers[0].status, "ready");
        assert!(cache.folder_completions.is_empty());
        cache.gate.begin("newer").unwrap();
        assert!(!apply_file_page(&mut cache, "current", generation, 0, Ok(page(vec![]))));
        apply_folder_page(&mut cache, "current", generation, &Settings::default(), Ok(page(vec![file(r"D:\项目\子目录", "folder")])));
        assert_eq!(cache.entries.len(), 1);
        assert!(cache.folder_completions.is_empty());
    }
    #[test]
    fn completion_success_and_pagination_keep_candidate_budget() {
        let mut cache = search_cache(r"D:\项目\子", "files", vec![]);
        cache.request_id = "current".into();
        cache.generation = cache.gate.begin("current").unwrap();
        cache.folder_prefix = query::folder_prefix(r"D:\项目\子");
        let generation = cache.generation;
        apply_file_page(&mut cache, "current", generation, 0, Ok(page(vec![])));
        apply_folder_page(&mut cache, "current", generation, &Settings::default(), Ok(page(vec![file(r"D:\项目\子目录", "folder")])));
        assert_eq!(cache.folder_completions.len(), 1);
        apply_file_page(&mut cache, "current", generation, 1, Ok(page(vec![])));
        assert_eq!(cache.folder_completions.len(), 1);
        apply_file_page(&mut cache, "current", generation, 0, Ok(page(vec![])));
        assert!(cache.folder_completions.is_empty());
        cache.folder_completions = (0..5).map(|i| FolderCompletion { title: format!("子{i}"), path: format!(r"D:\项目\子{i}"), query: format!("D:\\项目\\子{i}\\") }).collect();
        cache.results = (0..50).map(|i| (i.to_string(), app_entry(&i.to_string()))).collect();
        assert_eq!(results_layout_rows(&cache, 100, Some("current")), Some(6));
    }
    #[test]
    fn historical_file_snapshot_displays_actual_parent_directory() {
        let fixture = Fixture::new();
        let store = fixture.store();
        let mut entry = file(r"D:\项目\报告.pdf", "file");
        entry.title = "报告.pdf".into();
        entry.subtitle = "An old display label".into();
        let mut cache = search_cache("报告", "files", vec![entry]);
        let result = snapshot(&mut cache, &store, true).unwrap();
        assert_eq!(result.items[0].subtitle, r"D:\项目");
        assert_eq!(result.items[0].path, r"D:\项目\报告.pdf");
    }
    #[test]
    fn exact_supplement_survives_main_failure_and_keeps_main_pagination_counts() {
        let fixture = Fixture::new();
        let store = fixture.store();
        let mut cache = search_cache("report", "files", vec![]);
        cache.request_id = "current".into();
        cache.generation = cache.gate.begin("current").unwrap();
        let generation = cache.generation;
        let mut exact = file(r"D:\项目\report.txt", "file");
        exact.title = "report.txt".into();
        apply_exact_candidates(&mut cache, "current", generation, &Settings::default(), Ok(page(vec![exact.clone(), exact.clone()])));
        apply_file_page(&mut cache, "current", generation, 0, Err("Main failed".into()));
        assert_eq!(snapshot(&mut cache, &store, true).unwrap().items[0].title, "report.txt");
        assert_eq!((cache.offset, cache.total), (0, 0));
        let mut main_entries: Vec<_> = (0..200).map(|i| { let mut e = file(&format!(r"D:\项目\{i:03}-report.txt"), "file"); e.entity_id = e.locator.clone(); e.title = format!("{i:03}-report.txt"); e }).collect();
        main_entries.push(exact);
        let main_page = crate::everything::FilePage { entries: main_entries, total: 241, status: page(vec![]).status };
        apply_file_page(&mut cache, "current", generation, 0, Ok(main_page));
        assert_eq!((cache.offset, cache.total), (201, 241));
        let result = snapshot(&mut cache, &store, true).unwrap();
        assert_eq!(result.items[0].title, "report.txt");
        assert_eq!(result.items.iter().filter(|e| e.title == "report.txt").count(), 1);
        cache.gate.begin("newer").unwrap();
        apply_exact_candidates(&mut cache, "current", generation, &Settings::default(), Ok(page(vec![file(r"D:\项目\report-other.txt", "file")])));
        assert_eq!(cache.entries.len(), 201);
    }
    #[test]
    fn initial_window_is_compact_with_small_screen_minimum() {
        let config: Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let window = &config["app"]["windows"][0];
        assert_eq!(window["height"], 76);
        assert_eq!(window["minHeight"], 76);
        assert_eq!(window["width"], 680);
        assert_eq!(window["minWidth"], 360);
        assert_eq!(window["transparent"], true);
        assert_eq!(window["decorations"], false);
        assert_eq!(window["shadow"], false);
        assert_eq!(window["resizable"], false);
    }
    #[test]
    fn blank_search_has_no_candidates_in_every_scope() {
        let fixture = Fixture::new();
        let store = fixture.store();
        for scope in ["all", "apps", "files", "folders"] {
            let mut e = app_entry("retained");
            if scope == "files" { e.kind = "file".into(); }
            if scope == "folders" { e.kind = "folder".into(); }
            let mut cache = search_cache("   ", scope, vec![e]);
            assert!(snapshot(&mut cache, &store, true).unwrap().items.is_empty(), "{scope}");
        }
    }
    #[test]
    fn all_scope_does_not_promote_unused_apps_over_successful_files() {
        let fixture = Fixture::new();
        let mut store = fixture.store();
        let mut entries = vec![app_entry("Report")];
        for index in 0..4 {
            let mut e = app_entry(&format!("Report Notes {index}"));
            e.kind = "file".into();
            store.record_open(&e).unwrap();
            entries.push(e);
        }
        let mut cache = search_cache("report", "all", entries);
        let items = snapshot(&mut cache, &store, true).unwrap().items;
        assert!(items.iter().take(4).all(|e| e.kind == "file"));
    }
    #[test]
    fn disabling_history_stops_old_usage_influencing_rank() {
        let fixture = Fixture::new();
        let mut store = fixture.store();
        let unused = app_entry("Report A");
        let used = app_entry("Report Z");
        store.record_open(&used).unwrap();
        store.save_settings(store.settings().settings_version, json!({"recordHistory":false})).unwrap();
        let mut cache = search_cache("report", "apps", vec![used, unused]);
        assert_eq!(snapshot(&mut cache, &store, true).unwrap().items[0].title, "Report A");
    }
    fn fixture_file(fixture: &Fixture, title: &str) -> Entry {
        let locator = fixture.0.join(title);
        fs::create_dir_all(&fixture.0).unwrap();
        fs::write(&locator, "fixture").unwrap();
        Entry { entity_id:title.into(), kind:"file".into(), title:title.into(), subtitle:String::new(), locator:locator.to_string_lossy().into(), launch_kind:"path".into(), aliases:vec![], launch_key:title.into() }
    }
    #[test]
    fn historical_file_is_seeded_before_engine_results_and_deduplicated() {
        let fixture = Fixture::new();
        let mut store = fixture.store();
        let used = fixture_file(&fixture, "Report Notes.txt");
        store.record_open(&used).unwrap();
        let spec = query::parse("report", "files").unwrap();
        let entries = initial_entries(&store, &spec).unwrap();
        assert_eq!(entries, [used.clone()]);
        let mut cache = search_cache("report", "files", entries);
        cache.entries.push(used);
        assert_eq!(snapshot(&mut cache, &store, true).unwrap().items.len(), 1);
    }
    #[test]
    fn historical_seed_obeys_scope_filters_privacy_deletion_and_history_toggle() {
        let fixture = Fixture::new();
        let mut store = fixture.store();
        let used = fixture_file(&fixture, "Report Notes.txt");
        let removed = app_entry("Report removed");
        store.replace_discovered_apps(vec![removed.clone()]).unwrap();
        store.record_open(&used).unwrap();
        store.record_open(&removed).unwrap();
        store.replace_discovered_apps(vec![]).unwrap();
        for (raw, scope) in [("report", "apps"), ("report", "folders"), ("report missing", "files"), ("report ext:pdf", "files")] {
            assert!(initial_entries(&store, &query::parse(raw, scope).unwrap()).unwrap().is_empty());
        }
        let directory = fixture.0.to_string_lossy().replace('/', "\\");
        let query = query::parse(&format!("report ext:txt in:\"{directory}\""), "files").unwrap();
        assert_eq!(initial_entries(&store, &query).unwrap(), [used.clone()]);
        store.save_settings(store.settings().settings_version, json!({"excludeDirectories":[directory]})).unwrap();
        assert!(initial_entries(&store, &query).unwrap().is_empty());
        store.save_settings(store.settings().settings_version, json!({"excludeDirectories":[], "recordHistory":false})).unwrap();
        assert!(initial_entries(&store, &query).unwrap().is_empty());
        store.save_settings(store.settings().settings_version, json!({"recordHistory":true})).unwrap();
        fs::remove_file(used.locator).unwrap();
        assert!(initial_entries(&store, &query).unwrap().is_empty());
    }
    #[test]
    fn historical_seed_is_bounded_to_top_fifty_candidates() {
        let fixture = Fixture::new();
        let mut store = fixture.store();
        for index in 0..60 {
            let e = fixture_file(&fixture, &format!("Report {index:02}.txt"));
            store.record_open(&e).unwrap();
        }
        let entries = initial_entries(&store, &query::parse("report", "files").unwrap()).unwrap();
        assert_eq!(entries.len(),50);
        assert_eq!(entries[0].title,"Report 00.txt");
    }
    #[test]
    fn late_results_layout_cannot_expand_invalidated_or_blank_search() {
        let entry = app_entry("Report");
        let mut cache = search_cache("report", "all", vec![entry.clone()]);
        cache.request_id = "current".into();
        cache.generation = cache.gate.begin(&cache.request_id).unwrap();
        assert!(!may_expand(&cache));
        cache.results.insert("result".into(),entry);
        assert!(may_expand(&cache));
        cache.gate.invalidate();
        assert!(!may_expand(&cache));
        cache.request_id = "blank".into();
        cache.generation = cache.gate.begin(&cache.request_id).unwrap();
        cache.spec = Some(query::parse("  ","files").unwrap());
        assert!(!may_expand(&cache));
    }
    #[test]
    fn results_layout_ignores_previous_search_and_missing_request_id() {
        let entry = app_entry("Report");
        let mut cache = search_cache("report", "all", vec![entry.clone()]);
        cache.request_id = "first".into();
        cache.generation = cache.gate.begin(&cache.request_id).unwrap();
        cache.results.insert("result".into(),entry);
        assert_eq!(results_layout_rows(&cache,1,Some("first")),Some(1));
        cache.gate.invalidate();
        cache.request_id = "second".into();
        cache.generation = cache.gate.begin(&cache.request_id).unwrap();
        assert_eq!(results_layout_rows(&cache,6,Some("first")),None);
        assert_eq!(results_layout_rows(&cache,6,None),None);
    }
    #[test]
    fn results_layout_never_allocates_rows_beyond_current_result_count() {
        let entry = app_entry("Report");
        let mut cache = search_cache("report", "all", vec![entry.clone()]);
        cache.request_id = "current".into();
        cache.generation = cache.gate.begin(&cache.request_id).unwrap();
        cache.results.insert("result".into(),entry);
        assert_eq!(results_layout_rows(&cache,6,Some("current")),Some(1));
    }
    #[cfg(windows)]
    #[test]
    fn historical_seed_respects_hidden_and_system_metadata() {
        use windows::{core::PCWSTR, Win32::Storage::FileSystem::{SetFileAttributesW, FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_SYSTEM}};
        let fixture = Fixture::new();
        let mut store = fixture.store();
        let used = fixture_file(&fixture,"Report private.txt");
        store.record_open(&used).unwrap();
        let path: Vec<u16> = used.locator.encode_utf16().chain(Some(0)).collect();
        let spec = query::parse("report", "files").unwrap();
        for attribute in [FILE_ATTRIBUTE_HIDDEN,FILE_ATTRIBUTE_SYSTEM] {
            unsafe { SetFileAttributesW(PCWSTR(path.as_ptr()),attribute).unwrap(); }
            assert!(initial_entries(&store,&spec).unwrap().is_empty());
        }
        store.save_settings(store.settings().settings_version,json!({"hideHidden":false,"hideSystem":false})).unwrap();
        assert_eq!(initial_entries(&store,&spec).unwrap(),[used]);
        unsafe { SetFileAttributesW(PCWSTR(path.as_ptr()),FILE_ATTRIBUTE_NORMAL).unwrap(); }
    }
    #[test]
    fn startup_failure_restores_entire_settings_and_preserves_old_shortcut_and_data() {
        let fixture = Fixture::new();
        let mut store = fixture.store();
        let entry = app_entry("retained");
        store.add_manual_app(entry.clone()).unwrap();
        store.pin(&entry).unwrap();
        store.record_open(&entry).unwrap();
        let old = store.settings();
        let registered = RefCell::new(vec![old.shortcut.clone()]);
        let error = commit_settings(&mut store, old.settings_version,
            json!({"shortcut":"Control+Space","autostart":true,"theme":"dark","recordHistory":false,"includeDesktop":true,"hideHidden":false,"hideSystem":false,"includeDirectories":["C:\\Work"],"excludeDirectories":["C:\\Private"],"autoStartEverything":true}),
            |shortcut| { registered.borrow_mut().push(shortcut.into()); Ok(()) },
            |shortcut| registered.borrow_mut().retain(|s| s != shortcut),
            |_| Err("Startup denied".into())).unwrap_err();
        assert!(error.contains("Settings were restored"));
        let mut expected = old.clone();
        expected.settings_version += 2;
        assert_eq!(store.settings(), expected);
        assert_eq!(*registered.borrow(), [old.shortcut]);
        assert_eq!(store.apps(), [entry]);
        assert_eq!(store.pinned_entries().unwrap().len(), 1);
        assert_eq!(store.usage().unwrap()["retained"].success_count, 1);
        drop(store);
        assert_eq!(fixture.store().settings(), expected);
    }
    #[test]
    fn invalid_patch_has_no_shortcut_registration_side_effect() {
        let fixture = Fixture::new();
        let mut store = fixture.store();
        let old = store.settings();
        let registered = RefCell::new(Vec::<String>::new());
        assert!(commit_settings(&mut store, old.settings_version,
            json!({"shortcut":"Control+Space","theme":"invalid"}),
            |shortcut| { registered.borrow_mut().push(shortcut.into()); Ok(()) },
            |_| {}, |_| Ok(())).is_err());
        assert!(registered.borrow().is_empty());
        assert_eq!(store.settings(), old);
    }
    #[test]
    fn startup_failure_reports_failed_rollback_and_cleans_new_shortcut() {
        let fixture = Fixture::new();
        let mut store = fixture.store();
        let old = store.settings();
        let registered = RefCell::new(vec![old.shortcut.clone()]);
        let error = commit_settings(&mut store, old.settings_version,
            json!({"shortcut":"Control+Space","autostart":true,"theme":"dark"}),
            |shortcut| { registered.borrow_mut().push(shortcut.into()); Ok(()) },
            |shortcut| registered.borrow_mut().retain(|s| s != shortcut),
            |_| { fs::create_dir(fixture.0.join("config.json.tmp")).unwrap(); Err("Startup denied".into()) }).unwrap_err();
        assert!(error.contains("rollback failed"), "{error}");
        assert!(!error.contains("Settings were restored"));
        assert_eq!(*registered.borrow(), [old.shortcut]);
        assert_eq!(store.settings().settings_version, old.settings_version + 1);
    }
    #[test]
    fn refresh_reruns_discovery_when_desktop_setting_changes_during_scan() {
        let fixture = Fixture::new();
        let store = Mutex::new(fixture.store());
        let refreshing = AtomicBool::new(true);
        let error = Mutex::new(Some("Old failure".into()));
        let last = Mutex::new(Instant::now() - std::time::Duration::from_secs(600));
        let mut observed = Vec::new();
        refresh_catalog(&store, &refreshing, &error, &last, |desktop| {
            observed.push(desktop);
            if observed.len() == 1 {
                let mut store = store.lock().unwrap();
                let version = store.settings().settings_version;
                store.save_settings(version, json!({"includeDesktop":true})).unwrap();
                return Ok(vec![app_entry("stale")]);
            }
            assert!(desktop);
            Ok(vec![app_entry("desktop")])
        }).unwrap();
        assert_eq!(observed, [false, true]);
        assert_eq!(store.lock().unwrap().apps(), [app_entry("desktop")]);
        assert!(!refreshing.load(Ordering::Acquire));
        assert!(error.lock().unwrap().is_none());
        assert!(last.lock().unwrap().elapsed().as_secs() < 1);
    }
    #[test]
    fn refresh_failure_retains_catalog_error_and_retry_age() {
        let fixture = Fixture::new();
        let mut initial = fixture.store();
        initial.replace_discovered_apps(vec![app_entry("retained")]).unwrap();
        let store = Mutex::new(initial);
        let refreshing = AtomicBool::new(true);
        let error = Mutex::new(None);
        let previous = Instant::now() - std::time::Duration::from_secs(600);
        let last = Mutex::new(previous);
        let result = refresh_catalog(&store, &refreshing, &error, &last, |_| Err("Discovery denied".into()));
        assert_eq!(result, Err("Discovery denied".into()));
        assert_eq!(store.lock().unwrap().apps(), [app_entry("retained")]);
        assert!(!refreshing.load(Ordering::Acquire));
        assert_eq!(*error.lock().unwrap(), Some("Discovery denied".into()));
        assert_eq!(*last.lock().unwrap(), previous);
    }
    #[test]
    fn apps_provider_reports_ready_catalog() {
        let status = apps_provider_status(false, None, 4);
        assert_eq!(status.id, "apps");
        assert_eq!(status.status, "ready");
        assert_eq!(status.message, "4 applications");
    }
    #[test]
    fn apps_provider_reports_refresh_in_progress() {
        let status = apps_provider_status(true, Some("Old failure"), 4);
        assert_eq!(status.status, "loading");
        assert!(status.message.contains("4 applications"));
    }
    #[test]
    fn apps_provider_retains_discovery_failure_with_cached_apps() {
        let status = apps_provider_status(false, Some("Start Menu unavailable"), 4);
        assert_eq!(status.status, "unavailable");
        assert!(status.message.contains("Start Menu unavailable"));
        assert!(status.message.contains("4 applications"));
        let empty = apps_provider_status(false, Some("Start Menu unavailable"), 0);
        assert_eq!(empty.status, "unavailable");
        assert!(empty.message.contains("Start Menu unavailable"));
    }
    fn file(path:&str,kind:&str)->Entry { Entry {entity_id:"fixture".into(),kind:kind.into(),title:"fixture".into(),subtitle:String::new(),locator:path.into(),launch_kind:"path".into(),aliases:vec![],launch_key:"fixture".into()} }
    #[test]
    fn pinned_files_follow_settings_directory_boundaries_but_apps_do_not() {
        let mut settings=Settings::default();
        settings.include_directories=vec![r"C:\Work".into()];
        settings.exclude_directories=vec![r"C:\Work\Private".into()];
        assert!(visible_with_settings(&file(r"C:\Work\Report.txt","file"),&settings));
        assert!(!visible_with_settings(&file(r"C:\WorkBackup\Report.txt","file"),&settings));
        assert!(!visible_with_settings(&file(r"C:\Work\Private\Report.txt","file"),&settings));
        assert!(visible_with_settings(&file(r"C:\Work\PrivateBackup\Report.txt","file"),&settings));
        assert!(visible_with_settings(&file(r"C:\Other\App.lnk","application"),&settings));
    }
}

#[tauri::command]
pub fn start_search(app: AppHandle, request_id: String, raw_query: String, scope: String) -> Result<SearchSnapshot, String> {
    let accepted = Instant::now();
    let state = app.state::<AppState>();
    let spec = query::parse(&raw_query, &scope)?;
    let store = lock(&state.store)?;
    let settings = store.settings();
    let mut cache = lock(&state.cache)?;
    let generation = cache.gate.begin(&request_id)?;
    cache.request_id = request_id.clone();
    cache.generation = generation;
    cache.revision = 0;
    cache.spec = Some(spec.clone());
    cache.folder_prefix = if spec.scope != "apps" { query::folder_prefix(&raw_query) } else { None };
    cache.folder_completions.clear();
    cache.offset = 0;
    cache.total = 0;
    cache.loading_more = false;
    cache.entries.clear();
    cache.result_ids.clear();
    let empty = blank_query(&spec);
    let files = spec.scope != "apps" && !empty;
    cache.entries = initial_entries(&store, &spec)?;
    cache.providers = vec![apps_provider_status(state.refreshing.load(Ordering::Acquire), lock(&state.apps_error)?.as_deref(), store.apps().len())];
    let file_status = if files { ProviderStatus { id:"everything".into(),status:"loading".into(),message:"Searching file names…".into() } } else { state.everything.cached_status() };
    cache.providers.push(file_status);
    let shortcut_error = lock(&state.shortcut_error)?;
    cache.providers.push(ProviderStatus { id: "shortcut".into(), status: if shortcut_error.is_some() { "unavailable" } else { "ready" }.into(), message: shortcut_error.clone().unwrap_or_else(|| "Global shortcut is available.".into()) });
    drop(shortcut_error);
    let initial = snapshot(&mut cache, &store, !files)?;
    drop(cache);
    drop(store);
    if files {
        let expression = query::compile_everything(&spec, &settings);
        let request = request_id;
        tauri::async_runtime::spawn_blocking(move || finish_files(app, request, generation, expression, 0, accepted + std::time::Duration::from_millis(250)));
    }
    Ok(initial)
}

fn query_current<T>(
    cache: &Mutex<SearchCache>, request: &str, generation: u64,
    submit: impl FnOnce() -> Result<T, String>,
    wait: impl FnOnce(T) -> Result<crate::everything::FilePage, String>,
) -> Option<Result<crate::everything::FilePage, String>> {
    let pending = {
        let cache = cache.lock().ok()?;
        if !cache.gate.current(request, generation) { return None; }
        // 检查代次与邮箱提交共用短锁；等待任何主查询或补取回复时都释放状态锁。
        submit()
    };
    Some(pending.and_then(wait))
}

fn finish_files(app: AppHandle, request: String, generation: u64, expression: String, offset: u32, deadline: Instant) {
    let state = app.state::<AppState>();
    let Some(outcome) = query_current(&state.cache, &request, generation,
        || state.everything.submit_query(expression, offset, deadline), |pending| pending.wait()) else { return; };
    let (settings, exact, completion) = {
        let Ok(store) = state.store.lock() else { return; };
        let Ok(mut cache) = state.cache.lock() else { return; };
        if !cache.gate.current(&request, generation) { return; }
        let settings = store.settings();
        let exact = if offset == 0 && Instant::now() < deadline {
            outcome.as_ref().ok().filter(|page| page.total > page.entries.len() as u32).and_then(|page| {
                let spec = cache.spec.as_ref()?;
                (!page.entries.iter().any(|entry| entry.kind == "file" && ranking::match_tier(entry, spec) == Some(0)))
                    .then(|| query::compile_exact_candidates(spec, &settings)).flatten()
            })
        } else { None };
        let completion = if offset == 0 && outcome.is_ok() && Instant::now() < deadline {
            cache.folder_prefix.as_ref().map(|prefix| query::compile_folder_completion(prefix, &settings))
        } else { None };
        if !apply_file_page(&mut cache, &request, generation, offset, outcome) { return; }
        cache.loading_more = exact.is_some() || completion.is_some();
        // 先显示已获得的主结果，精确候选与目录补全稍后合并，不拖延首屏。
        if let Ok(result) = snapshot(&mut cache, &store, exact.is_none() && completion.is_none()) {
            let _ = app.emit("search_snapshot", result);
        }
        (settings, exact, completion)
    };
    if exact.is_none() && completion.is_none() { return; }
    if let Some(expression) = exact {
        let extra_deadline = deadline.min(Instant::now() + std::time::Duration::from_millis(120));
        let Some(outcome) = query_current(&state.cache, &request, generation,
            || state.everything.submit_query(expression, 0, extra_deadline), |pending| pending.wait()) else { return; };
        let Ok(mut cache) = state.cache.lock() else { return; };
        apply_exact_candidates(&mut cache, &request, generation, &settings, outcome);
    }
    if let Some(expression) = completion {
        let extra_deadline = deadline.min(Instant::now() + std::time::Duration::from_millis(60));
        let Some(outcome) = query_current(&state.cache, &request, generation,
            || state.everything.submit_query(expression, 0, extra_deadline), |pending| pending.wait()) else { return; };
        let Ok(mut cache) = state.cache.lock() else { return; };
        apply_folder_page(&mut cache, &request, generation, &settings, outcome);
    }
    let Ok(store) = state.store.lock() else { return; };
    let Ok(mut cache) = state.cache.lock() else { return; };
    if !cache.gate.current(&request, generation) { return; }
    cache.loading_more = false;
    if let Ok(result) = snapshot(&mut cache, &store, true) { let _ = app.emit("search_snapshot", result); }
}

fn merge_file_entries(cache: &mut SearchCache, entries: impl IntoIterator<Item = Entry>) {
    let mut count = cache.entries.iter().filter(|e| e.kind != "application").count();
    for entry in entries {
        if count >= 1000 { break; }
        if cache.entries.iter().any(|e| e.kind == entry.kind && e.locator.eq_ignore_ascii_case(&entry.locator)) { continue; }
        cache.entries.push(entry);
        count += 1;
    }
}

fn apply_exact_candidates(
    cache: &mut SearchCache, request: &str, generation: u64, settings: &Settings,
    outcome: Result<crate::everything::FilePage, String>,
) {
    if !cache.gate.current(request, generation) { return; }
    let Some(spec) = cache.spec.as_ref() else { return; };
    // 精确候选只取最多 50 项；正常文件页的 offset/total 始终由主查询维护。
    if let Ok(page) = outcome {
        let entries: Vec<_> = page.entries.into_iter().take(50).filter(|e| ranking::match_tier(e, spec) == Some(0) && visible_directory(e, settings)).collect();
        merge_file_entries(cache, entries);
    }
}

fn apply_folder_page(cache: &mut SearchCache, request: &str, generation: u64, settings: &Settings, outcome: Result<crate::everything::FilePage, String>) {
    if !cache.gate.current(request, generation) { return; }
    if let (Some(prefix), Ok(page)) = (&cache.folder_prefix, outcome) {
        cache.folder_completions = query::folder_completions(prefix, page.entries, settings);
    }
}

fn apply_file_page(
    cache: &mut SearchCache, request: &str, generation: u64, offset: u32,
    outcome: Result<crate::everything::FilePage, String>,
) -> bool {
    if !cache.gate.current(request, generation) { return false; }
    cache.loading_more = false;
    cache.providers.retain(|p| p.id != "everything");
    if offset == 0 { cache.folder_completions.clear(); }
    match outcome {
        Ok(page) => {
            cache.offset = offset + page.entries.len() as u32;
            cache.total = page.total;
            merge_file_entries(cache, page.entries);
            cache.providers.push(page.status);
        }
        Err(_) => {
            cache.providers.push(ProviderStatus { id:"everything".into(),status:"unavailable".into(),message:"File search took too long or is unavailable. Application search is still available.".into() });
        }
    }
    true
}

#[tauri::command]
pub fn cancel_search(state: State<AppState>, request_id: String) -> Result<(), String> {
    let mut cache = lock(&state.cache)?;
    let generation = cache.generation;
    let active = cache.gate.current(&request_id, generation);
    cache.gate.cancel(&request_id);
    if active {
        cache.results.clear();
        cache.entries.clear();
        cache.spec = None;
        cache.folder_prefix = None;
        cache.folder_completions.clear();
        cache.loading_more = false;
    }
    Ok(())
}

#[tauri::command]
pub fn load_more(app: AppHandle, request_id: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let store = lock(&state.store)?;
    let mut cache = lock(&state.cache)?;
    let generation = cache.generation;
    if !cache.gate.current(&request_id, generation) || cache.loading_more || cache.offset >= 1000 || cache.offset >= cache.total { return Err("No more results for this search".into()); }
    let spec = cache.spec.as_ref().ok_or("No active search")?;
    let expression = query::compile_everything(spec, &store.settings());
    let offset = cache.offset;
    cache.loading_more = true;
    drop(cache);
    drop(store);
    tauri::async_runtime::spawn_blocking(move || finish_files(app, request_id, generation, expression, offset, Instant::now() + std::time::Duration::from_millis(250)));
    Ok(())
}

fn result_entry(cache: &SearchCache, request: &str, result: &str) -> Result<Entry, String> {
    if !cache.gate.current(request, cache.generation) { return Err("Search is no longer current".into()); }
    cache.results.get(result).cloned().ok_or("This result is no longer available".into())
}

#[tauri::command]
pub fn execute_action(app: AppHandle, state: State<AppState>, request_id: String, result_id: String, action: String) -> Result<Value, String> {
    let mut store = lock(&state.store)?;
    let cache = lock(&state.cache)?;
    let entry = result_entry(&cache, &request_id, &result_id)?;
    match action.as_str() {
        "open" => platform::open_entry(&entry)?,
        "reveal" if entry.launch_kind != "shell" => platform::reveal_entry(&entry)?,
        "copyPath" if entry.launch_kind != "shell" => platform::copy_path(&entry)?,
        _ => return Err("Unsupported action".into()),
    }
    let history_saved = if action == "open" { store.record_open(&entry).is_ok() } else { true };
    drop(cache);
    drop(store);
    if action == "open" {
        invalidate(&app);
        if let Some(window) = app.get_webview_window("main") { let _ = window.hide(); }
        let _ = app.emit("hide_search", ());
    }
    Ok(json!({"accepted":true,"historySaved":history_saved}))
}

#[tauri::command]
pub fn set_pin(state: State<AppState>, request_id: String, result_id: String, enabled: bool) -> Result<(), String> {
    let mut store = lock(&state.store)?;
    let cache = lock(&state.cache)?;
    let entry = result_entry(&cache, &request_id, &result_id)?;
    if enabled { store.pin(&entry) } else { store.unpin(&entry.entity_id) }
}

#[tauri::command]
pub fn remove_pin(state: State<AppState>, pin_id: String) -> Result<(), String> { lock(&state.store)?.remove_pin(&pin_id) }

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Result<Settings, String> { Ok(lock(&state.store)?.settings()) }

#[tauri::command]
pub fn update_settings(app: AppHandle, state: State<AppState>, settings_version: u64, patch: Value) -> Result<Settings, String> {
    let mut store = lock(&state.store)?;
    let old = store.settings();
    // 可执行文件位置只能由原生文件选择器设置，普通前端补丁不能更换目标。
    if patch.get("everythingPath").is_some_and(|value| value.as_str() != Some(old.everything_path.as_str())) {
        return Err("Choose Everything.exe using the native file picker.".into());
    }
    let settings = commit_settings(&mut store, settings_version, patch,
        |shortcut| app.global_shortcut().register(shortcut).map_err(|_| "Shortcut is unavailable. Choose another shortcut.".to_string()),
        |shortcut| { let _ = app.global_shortcut().unregister(shortcut); },
        |enabled| (if enabled { app.autolaunch().enable() } else { app.autolaunch().disable() }).map_err(|_| "Unable to change Windows startup.".to_string()))?;
    drop(store);
    if settings.shortcut != old.shortcut { *lock(&state.shortcut_error)? = None; }
    if settings.include_directories != old.include_directories {
        // 设置已提交；索引线程异常由来源状态反馈，不能将成功保存误报为失败并遗留旧版本。
        let _ = state.everything.retry(settings.include_directories.clone());
    }
    invalidate(&app);
    if settings.include_desktop != old.include_desktop { refresh_in_background(app); }
    Ok(settings)
}

fn commit_settings(
    store: &mut Store, settings_version: u64, patch: Value,
    mut register: impl FnMut(&str) -> Result<(), String>,
    mut unregister: impl FnMut(&str),
    mut autostart: impl FnMut(bool) -> Result<(), String>,
) -> Result<Settings, String> {
    let old = store.settings();
    // 先验证完整补丁，再触发系统注册；同一存储锁保护提交与回滚。
    let preview = store.preview_settings(settings_version, patch.clone())?;
    let new_shortcut = preview.shortcut;
    let shortcut_changed = new_shortcut != old.shortcut;
    if shortcut_changed {
        register(new_shortcut.as_str())?;
    }
    let saved = store.save_settings(settings_version, patch);
    let settings = match saved {
        Ok(s) => s,
        Err(error) => {
            if shortcut_changed { unregister(new_shortcut.as_str()); }
            return Err(error);
        }
    };
    if settings.autostart != old.autostart {
        let outcome = autostart(settings.autostart);
        if outcome.is_err() {
            if shortcut_changed { unregister(new_shortcut.as_str()); }
            // 完整恢复旧配置，版本继续递增，拒绝旧页面后续保存。
            let mut rollback = serde_json::to_value(&old).map_err(|_| "Unable to change Windows startup; settings rollback failed: Cannot serialize settings".to_string())?;
            rollback.as_object_mut().ok_or("Unable to change Windows startup; settings rollback failed: Invalid settings")?.remove("settingsVersion");
            return match store.save_settings(settings.settings_version, rollback) {
                Ok(_) => Err("Unable to change Windows startup. Settings were restored.".into()),
                Err(error) => Err(format!("Unable to change Windows startup; settings rollback failed: {error}. Reload settings and try again.")),
            };
        }
    }
    if shortcut_changed { unregister(old.shortcut.as_str()); }
    Ok(settings)
}

pub fn refresh_in_background(app: AppHandle) {
    let state = app.state::<AppState>();
    if state.refreshing.swap(true, Ordering::AcqRel) { return; }
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let result = refresh_catalog(&state.store, &state.refreshing, &state.apps_error, &state.last_refresh, platform::discover_apps);
        let count = state.store.lock().map(|store| store.apps().len()).unwrap_or(0);
        let _ = app.emit("apps_status", apps_provider_status(false, result.as_ref().err().map(String::as_str), count));
    });
}

fn refresh_catalog(
    store: &Mutex<Store>, refreshing: &AtomicBool, apps_error: &Mutex<Option<String>>,
    last_refresh: &Mutex<Instant>, mut discover: impl FnMut(bool) -> Result<Vec<Entry>, String>,
) -> Result<(), String> {
    let mut include_desktop = match store.lock() {
        Ok(store) => store.settings().include_desktop,
        Err(_) => {
            let error = "Storage unavailable".to_string();
            if let Ok(mut status) = apps_error.lock() { *status = Some(error.clone()); }
            refreshing.store(false, Ordering::Release);
            return Err(error);
        }
    };
    loop {
        let discovered = discover(include_desktop);
        let mut store = match store.lock() {
            Ok(store) => store,
            Err(_) => {
                let error = "Storage unavailable".to_string();
                if let Ok(mut status) = apps_error.lock() { *status = Some(error.clone()); }
                refreshing.store(false, Ordering::Release);
                return Err(error);
            }
        };
        // 扫描期间设置改变则重扫；最终提交与清除刷新标记共用同一存储锁。
        let current = store.settings().include_desktop;
        if current != include_desktop {
            include_desktop = current;
            drop(store);
            continue;
        }
        let result = discovered.and_then(|entries| store.replace_discovered_apps(entries));
        // 失败时保留旧目录，并将错误保留到下一次成功刷新。
        if let Ok(mut error) = apps_error.lock() { *error = result.as_ref().err().cloned(); }
        if result.is_ok() { if let Ok(mut last) = last_refresh.lock() { *last = Instant::now(); } }
        refreshing.store(false, Ordering::Release);
        return result;
    }
}

#[tauri::command]
pub fn refresh_apps(app: AppHandle) { refresh_in_background(app); }

#[tauri::command]
pub async fn get_provider_status(app: AppHandle) -> Result<Vec<ProviderStatus>, String> {
    run_provider_probe(move || collect_provider_status(&app.state::<AppState>())).await
}

fn collect_provider_status(state: &AppState) -> Result<Vec<ProviderStatus>, String> {
    // 状态检查可能等待原生 IPC；等待期间不占用应用目录锁。
    let status = state.everything.status();
    let count = lock(&state.store)?.apps().len();
    let apps_status = apps_provider_status(state.refreshing.load(Ordering::Acquire), lock(&state.apps_error)?.as_deref(), count);
    let mut results = vec![apps_status,status];
    let shortcut_error = lock(&state.shortcut_error)?;
    results.push(ProviderStatus { id: "shortcut".into(), status: if shortcut_error.is_some() { "unavailable" } else { "ready" }.into(), message: shortcut_error.clone().unwrap_or_else(|| "Global shortcut is available.".into()) });
    Ok(results)
}

async fn run_provider_probe(probe: impl FnOnce() -> Result<Vec<ProviderStatus>, String> + Send + 'static) -> Result<Vec<ProviderStatus>, String> {
    // SDK 的有界同步等待交给阻塞线程池，呼出窗口时不会占用原生命令/UI 线程。
    tauri::async_runtime::spawn_blocking(probe).await.map_err(|_| "Provider status worker failed".to_string())?
}

#[tauri::command]
pub fn start_everything(state: State<AppState>) -> Result<(), String> {
    let scope = lock(&state.store)?.settings().include_directories;
    state.everything.retry(scope)
}

#[tauri::command]
pub fn choose_everything(state: State<AppState>) -> Result<Settings, String> {
    let Some(path) = platform::choose_file("everything")? else { return Ok(lock(&state.store)?.settings()); };
    if Path::new(&path).file_name().and_then(|s|s.to_str()).map(|s|s.eq_ignore_ascii_case("Everything.exe")) != Some(true) { return Err("Please select Everything.exe".into()); }
    let mut store = lock(&state.store)?;
    let version = store.settings().settings_version;
    store.save_settings(version,json!({"everythingPath":path}))
}

#[tauri::command]
pub fn add_app(state: State<AppState>, display_name: String, aliases: Vec<String>) -> Result<(), String> {
    let Some(path) = platform::choose_file("app")? else { return Ok(()); };
    let entry = platform::manual_app(path,display_name,aliases)?;
    lock(&state.store)?.add_manual_app(entry)
}

#[tauri::command]
pub fn set_app_aliases(state: State<AppState>, app_id: String, aliases: Vec<String>) -> Result<(), String> { lock(&state.store)?.set_aliases(&app_id,aliases) }

#[tauri::command]
pub fn clear_usage_history(state: State<AppState>) -> Result<Value, String> { Ok(json!({"deleted":lock(&state.store)?.clear_usage()?})) }

#[tauri::command]
pub fn search_web(app: AppHandle, raw_query: String) -> Result<(), String> {
    // 浏览器明确受理后再关闭搜索；失败保留输入与窗口，且不记录本地使用历史。
    platform::open_web_search(&raw_query)?;
    hide_window(app);
    Ok(())
}

#[tauri::command]
pub fn hide_window(app: AppHandle) {
    invalidate(&app);
    if let Some(window)=app.get_webview_window("main") {let _=window.hide();}
    let _=app.emit("hide_search",());
}

#[tauri::command]
pub fn get_diagnostics(state: State<AppState>) -> Result<Value, String> {
    Ok(json!({"application":"SuperSearch","version":env!("CARGO_PKG_VERSION"),"platform":"Windows x64","appCount":lock(&state.store)?.apps().len(),"providers":state.everything.status(),"engine":state.everything.diagnostics(),"telemetry":false,"queryLogging":false,"timestamp":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()}))
}
