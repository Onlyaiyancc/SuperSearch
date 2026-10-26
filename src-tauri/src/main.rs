#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod model;
mod query;
mod ranking;
mod store;
mod session;
mod platform;
mod everything;
mod engine;
mod service;
mod native_smoke;
mod layout;
mod ui_benchmark;
mod translation;
mod file_preview;
mod pdf_preview;
mod office_preview;
mod reminders;
mod notifications;
mod conversion;
mod conversion_components;
mod conversion_process;
mod qr_scan;
mod qr_links;
mod screen_ocr;
mod qr_capture;
mod qr_smoke;

use std::{sync::{atomic::AtomicBool, Mutex}, time::Instant};
use tauri::{menu::{Menu, MenuItem}, tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState}, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

fn smoke_test() -> Result<(), String> {
    native_smoke::run()
}

fn request_exit(app: &tauri::AppHandle) {
    let converting = app.try_state::<conversion::ConversionState>().is_some_and(|state| state.has_active());
    let installing = app.try_state::<conversion_components::ComponentState>().is_some_and(|state| state.has_active());
    if converting || installing {
        // 只有真正退出时确认；隐藏窗口不打断后台任务。
        use windows::{core::w, Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONQUESTION, MB_YESNO, IDYES}};
        let answer = unsafe { MessageBoxW(None, w!("仍有格式转换或组件安装任务。是否取消任务并退出？"), w!("退出 SuperSearch"), MB_YESNO | MB_ICONQUESTION) };
        if answer != IDYES { return; }
    }
    if let Some(state) = app.try_state::<conversion::ConversionState>() { state.cancel_all(); }
    if let Some(state) = app.try_state::<conversion_components::ComponentState>() { state.cancel_all(); }
    qr_scan::cancel(app);
    app.exit(0);
}

fn main() {
    if std::env::args().any(|a| a == "--screen-ocr-worker") {
        if let Err(error) = screen_ocr::run_worker() {
            // 工作进程只返回本模块固定错误，保留错误阶段便于诊断，不输出 OCR 原文。
            println!("{}", serde_json::json!({"error":error}));
            std::process::exit(1);
        }
        return;
    }
    if std::env::args().any(|a| a == "--smoke-test-screen-links") {
        if let Err(error) = qr_smoke::run_screen_links() { eprintln!("Screen links smoke test failed: {error}"); std::process::exit(1); }
        return;
    }
    if std::env::args().any(|a| a == "--smoke-test-screen-links-image") {
        if let Err(error) = qr_smoke::run_screen_links_image() { eprintln!("Screen links OCR smoke test failed: {error}"); std::process::exit(1); }
        return;
    }
    if std::env::args().any(|a| a=="--smoke-test-component-download") {
        let started = Instant::now();
        let directory = std::path::Path::new("test-results/usability-fixes");
        let path = directory.join("pandoc-download.partial");
        let result = (|| -> Result<(), String> {
            std::fs::create_dir_all(directory).map_err(|_| "Component diagnostic storage unavailable")?;
            if path.exists() { std::fs::remove_file(&path).map_err(|_| "Component diagnostic storage unavailable")?; }
            tokio::runtime::Builder::new_current_thread().enable_all().build()
                .map_err(|_| "Component diagnostic runtime unavailable")?.block_on(async {
                    tokio::time::timeout(std::time::Duration::from_secs(120), conversion_components::diagnose_pandoc_download(&path))
                        .await.map_err(|_| "Component diagnostic download timed out")?
                })
        })();
        let report = serde_json::json!({"status":if result.is_ok(){"passed"}else{"failed"},"check":"official-pandoc-download","durationMs":started.elapsed().as_millis(),"sizeAndSha256Verified":result.is_ok(),"error":result.as_ref().err()});
        if let Ok(text) = serde_json::to_string_pretty(&report) {
            let _ = std::fs::write(directory.join("pandoc-live-download.json"), &text);
            println!("{text}");
        }
        let _ = std::fs::remove_file(&path);
        if result.is_err() { std::process::exit(1); }
        return;
    }
    if std::env::args().any(|a| a=="--smoke-test-qr") {
        if let Err(error)=qr_smoke::run(){ eprintln!("QR screen smoke test failed: {error}"); std::process::exit(1); }
        return;
    }
    if std::env::args().any(|a| a=="--smoke-test-fast") {
        if let Err(error)=native_smoke::run_fast(){ eprintln!("Fast indexing smoke test failed: {error}"); std::process::exit(1); }
        return;
    }
    if std::env::args().any(|a| a=="--smoke-test-bundled") {
        if let Err(error)=native_smoke::run_bundled(){ eprintln!("Bundled smoke test failed: {error}"); std::process::exit(1); }
        return;
    }
    if std::env::args().any(|a| a=="--smoke-test") {
        if let Err(error)=smoke_test(){ eprintln!("Smoke test failed: {error}"); std::process::exit(1); }
        return;
    }
    let benchmark_options = match ui_benchmark::BenchmarkOptions::parse(&std::env::args().skip(1).collect::<Vec<_>>()) {
        Ok(options) => options,
        Err(error) => { eprintln!("Invalid UI diagnostic options: {error}"); std::process::exit(1); }
    };
    let benchmark = benchmark_options.is_some();
    let benchmark_state = if let Some(options) = benchmark_options {
        // 每次诊断先移除上一次同类报告，外部采样器不会把旧结果当作新验收。
        match std::fs::remove_file(options.report_path()) {
            Ok(()) => {},
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
            Err(error) => { eprintln!("Unable to clear previous UI diagnostic report: {error}"); std::process::exit(1); }
        }
        match ui_benchmark::BenchmarkState::with_options(options) {
            Ok(state) => Some(state),
            Err(error) => { eprintln!("Unable to prepare UI benchmark: {error}"); std::process::exit(1); }
        }
    } else { None };
    let mut context = tauri::generate_context!();
    if let Some(state) = &benchmark_state { state.isolate_webview(context.config_mut()); }
    let builder = tauri::Builder::default();
    // 性能验收使用隔离进程与数据，正常启动仍执行单实例规则。
    let builder = if benchmark { builder } else { builder.plugin(tauri_plugin_single_instance::init(|app,_,_| service::show(app,false))) };
    builder
        .manage(qr_scan::ScanState::default())
        .manage(translation::LookupState::default())
        .manage(pdf_preview::PdfState::default())
        .manage(reminders::SchedulerState::default())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().with_handler(|app,shortcut,event| {
            if event.state != ShortcutState::Pressed { return; }
            let Some(state) = app.try_state::<service::AppState>() else { return; };
            let Ok(store) = state.store.lock() else { return; };
            let settings = store.settings();
            drop(store);
            if settings.qr_scan_enabled && settings.qr_scan_shortcut.parse::<tauri_plugin_global_shortcut::Shortcut>().ok().as_ref() == Some(shortcut) {
                qr_scan::request_from_shortcut(app);
            } else if settings.shortcut.parse::<tauri_plugin_global_shortcut::Shortcut>().ok().as_ref() == Some(shortcut) { service::toggle(app); }
        }).build())
        .setup(move |app| {
            let directory=benchmark_state.as_ref().map(|state| state.root.join("store")).unwrap_or(app.path().app_local_data_dir()?);
            app.manage(conversion::ConversionState::new(directory.clone()).map_err(std::io::Error::other)?);
            app.manage(conversion_components::ComponentState::new(directory.clone()).map_err(std::io::Error::other)?);
            let store=store::Store::new(directory.clone()).map_err(std::io::Error::other)?;
            let settings=store.settings();
            let shortcut_error = if benchmark { None } else {
                let errors: Vec<_> = qr_scan::active_shortcuts(&settings).iter().filter_map(|shortcut| {
                    app.global_shortcut().register(shortcut.as_str()).err().map(|_| "Shortcut is unavailable. Open Settings from the tray and choose another shortcut.".to_string())
                }).collect();
                if errors.is_empty() { None } else { Some(errors.join(" ")) }
            };
            let mut engine_options = engine::EngineOptions::new(directory, settings.include_directories.clone());
            if let Some(state) = &benchmark_state {
                // UI 基准始终只索引独立夹具，不因本机服务状态而扫描整盘或改变测量路线。
                engine_options.scope = vec![state.root.join("fixtures").to_string_lossy().into_owned()];
                engine_options.force_folders = true;
            }
            let everything=everything::EverythingClient::managed(engine_options);
            app.manage(service::AppState {store:Mutex::new(store),cache:Mutex::new(service::SearchCache::default()),everything,refreshing:AtomicBool::new(false),apps_error:Mutex::new(None),shortcut_error:Mutex::new(shortcut_error),last_refresh:Mutex::new(Instant::now()),window_position_initialized:AtomicBool::new(false),window_monitor:Mutex::new(layout::MonitorMemory::default())});
            if let Some(state) = benchmark_state { app.manage(state); }
            if !benchmark { reminders::start(app.handle().clone()); }
            let open=MenuItem::with_id(app,"open","打开搜索",true,None::<&str>)?;
            let settings_item=MenuItem::with_id(app,"settings","设置",true,None::<&str>)?;
            let status=MenuItem::with_id(app,"status","Everything 状态",true,None::<&str>)?;
            let quit=MenuItem::with_id(app,"quit","退出",true,None::<&str>)?;
            let menu=Menu::with_items(app,&[&open,&settings_item,&status,&quit])?;
            TrayIconBuilder::with_id("supersearch-tray")
                .icon(tauri::image::Image::new_owned(include_bytes!("../icons/tray.rgba").to_vec(),32,32))
                .tooltip("SuperSearch · 按 Alt+Space 搜索")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app,event| match event.id().as_ref() {
                    "open"=>service::show(app,false),
                    "settings"|"status"=>service::show(app,true),
                    "quit"=>request_exit(app),
                    _=>{}
                })
                .on_tray_icon_event(|tray,event| {
                    if let TrayIconEvent::Click{button:MouseButton::Left,button_state:MouseButtonState::Up,..}=event { service::show(tray.app_handle(),false); }
                })
                .build(app)?;
            service::refresh_in_background(app.handle().clone());
            if benchmark || std::env::args().any(|a| a=="--show") { service::show(app.handle(),false); }
            Ok(())
        })
        .on_window_event(|window,event| {
            if let tauri::WindowEvent::Moved(_) = event { service::remember_window_monitor(window); }
            if let tauri::WindowEvent::ScaleFactorChanged { .. } = event {
                // DPI 变化可使系统重写物理尺寸，让前端按当前面板强制重发一次布局。
                let _=tauri::Emitter::emit(window,"window_scale_changed",());
            }
            if let tauri::WindowEvent::CloseRequested{api,..}=event {
                api.prevent_close();
                qr_scan::cancel(window.app_handle());
                service::invalidate(window.app_handle());
                let _=window.hide();
                let _=tauri::Emitter::emit(window,"hide_search",());
            }
        })
        .invoke_handler(tauri::generate_handler![
            qr_scan::prepare_qr_scan,qr_scan::scan_qr_screen,qr_scan::close_qr_scan,qr_scan::qr_scan_action,qr_scan::extract_qr_links,qr_scan::screen_web_link_action,
            service::conversion_from_result,conversion::choose_conversion_files,conversion::choose_conversion_directory,
            conversion::start_conversion,conversion::retry_conversion,conversion::list_conversions,conversion::cancel_conversion,conversion::open_conversion_output,conversion::release_conversion_sources,conversion::preview_conversion_csv,
            conversion_components::get_conversion_components,conversion_components::install_conversion_component,
            conversion_components::cancel_component_install,conversion_components::remove_conversion_component,conversion_components::import_conversion_component,
            service::preview_file,service::close_file_preview,reminders::create_reminder,reminders::list_reminders,reminders::cancel_reminder,translation::lookup_translation,translation::copy_translation,ui_benchmark::get_ui_benchmark,ui_benchmark::save_ui_benchmark,ui_benchmark::ui_soak_window,service::start_search,service::cancel_search,service::load_more,service::execute_action,service::set_pin,service::remove_pin,service::get_settings,service::update_settings,service::refresh_apps,service::get_provider_status,service::start_everything,service::choose_everything,service::add_app,service::set_app_aliases,service::clear_usage_history,service::search_web,service::hide_window,service::sync_window_layout,service::get_diagnostics])
        .build(context)
        .expect("Unable to start SuperSearch")
        .run(|app,event| {
            if matches!(event,tauri::RunEvent::Exit) {
                if let Some(state)=app.try_state::<conversion::ConversionState>() { state.cancel_all(); }
                if let Some(state)=app.try_state::<conversion_components::ComponentState>() { state.cancel_all(); }
                if let Some(state)=app.try_state::<service::AppState>() { state.everything.shutdown(); }
            }
        });
}
