use serde_json::{json, Value};
use std::{path::PathBuf, time::Instant};
use tauri::{AppHandle, Manager};

#[derive(Clone, Debug)]
pub struct BenchmarkOptions { pub soak_seconds: Option<u64>, pub fixture_files: usize }

impl BenchmarkOptions {
    pub fn parse(args: &[String]) -> Result<Option<Self>, String> {
        let soak = args.iter().any(|arg| arg == "--ui-soak");
        let benchmark = args.iter().any(|arg| arg == "--ui-benchmark");
        if soak && benchmark { return Err("Choose either --ui-soak or --ui-benchmark".into()); }
        let value = |flag: &str| -> Result<Option<u64>, String> {
            let positions = args.iter().enumerate().filter(|(_, arg)| arg.as_str() == flag).map(|(index, _)| index).collect::<Vec<_>>();
            if positions.len() > 1 { return Err(format!("{flag} must be provided once")); }
            positions.first().map(|index| args.get(index + 1).ok_or_else(|| format!("Missing value for {flag}"))?
                .parse::<u64>().map_err(|_| format!("{flag} requires a positive integer"))).transpose()
        };
        let seconds = value("--soak-seconds")?;
        let files = value("--benchmark-files")?;
        if !soak && seconds.is_some() { return Err("--soak-seconds requires --ui-soak".into()); }
        if !soak && !benchmark {
            return if files.is_some() { Err("--benchmark-files requires a UI diagnostic mode".into()) } else { Ok(None) };
        }
        let seconds = seconds.unwrap_or(1800);
        let files = files.unwrap_or(483);
        if soak && !(1..=3600).contains(&seconds) { return Err("--soak-seconds must be between 1 and 3600".into()); }
        if !(483..=1_000_000).contains(&files) { return Err("--benchmark-files must be between 483 and 1000000".into()); }
        Ok(Some(Self { soak_seconds: soak.then_some(seconds), fixture_files: files as usize }))
    }

    pub fn report_path(&self) -> &'static str {
        if self.soak_seconds.is_some() { "test-results/ui-native-soak.json" } else { "test-results/ui-native-benchmark.json" }
    }

    pub fn cold_timeout_ms(&self) -> u64 {
        if self.fixture_files > 10_000 { 600_000 } else { 120_000 }
    }
}

pub struct BenchmarkState { pub root: PathBuf, pub started: Instant, pub token: String, pub options: BenchmarkOptions }

#[cfg(test)]
mod tests {
    use super::*;
    fn options(args: &[&str]) -> Result<Option<BenchmarkOptions>, String> {
        BenchmarkOptions::parse(&args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>())
    }
    #[test]
    fn diagnostic_options_default_to_half_hour_and_existing_fixture_count() {
        let soak = options(&["--ui-soak"]).unwrap().unwrap();
        assert_eq!(soak.soak_seconds, Some(1800));
        assert_eq!(soak.fixture_files, 483);
        let quick = options(&["--ui-soak", "--soak-seconds", "2", "--benchmark-files", "100000"]).unwrap().unwrap();
        assert_eq!(quick.soak_seconds, Some(2));
        assert_eq!(quick.fixture_files, 100000);
        assert!(options(&["--show"]).unwrap().is_none());
    }
    #[test]
    fn diagnostic_options_reject_invalid_bounds_and_conflicting_modes() {
        for args in [vec!["--ui-soak", "--soak-seconds", "0"], vec!["--ui-soak", "--soak-seconds", "3601"],
            vec!["--ui-benchmark", "--soak-seconds", "2"], vec!["--soak-seconds", "2"],
            vec!["--ui-benchmark", "--ui-soak"], vec!["--ui-benchmark", "--benchmark-files", "482"],
            vec!["--ui-benchmark", "--benchmark-files", "1000001"], vec!["--ui-soak", "--benchmark-files"],
            vec!["--ui-soak", "--soak-seconds", "bad"]] {
            assert!(options(&args).is_err(), "Invalid options were accepted: {args:?}");
        }
    }
    #[test]
    fn benchmark_fixture_files_exist_before_the_indexer_starts() {
        let state = BenchmarkState::new().unwrap();
        let exists = state.root.join("fixtures/ranking").join(format!("{}-late-报告.txt", state.token)).is_file();
        assert!(state.root.starts_with(std::env::current_dir().unwrap().join(".tools/ui-benchmark")));
        std::fs::remove_dir_all(&state.root).unwrap();
        assert!(exists, "All benchmark files must exist before starting the engine");
    }
    #[test]
    fn benchmark_webview_uses_its_own_user_data_directory() {
        let state = BenchmarkState { root: PathBuf::from(r"C:\Workspace\.tools\benchmark"), started: Instant::now(), token: "fixture".into(), options: BenchmarkOptions { soak_seconds: None, fixture_files: 483 } };
        let mut config: tauri::Config = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        state.isolate_webview(&mut config);
        assert_eq!(config.app.app_directories_override, Some(tauri::utils::config::AppDirectoriesOverride::Root(state.root.join("webview"))));
    }
    #[test]
    fn soak_is_visible_without_accepting_native_focus_but_short_benchmark_keeps_focus() {
        let mut state = BenchmarkState { root: PathBuf::from(r"C:\Workspace\.tools\benchmark"), started: Instant::now(), token: "fixture".into(), options: BenchmarkOptions { soak_seconds: None, fixture_files: 483 } };
        let mut config: tauri::Config = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        state.isolate_webview(&mut config);
        assert!(config.app.windows[0].focusable);
        state.options.soak_seconds = Some(1800);
        state.isolate_webview(&mut config);
        assert!(!config.app.windows[0].focus);
        assert!(!config.app.windows[0].focusable);
    }
}

impl BenchmarkState {
    pub fn isolate_webview(&self, config: &mut tauri::Config) {
        // 默认 WebView2 路径随应用本地目录覆盖，避免复用日常程序的浏览器进程。
        config.app.app_directories_override = Some(tauri::utils::config::AppDirectoriesOverride::Root(self.root.join("webview")));
        if self.options.soak_seconds.is_some() {
            // 浸泡验收只观察可见性和尺寸，不接收用户键鼠或反复抢占前台。
            for window in &mut config.app.windows { window.focus = false; window.focusable = false; }
        }
    }

    #[cfg(test)]
    pub fn new() -> Result<Self, String> {
        Self::with_options(BenchmarkOptions { soak_seconds: None, fixture_files: 483 })
    }

    pub fn with_options(options: BenchmarkOptions) -> Result<Self, String> {
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos();
        let root = std::env::current_dir().map_err(|e| e.to_string())?.join(".tools/ui-benchmark").join(format!("{nonce} workspace"));
        std::fs::create_dir_all(root.join("fixtures")).map_err(|e| e.to_string())?;
        let state = Self { root, started: Instant::now(), token: format!("UI验收{nonce}"), options };
        // 完整夹具在索引器启动前写入，避免实时目录监控尚未收齐文件就开始排序验收。
        let fixtures = state.root.join("fixtures");
        std::fs::create_dir_all(fixtures.join("候选目录")).map_err(|e| e.to_string())?;
        for name in [format!("{}-报告.txt", state.token), format!("{}-报告计划.txt", state.token)] {
            std::fs::write(fixtures.join(name), "Synthetic UI benchmark fixture").map_err(|e| e.to_string())?;
        }
        // 固定两份精确文件及 241 份排序夹具，其余文件用于宽查询和大目录验收。
        for index in 0..state.options.fixture_files - 243 {
            std::fs::write(fixtures.join(format!("{}-many-{index:03}.txt", state.token)), "Synthetic broad-result benchmark fixture").map_err(|e| e.to_string())?;
        }
        let ranking = fixtures.join("ranking");
        std::fs::create_dir_all(&ranking).map_err(|e| e.to_string())?;
        for index in 0..240 {
            std::fs::write(ranking.join(format!("{index:03}-{}-late-报告.txt", state.token)), "Synthetic alphabetical ranking fixture").map_err(|e| e.to_string())?;
        }
        std::fs::write(ranking.join(format!("{}-late-报告.txt", state.token)), "Exact stem match beyond the first alphabetical page").map_err(|e| e.to_string())?;
        Ok(state)
    }
}

#[tauri::command]
pub fn get_ui_benchmark(app: AppHandle) -> Result<Option<Value>, String> {
    let Some(state) = app.try_state::<BenchmarkState>() else { return Ok(None); };
    // 后台启动器可能隐藏首次 ShowWindow；前端已就绪后确保诊断窗口实际可见。
    if let Some(window) = app.get_webview_window("main") { window.show().map_err(|e| e.to_string())?; }
    let fixtures = state.root.join("fixtures");
    let ranking = fixtures.join("ranking");
    Ok(Some(json!({
        "uniqueQuery":format!("{}/{}-报告",fixtures.display(),state.token),
        "uniqueTitle":format!("{}-报告.txt",state.token),
        "broadQuery":format!("{}/{}-many",fixtures.display(),state.token),
        "completionQuery":format!("{}/候",fixtures.display()),
        "rankingQuery":format!("{}/{}-late-报告",ranking.display(),state.token),
        "rankingTitle":format!("{}-late-报告.txt",state.token),
        "minimumBroadResults":50,
        "setupToFrontendMs":state.started.elapsed().as_millis(),
        "fixtureFiles":state.options.fixture_files,
        "coldTimeoutMs":state.options.cold_timeout_ms(),
        "soakSeconds":state.options.soak_seconds,
    })))
}

#[tauri::command]
pub async fn ui_soak_window(app: AppHandle, hidden_ms: Option<u64>) -> Result<Value, String> {
    let state = app.try_state::<BenchmarkState>().ok_or("UI soak mode is not active")?;
    if state.options.soak_seconds.is_none() { return Err("UI soak mode is not active".into()); }
    if hidden_ms.is_some_and(|ms| ms == 0 || ms > 15000) { return Err("Hidden idle must be between 1 and 15000 milliseconds".into()); }
    // 原生线程负责隐藏后的唤起，不依赖隐藏 WebView 中可能暂停的帧回调或计时器。
    tauri::async_runtime::spawn_blocking(move || {
        let window = app.get_webview_window("main").ok_or("Search window is unavailable")?;
        let mut hidden_verified = false;
        let begin = Instant::now();
        if let Some(ms) = hidden_ms {
            crate::service::hide_window(app.clone());
            hidden_verified = !window.is_visible().map_err(|e| e.to_string())?;
            std::thread::sleep(std::time::Duration::from_millis(ms));
            crate::service::show(&app, false);
        }
        let size = window.inner_size().map_err(|e| e.to_string())?;
        let scale = window.scale_factor().map_err(|e| e.to_string())?;
        Ok(json!({ "visible":window.is_visible().map_err(|e| e.to_string())?,
            "widthPx":size.width,"heightPx":size.height,"scaleFactor":scale,
            "logicalHeight":size.height as f64 / scale,
            "hiddenVerified":hidden_verified,"hiddenMs":if hidden_ms.is_some() { begin.elapsed().as_millis() } else { 0 },
            "timestampUnixMs":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_millis() }))
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn save_ui_benchmark(app: AppHandle, mut report: Value) -> Result<(), String> {
    let state = app.try_state::<BenchmarkState>().ok_or("UI benchmark mode is not active")?;
    let maximum_bytes = if state.options.soak_seconds.is_some() { 1_048_576 } else { 131072 };
    if !report.is_object() || serde_json::to_vec(&report).map_err(|e| e.to_string())?.len() > maximum_bytes {
        return Err("Invalid UI benchmark report".into());
    }
    // 仅显式测试模式可写报告；测试程序使用隔离目录，不读取或修改日常搜索历史。
    report["native"] = json!(true);
    if report["outcome"] != "passed" {
        let raw = format!("{}/{}-late-报告", state.root.join("fixtures/ranking").display(), state.token);
        let spec = crate::query::parse(&raw, "files")?;
        if let Some(expression) = crate::query::compile_exact_candidates(&spec, &crate::model::Settings::default()) {
            let begin = Instant::now();
            let response = app.state::<crate::service::AppState>().everything.query(expression, 0, Instant::now() + std::time::Duration::from_millis(250));
            report["exactProbe"] = match response {
                Ok(page) => json!({"durationMs":begin.elapsed().as_millis(),"count":page.total,"titles":page.entries.into_iter().map(|entry|entry.title).collect::<Vec<_>>() }),
                Err(error) => json!({"durationMs":begin.elapsed().as_millis(),"error":error}),
            };
        }
    }
    report["engine"] = app.state::<crate::service::AppState>().everything.diagnostics();
    report["startupToReportMs"] = json!(state.started.elapsed().as_millis());
    report["visibleWindow"] = json!(app.get_webview_window("main").is_some_and(|window| window.is_visible().unwrap_or(false)));
    std::fs::create_dir_all("test-results").map_err(|e| e.to_string())?;
    report["fixtureFiles"] = json!(state.options.fixture_files);
    report["isolatedRoot"] = json!(state.root);
    std::fs::write(state.options.report_path(), serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    app.exit(if report["outcome"] == "passed" { 0 } else { 1 });
    Ok(())
}
