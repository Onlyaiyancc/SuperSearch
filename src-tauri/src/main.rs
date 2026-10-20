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

use std::{sync::{atomic::AtomicBool, Mutex}, time::Instant};
use tauri::{menu::{Menu, MenuItem}, tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState}, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

fn smoke_test() -> Result<(), String> {
    native_smoke::run()
}

fn main() {
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
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().with_handler(|app,_,event| {
            if event.state==ShortcutState::Pressed { service::toggle(app); }
        }).build())
        .setup(move |app| {
            let directory=benchmark_state.as_ref().map(|state| state.root.join("store")).unwrap_or(app.path().app_local_data_dir()?);
            let store=store::Store::new(directory.clone()).map_err(std::io::Error::other)?;
            let settings=store.settings();
            let shortcut_error=if benchmark { None } else { app.global_shortcut().register(settings.shortcut.as_str()).err().map(|_|"Shortcut is unavailable. Open Settings from the tray and choose another shortcut.".to_string()) };
            let mut engine_options = engine::EngineOptions::new(directory, settings.include_directories.clone());
            if let Some(state) = &benchmark_state {
                // UI 基准始终只索引独立夹具，不因本机服务状态而扫描整盘或改变测量路线。
                engine_options.scope = vec![state.root.join("fixtures").to_string_lossy().into_owned()];
                engine_options.force_folders = true;
            }
            let everything=everything::EverythingClient::managed(engine_options);
            app.manage(service::AppState {store:Mutex::new(store),cache:Mutex::new(service::SearchCache::default()),everything,refreshing:AtomicBool::new(false),apps_error:Mutex::new(None),shortcut_error:Mutex::new(shortcut_error),last_refresh:Mutex::new(Instant::now()),window_position_initialized:AtomicBool::new(false)});
            if let Some(state) = benchmark_state { app.manage(state); }
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
                    "quit"=>app.exit(0),
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
            if let tauri::WindowEvent::ScaleFactorChanged { .. } = event {
                // DPI 变化可使系统重写物理尺寸，让前端按当前面板强制重发一次布局。
                let _=tauri::Emitter::emit(window,"window_scale_changed",());
            }
            if let tauri::WindowEvent::CloseRequested{api,..}=event {
                api.prevent_close();
                service::invalidate(window.app_handle());
                let _=window.hide();
                let _=tauri::Emitter::emit(window,"hide_search",());
            }
        })
        .invoke_handler(tauri::generate_handler![ui_benchmark::get_ui_benchmark,ui_benchmark::save_ui_benchmark,ui_benchmark::ui_soak_window,service::start_search,service::cancel_search,service::load_more,service::execute_action,service::set_pin,service::remove_pin,service::get_settings,service::update_settings,service::refresh_apps,service::get_provider_status,service::start_everything,service::choose_everything,service::add_app,service::set_app_aliases,service::clear_usage_history,service::search_web,service::hide_window,service::sync_window_layout,service::get_diagnostics])
        .build(context)
        .expect("Unable to start SuperSearch")
        .run(|app,event| {
            if matches!(event,tauri::RunEvent::Exit) {
                if let Some(state)=app.try_state::<service::AppState>() { state.everything.shutdown(); }
            }
        });
}
