use crate::{everything::{EverythingClient, FilePage}, model::Settings, query::{self, QuerySpec}};
use serde_json::{json, Value};
use std::{path::Path, time::{Duration, Instant}};

fn valid_matches(page: &FilePage, spec: &QuerySpec) -> bool {
    page.status.status == "ready" && !page.entries.is_empty() && page.entries.len() <= 200
        && page.total as usize >= page.entries.len()
        && page.entries.iter().all(|entry| {
            let title = entry.title.to_lowercase();
            let correct_kind = match spec.scope.as_str() {
                "files" => entry.kind == "file",
                "folders" => entry.kind == "folder",
                _ => entry.kind == "file" || entry.kind == "folder",
            };
            let extension = Path::new(&entry.title).extension().map(|e| e.to_string_lossy().to_ascii_lowercase());
            correct_kind && !title.is_empty() && !entry.locator.is_empty()
                && spec.terms.iter().all(|term| title.contains(&term.to_lowercase()))
                && (spec.extensions.is_empty() || extension.is_some_and(|ext| spec.extensions.contains(&ext)))
                && spec.directory.as_ref().is_none_or(|directory| {
                    query::directory_matches(&entry.locator, directory)
                })
        })
}

fn valid_empty(page: &FilePage) -> bool {
    page.status.status == "ready" && page.entries.is_empty() && page.total == 0
}

fn fixture_path_matches(locator: &str, fixture: &Path) -> bool {
    // Windows 索引返回反斜杠；夹具拼接可能保留正斜杠，比较前只统一分隔符。
    locator.replace('/', "\\").eq_ignore_ascii_case(&fixture.to_string_lossy().replace('/', "\\"))
}

fn check_query(client: &EverythingClient, settings: &Settings, raw: &str, name: &str, allow_empty: bool, report: &mut Value) -> Result<FilePage, String> {
    let spec = query::parse(raw, "all").map_err(|_| "Diagnostic query compilation failed")?;
    let started = Instant::now();
    let result = client.query(query::compile_everything(&spec, settings), 0, Instant::now() + Duration::from_millis(250));
    let elapsed = started.elapsed().as_millis();
    let page = match result {
        Ok(page) => page,
        Err(_) => {
            report["checks"][name] = json!({"outcome":"failed", "durationMs":elapsed, "error":"Native query backend failed"});
            return Err(format!("Native query backend failed during {name}"));
        }
    };
    let empty = valid_empty(&page);
    let valid = if name == "noResults" { empty } else { valid_matches(&page, &spec) || (allow_empty && empty) };
    report["checks"][name] = json!({
        "outcome": if !valid { "failed" } else if empty && name != "noResults" { "unverified-no-matches" } else { "passed" },
        "durationMs":elapsed, "returnedCount":page.entries.len(), "totalCount":page.total, "providerStatus":page.status.status,
    });
    if !valid { return Err(format!("Native result validation failed during {name}")); }
    Ok(page)
}

fn file_checks(client: &EverythingClient, report: &mut Value) -> Result<(), String> {
    // 诊断基线与独立 SDK 探针一致；另行检查产品默认的隐藏/系统过滤条件。
    let settings = Settings { hide_hidden: false, hide_system: false, ..Settings::default() };
    let known = check_query(client, &settings, "file: Everything.exe", "knownFile", false, report)?;
    check_query(client, &Settings::default(), "file: Everything.exe", "defaultFilteredFile", true, report)?;
    check_query(client, &settings, "file: 报告", "chineseFile", false, report)?;
    check_query(client, &settings, "folder: 项目", "folderOnly", true, report)?;
    check_query(client, &settings, "file: Everything.exe ext:exe", "extensionFilter", false, report)?;
    let directory = known.entries.iter().find_map(|entry| {
        Path::new(&entry.locator).parent().and_then(|parent| query::local_directory(&parent.to_string_lossy()).ok())
    }).ok_or("No safe parent directory was returned for the boundary check")?;
    // 只在内存中使用已有结果的父目录，报告不保存原始查询或任何路径。
    check_query(client, &settings, &format!("file: Everything.exe in:\"{directory}\""), "directoryBoundary", false, report)?;
    check_query(client, &settings, "file: SuperSearch_no_results_9fbc45fda2e9417bb208940dce8fc987", "noResults", false, report)?;

    let fixture_dir = std::env::current_dir().map_err(|_| "Working directory unavailable")?.join(".tools/smoke-fixtures");
    let fixture = fixture_dir.join("超级搜索验收报告.txt");
    if std::fs::create_dir_all(&fixture_dir).and_then(|_| std::fs::write(&fixture, "SuperSearch native integration fixture")).is_err() {
        report["freshFixtureIndexing"] = json!("unverified-fixture-unavailable");
    } else {
        let page = check_query(client, &settings, &format!("file: 超级搜索验收报告 in:\"{}\"", fixture_dir.display()), "freshFixture", true, report)?;
        report["freshFixtureIndexing"] = json!(if page.entries.iter().any(|entry| fixture_path_matches(&entry.locator, &fixture)) {
            "passed"
        } else { "unverified-not-indexed" });
    }
    Ok(())
}

pub fn run() -> Result<(), String> {
    let started = Instant::now();
    let mut report = json!({
        "outcome":"failed", "apps":"unverified", "appCount":0, "files":"unverified",
        "checks":{}, "freshFixtureIndexing":"unverified-not-run", "nativeActions":"not executed",
        "timingScope":"Single diagnostic run; no p95 benchmark"
    });
    for name in ["knownFile", "defaultFilteredFile", "chineseFile", "folderOnly", "extensionFilter", "directoryBoundary", "noResults", "freshFixture"] {
        report["checks"][name] = json!({"outcome":"unverified-not-run"});
    }
    let result = (|| {
        let apps = crate::platform::discover_apps(false).map_err(|_| "Application discovery failed")?;
        report["appCount"] = json!(apps.len());
        if apps.is_empty() { return Err("No applications discovered".to_string()); }
        report["apps"] = json!("passed");
        let client = EverythingClient::new();
        let status = client.status();
        let ready = status.status == "ready";
        report["provider"] = json!(status);
        if ready {
            report["files"] = json!("failed");
            file_checks(&client, &mut report)?;
            report["files"] = json!("passed");
        }
        Ok(())
    })();
    report["outcome"] = json!(if result.is_ok() { "diagnostics-completed" } else { "failed" });
    if let Err(error) = &result { report["error"] = json!(error); }
    report["elapsedMs"] = json!(started.elapsed().as_millis());
    let output = serde_json::to_string_pretty(&report).map_err(|_| "Report serialization failed")?;
    std::fs::create_dir_all("test-results").map_err(|_| "Report directory failed")?;
    std::fs::write("test-results/native-smoke.json", &output).map_err(|_| "Report save failed")?;
    println!("{output}");
    result
}

pub fn run_bundled() -> Result<(), String> {
    let started = Instant::now();
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
    let root = std::env::current_dir().map_err(|e| e.to_string())?.join(".tools").join("bundled-smoke").join(format!("{}-{nonce}", std::process::id()));
    let scope = root.join("fixtures");
    let folder = scope.join("验收项目");
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(scope.join("验收隐藏目录")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(scope.join("验收排除目录")).map_err(|e| e.to_string())?;
    for name in ["Everything.exe", "超级搜索验收报告.txt", "验收隐藏报告.txt", "验收系统报告.txt"] {
        std::fs::write(scope.join(name), "Bundled engine diagnostic fixture").map_err(|e| e.to_string())?;
    }
    std::fs::write(folder.join("超级搜索验收报告.txt"), "Directory boundary fixture").map_err(|e| e.to_string())?;
    for index in 0..205 { std::fs::write(scope.join(format!("验收分页{index:03}.txt")), "Pagination fixture").map_err(|e| e.to_string())?; }
    unsafe {
        use windows::{core::PCWSTR, Win32::Storage::FileSystem::{SetFileAttributesW, FILE_FLAGS_AND_ATTRIBUTES}};
        for (name, attribute) in [("验收隐藏报告.txt", 2), ("验收系统报告.txt", 4), ("验收隐藏目录", 2)] {
            let path: Vec<u16> = scope.join(name).to_string_lossy().encode_utf16().chain(Some(0)).collect();
            SetFileAttributesW(PCWSTR(path.as_ptr()), FILE_FLAGS_AND_ATTRIBUTES(attribute)).map_err(|e| e.to_string())?;
        }
    }
    // 与生产相同的非管理员目录索引，只索引本次夹具，即使机器已有服务也不依赖它。
    let mut options = crate::engine::EngineOptions::new(root.join("helper"), vec![scope.to_string_lossy().into_owned()]);
    options.force_folders = true;
    let engine_started = Instant::now();
    let client = EverythingClient::managed(options);
    let mut report = json!({"outcome":"failed", "mode":"bundled", "serviceDependency":false, "nativeActions":"No user file/application actions; diagnostic-owned engine termination only", "checks":{}});
    let result = (|| {
        let settings = Settings { hide_hidden: false, hide_system: false, ..Settings::default() };
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let status = client.status();
            if status.status == "ready" {
                let spec = query::parse("file: 超级搜索验收报告", "all")?;
                if client.query(query::compile_everything(&spec, &settings), 0, Instant::now() + Duration::from_millis(250)).is_ok_and(|page| page.total == 2) { break; }
            }
            if Instant::now() >= deadline { report["provider"] = json!(status); return Err("Bundled test-folder index did not become ready within 30 seconds.".into()); }
            std::thread::sleep(Duration::from_millis(100));
        }
        report["engine"] = client.diagnostics();
        report["startup"]["initialIndexReadyMs"] = json!(engine_started.elapsed().as_millis());
        if report["engine"]["indexing"] != "folders" || report["engine"]["scope"] != json!([scope.to_string_lossy()]) { return Err("Bundled diagnostics did not confirm isolated folder indexing.".into()); }
        for (name, raw) in [("knownFile", "file: Everything.exe"), ("chineseFile", "file: 超级搜索验收报告"), ("folderOnly", "folder: 验收项目"), ("extensionFilter", "file: 超级搜索验收报告 ext:txt"), ("noResults", "file: SuperSearch_no_results_9fbc45fda2e9417bb208940dce8fc987")] {
            check_query(&client, &settings, raw, name, false, &mut report)?;
        }
        check_query(&client, &settings, &format!("file: 超级搜索验收报告 in:\"{}\"", folder.display()), "directoryBoundary", false, &mut report)?;
        let simple = check_query(&client, &settings, &format!("{} 超级搜索验收报告", folder.display()), "simpleFolderInput", false, &mut report)?;
        if simple.total != 1 || !simple.entries.iter().all(|entry| fixture_path_matches(&entry.locator, &folder.join("超级搜索验收报告.txt"))) { return Err("Simple folder input escaped its selected directory.".into()); }
        for (name, raw) in [("joinedFolderInput", format!("{}/超级搜索验收报告", folder.display())), ("namedFolderInput", "验收项目/超级搜索验收报告".into())] {
            let page = check_query(&client, &settings, &raw, name, false, &mut report)?;
            if page.total != 1 || !page.entries.iter().all(|entry| fixture_path_matches(&entry.locator, &folder.join("超级搜索验收报告.txt"))) { return Err("Joined folder input escaped its directory boundary.".into()); }
        }
        let completion_settings = Settings { exclude_directories: vec![scope.join("验收排除目录").to_string_lossy().into_owned()], ..Settings::default() };
        let prefix = query::folder_prefix(&format!("{}/验收", scope.display())).ok_or("Folder prefix compilation failed")?;
        let candidates_page = client.query(query::compile_folder_completion(&prefix, &completion_settings), 0, Instant::now() + Duration::from_millis(250))?;
        let candidates = query::folder_completions(&prefix, candidates_page.entries, &completion_settings);
        if candidates.len() != 1 || !fixture_path_matches(&candidates[0].path, &folder) { return Err("Native folder completion did not respect hidden/excluded directory boundaries.".into()); }
        report["checks"]["folderCompletion"] = json!({"outcome":"passed","hiddenAndExcludedDirectoriesFiltered":true});
        let filtered = check_query(&client, &Settings::default(), "file: 验收隐藏报告", "hideHidden", true, &mut report)?;
        if filtered.total != 0 { return Err("Hidden file filtering failed.".into()); }
        report["checks"]["hideHidden"]["outcome"] = json!("passed");
        let filtered = check_query(&client, &Settings::default(), "file: 验收系统报告", "hideSystem", true, &mut report)?;
        if filtered.total != 0 { return Err("System file filtering failed.".into()); }
        report["checks"]["hideSystem"]["outcome"] = json!("passed");
        let spec = query::parse("file: 验收分页", "all")?;
        let expression = query::compile_everything(&spec, &settings);
        let first = client.query(expression.clone(), 0, Instant::now() + Duration::from_millis(250))?;
        let second = client.query(expression, 200, Instant::now() + Duration::from_millis(250))?;
        if first.total != 205 || first.entries.len() != 200 || second.entries.len() != 5 || first.entries.iter().any(|entry| second.entries.iter().any(|other| other.locator == entry.locator)) { return Err("Bundled file pagination failed.".into()); }
        report["checks"]["pagination"] = json!({"outcome":"passed","firstPage":200,"secondPage":5});
        let fresh = scope.join("验收实时更新.txt");
        std::fs::write(&fresh, "Live monitoring fixture").map_err(|e| e.to_string())?;
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let spec = query::parse("file: 验收实时更新", "all")?;
            if client.query(query::compile_everything(&spec, &settings), 0, Instant::now() + Duration::from_millis(250)).is_ok_and(|page| page.entries.iter().any(|entry| fixture_path_matches(&entry.locator, &fresh))) { break; }
            if Instant::now() >= deadline { return Err("Bundled folder monitoring did not detect the new fixture.".into()); }
            std::thread::sleep(Duration::from_millis(100));
        }
        report["checks"]["freshFixtureIndexing"] = json!({"outcome":"passed"});

        let wait_ready = || -> Result<u128, String> {
            let begin = Instant::now();
            let spec = query::parse("file: 超级搜索验收报告", "all")?;
            let expression = query::compile_everything(&spec, &settings);
            loop {
                if client.query(expression.clone(), 0, Instant::now() + Duration::from_millis(250))
                    .is_ok_and(|page| page.status.status == "ready" && page.total == 2) {
                    return Ok(begin.elapsed().as_millis());
                }
                if begin.elapsed() >= Duration::from_secs(30) { return Err("Owned engine recovery did not become ready within 30 seconds.".into()); }
                std::thread::sleep(Duration::from_millis(50));
            }
        };
        let old_pid = client.diagnostics()["processId"].as_u64().ok_or("Owned engine process ID is unavailable")? as u32;
        unsafe {
            use windows::Win32::{Foundation::CloseHandle, System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE}};
            // 仅终止本诊断刚创建的独立客户端；不停止服务或其它 Everything 实例。
            let process = OpenProcess(PROCESS_TERMINATE, false, old_pid).map_err(|e| e.to_string())?;
            let result = TerminateProcess(process, 0);
            let _ = CloseHandle(process);
            result.map_err(|e| e.to_string())?;
        }
        let recovery_ms = wait_ready()?;
        let recovered = client.diagnostics();
        if recovered["processId"] == json!(old_pid) || recovered["indexing"] != "folders" {
            return Err("Owned engine recovery did not replace the failed isolated client.".into());
        }
        report["checks"]["ownedEngineRecovery"] = json!({"outcome":"passed", "readyMs":recovery_ms, "scopePreserved": recovered["scope"] == report["engine"]["scope"]});
        if recovered["scope"] != report["engine"]["scope"] { return Err("Owned engine recovery changed the diagnostic scope.".into()); }

        let restart_started = Instant::now();
        client.retry(vec![scope.to_string_lossy().into_owned()])?;
        wait_ready()?;
        report["startup"]["cachedIndexRestartReadyMs"] = json!(restart_started.elapsed().as_millis());
        report["checks"]["cachedIndexRestart"] = json!({"outcome":"passed"});
        report["startup"]["measurement"] = json!("Isolated folder engine startup and manual cached-index restart; excludes WebView2/UI startup and whole-volume indexing.");

        let spec = query::parse("file: 验收分页", "all")?;
        let expression = query::compile_everything(&spec, &settings);
        let mut durations = Vec::new();
        for _ in 0..20 {
            let begin = Instant::now();
            let page = client.query(expression.clone(), 0, Instant::now() + Duration::from_millis(250))?;
            if page.status.status != "ready" || page.entries.len() != 200 || page.total != 205 { return Err("Warm query benchmark returned an incorrect candidate page.".into()); }
            durations.push(begin.elapsed().as_secs_f64() * 1000.0);
        }
        durations.sort_by(f64::total_cmp);
        report["queryBenchmark"] = json!({"samples":20, "returnedCandidates":200, "totalCandidates":205, "p50Ms":durations[9], "p95Ms":durations[18], "measurement":"Backend IPC only; excludes input debounce, ranking and UI rendering."});
        Ok(())
    })();
    client.shutdown();
    report["outcome"] = json!(if result.is_ok() { "passed" } else { "failed" });
    report["elapsedMs"] = json!(started.elapsed().as_millis());
    if let Err(error) = &result { report["error"] = json!(error); }
    let output = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    std::fs::create_dir_all("test-results").map_err(|e| e.to_string())?;
    std::fs::write("test-results/bundled-native-smoke.json", &output).map_err(|e| e.to_string())?;
    println!("{output}");
    result
}

pub fn run_fast() -> Result<(), String> {
    if !crate::engine::service_running() { return Err("The dedicated SuperSearch indexing service is not running. Install the machine setup first.".into()); }
    let started = Instant::now();
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
    let root = std::env::current_dir().map_err(|e| e.to_string())?.join(".tools/fast-smoke").join(nonce.to_string());
    let fixtures = root.join("fixtures");
    std::fs::create_dir_all(&fixtures).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(fixtures.join("补全目录")).map_err(|e| e.to_string())?;
    let token = format!("SuperSearch高速验收{nonce}");
    let original = fixtures.join(format!("{token}.txt"));
    let renamed = fixtures.join(format!("{token}-改名.txt"));
    std::fs::write(&original, "NTFS indexing diagnostic fixture").map_err(|e| e.to_string())?;
    // 与生产相同的普通权限客户端；卷索引覆盖磁盘，结果仍遵守目录筛选。
    let client = EverythingClient::managed(crate::engine::EngineOptions::new(root.join("helper"), vec![]));
    let settings = Settings { include_directories: vec![fixtures.to_string_lossy().into_owned()], hide_hidden: false, hide_system: false, ..Settings::default() };
    let expression = query::compile_everything(&query::parse(&format!("file: {token}"), "all")?, &settings);
    let mut report = json!({"outcome":"failed", "mode":"ntfs-usn", "nativeActions":"not executed", "checks":{}});
    let result = (|| {
        let wait_for = |expected: Option<&Path>, timeout: Duration| -> Result<u128, String> {
            let begin = Instant::now();
            loop {
                if let Ok(page) = client.query(expression.clone(), 0, Instant::now() + Duration::from_millis(250)) {
                    if page.status.status == "ready" && match expected {
                        Some(path) => page.total == 1 && page.entries.len() == 1 && fixture_path_matches(&page.entries[0].locator, path),
                        None => page.total == 0 && page.entries.is_empty(),
                    } { return Ok(begin.elapsed().as_millis()); }
                }
                if begin.elapsed() >= timeout { return Err("NTFS/USN diagnostic did not observe the expected file state within its deadline.".into()); }
                std::thread::sleep(Duration::from_millis(50));
            }
        };
        let load_ms = wait_for(Some(&original), Duration::from_secs(120))?;
        report["engine"] = client.diagnostics();
        if report["engine"]["indexing"] != "ntfs-usn" { return Err("The diagnostic did not use NTFS/USN fast indexing.".into()); }
        report["checks"]["initialFixture"] = json!({"outcome":"passed","indexReadyMs":load_ms});
        std::fs::remove_file(&original).map_err(|e| e.to_string())?;
        wait_for(None, Duration::from_secs(10))?;
        // 先确认消失，再创建同名文件，避免旧索引命中伪装为实时更新。
        std::fs::write(&original, "Fresh NTFS fixture after index ready").map_err(|e| e.to_string())?;
        let create_ms = wait_for(Some(&original), Duration::from_secs(10))?;
        std::fs::rename(&original, &renamed).map_err(|e| e.to_string())?;
        let rename_ms = wait_for(Some(&renamed), Duration::from_secs(10))?;
        let mut samples = Vec::new();
        for _ in 0..20 {
            let begin = Instant::now();
            let page = client.query(expression.clone(), 0, Instant::now() + Duration::from_millis(250))?;
            if page.status.status != "ready" || page.total != 1 || page.entries.len() != 1 || !fixture_path_matches(&page.entries[0].locator, &renamed) { return Err("Fast query returned an unexpected fixture or duplicate row.".into()); }
            samples.push(begin.elapsed().as_micros());
        }
        samples.sort_unstable();
        let excluded = Settings { exclude_directories: vec![fixtures.to_string_lossy().into_owned()], ..settings.clone() };
        let filtered = client.query(query::compile_everything(&query::parse(&token, "all")?, &excluded), 0, Instant::now() + Duration::from_millis(250))?;
        if !valid_empty(&filtered) { return Err("NTFS indexing bypassed the configured excluded directory.".into()); }
        let simple = query::parse(&format!("{} {token}", fixtures.display()), "all")?;
        let scoped = client.query(query::compile_everything(&simple, &settings), 0, Instant::now() + Duration::from_millis(250))?;
        // 夹具名只属于此目录；指定空目录还必须排除同名的全盘命中。
        let outside = fixtures.join("empty sibling");
        std::fs::create_dir_all(&outside).map_err(|e| e.to_string())?;
        let empty_spec = query::parse(&format!("{} {token}", outside.display()), "all")?;
        let wrong_scope = client.query(query::compile_everything(&empty_spec, &settings), 0, Instant::now() + Duration::from_millis(250))?;
        if scoped.total != 1 || scoped.entries.len() != 1 || !fixture_path_matches(&scoped.entries[0].locator, &renamed) || !valid_empty(&wrong_scope) { return Err("Simple folder input did not preserve the directory boundary in NTFS mode.".into()); }
        report["checks"]["simpleFolderInput"] = json!({"outcome":"passed"});
        let prefix = query::folder_prefix(&format!("{}/补全", fixtures.display())).ok_or("Folder prefix compilation failed")?;
        let page = client.query(query::compile_folder_completion(&prefix, &settings), 0, Instant::now() + Duration::from_millis(250))?;
        let candidates = query::folder_completions(&prefix, page.entries, &settings);
        if candidates.len() != 1 || !fixture_path_matches(&candidates[0].path, &fixtures.join("补全目录")) { return Err("NTFS folder completion failed.".into()); }
        report["checks"]["folderCompletion"] = json!({"outcome":"passed"});
        for (name, raw) in [("joinedFolderInput", format!("{}/{token}", fixtures.display())), ("namedFolderInput", format!("fixtures/{token}"))] {
            let page = check_query(&client, &settings, &raw, name, false, &mut report)?;
            if page.total != 1 || !fixture_path_matches(&page.entries[0].locator, &renamed) { return Err("Joined folder input failed in NTFS mode.".into()); }
        }
        std::fs::remove_file(&renamed).map_err(|e| e.to_string())?;
        let delete_ms = wait_for(None, Duration::from_secs(10))?;
        report["checks"]["liveUpdates"] = json!({"outcome":"passed","createMs":create_ms,"renameMs":rename_ms,"deleteMs":delete_ms,"pollIntervalMs":50});
        report["checks"]["directoryFilters"] = json!({"outcome":"passed"});
        report["checks"]["warmQuery"] = json!({"outcome":"passed","samples":20,"medianUs":samples[10],"p95Us":samples[18],"maximumUs":samples[19],"scope":"Backend IPC query for one unique fixture, not end-to-end UI latency."});
        Ok(())
    })();
    client.shutdown();
    report["outcome"] = json!(if result.is_ok() { "passed" } else { "failed" });
    report["elapsedMs"] = json!(started.elapsed().as_millis());
    if let Err(error) = &result { report["error"] = json!(error); }
    let output = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    std::fs::create_dir_all("test-results").map_err(|e| e.to_string())?;
    std::fs::write("test-results/fast-native-smoke.json", &output).map_err(|e| e.to_string())?;
    println!("{output}");
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Entry, ProviderStatus};

    fn page(rows: &[(&str, &str, &str)]) -> FilePage {
        FilePage {
            entries: rows.iter().map(|(kind, title, locator)| Entry {
                entity_id: "test".into(), kind: (*kind).into(), title: (*title).into(),
                subtitle: String::new(), locator: (*locator).into(), launch_kind: "path".into(),
                aliases: vec![], launch_key: (*locator).into(),
            }).collect(),
            total: rows.len() as u32,
            status: ProviderStatus { id: "everything".into(), status: "ready".into(), message: String::new() },
        }
    }

    #[test]
    fn fixture_binding_normalizes_separators_without_accepting_other_paths() {
        let fixture = Path::new(r"C:\Workspace\.tools/smoke-fixtures\超级搜索验收报告.txt");
        assert!(fixture_path_matches(r"C:\Workspace\.tools\smoke-fixtures\超级搜索验收报告.txt", fixture));
        assert!(fixture_path_matches(r"c:/Workspace/.tools/smoke-fixtures/超级搜索验收报告.txt", fixture));
        assert!(!fixture_path_matches(r"C:\Workspace\.tools\smoke-fixtures\超级搜索验收报告副本.txt", fixture));
        assert!(!fixture_path_matches(r"C:\Workspace\.tools\other-fixtures\超级搜索验收报告.txt", fixture));
        assert!(!fixture_path_matches(r"C:\WorkspaceBackup\.tools\smoke-fixtures\超级搜索验收报告.txt", fixture));
    }

    #[test]
    fn existing_match_requires_nonempty_correctly_typed_matching_rows() {
        let spec = crate::query::parse("file: 报告", "all").unwrap();
        assert!(valid_matches(&page(&[("file", "年度报告.txt", r"C:\Work\年度报告.txt")]), &spec));
        assert!(!valid_matches(&page(&[]), &spec));
        assert!(!valid_matches(&page(&[("folder", "报告", r"C:\Work\报告")]), &spec));
        assert!(!valid_matches(&page(&[("file", "Other.txt", r"C:\Work\Other.txt")]), &spec));
        let mut unavailable = page(&[("file", "报告.txt", r"C:\Work\报告.txt")]);
        unavailable.status.status = "unavailable".into();
        assert!(!valid_matches(&unavailable, &spec));
    }

    #[test]
    fn directory_and_extension_checks_reject_sibling_prefix_and_wrong_extension() {
        let spec = crate::query::parse(r#"file: Everything.exe ext:exe in:"C:\Work""#, "all").unwrap();
        assert!(valid_matches(&page(&[("file", "Everything.exe", r"C:\Work\Tools\Everything.exe")]), &spec));
        assert!(!valid_matches(&page(&[("file", "Everything.exe", r"C:\Workspace\Everything.exe")]), &spec));
        assert!(!valid_matches(&page(&[("file", "Everything.exe.txt", r"C:\Work\Everything.exe.txt")]), &spec));
    }

    #[test]
    fn no_results_requires_ready_provider_and_zero_total() {
        assert!(valid_empty(&page(&[])));
        assert!(!valid_empty(&page(&[("file", "unexpected.txt", r"C:\Work\unexpected.txt")])));
        let mut inconsistent = page(&[]);
        inconsistent.total = 1;
        assert!(!valid_empty(&inconsistent));
        inconsistent.total = 0;
        inconsistent.status.status = "unavailable".into();
        assert!(!valid_empty(&inconsistent));
    }
}
