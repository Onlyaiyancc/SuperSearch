use crate::qr_capture::CaptureRect;
pub use crate::qr_links::web_url;
use base64::Engine;
use image::{ImageFormat, RgbaImage};
use serde::Serialize;
use std::{
    io::Cursor,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::Shortcut;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanItem {
    pub id: String,
    pub text: String,
    pub thumbnail: String,
    pub domain: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub session_id: String,
    pub items: Vec<ScanItem>,
    pub limited: bool,
    pub web_links: Vec<crate::qr_links::WebLink>,
    pub web_links_limited: bool,
    pub web_link_warning: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedScan {
    session_id: String,
}

struct Session {
    id: String,
    rect: CaptureRect,
    items: Vec<ScanItem>,
    web_links: Vec<crate::qr_links::WebLink>,
    cancel: Arc<AtomicBool>,
}

#[derive(Default)]
pub struct ScanState {
    session: Mutex<Option<Session>>,
    next: AtomicU64,
    running: AtomicBool,
}

impl ScanState {
    fn prepare(&self, rect: CaptureRect) -> Result<String, String> {
        if self.running.load(Ordering::Acquire) {
            return Err("QR scan busy".into());
        }
        let id = format!("qr-{}", self.next.fetch_add(1, Ordering::Relaxed));
        let mut session = self.session.lock().map_err(|_| "QR session unavailable")?;
        if self.running.load(Ordering::Acquire) {
            return Err("QR scan busy".into());
        }
        if let Some(previous) = session.as_ref() {
            previous.cancel.store(true, Ordering::Release);
        }
        *session = Some(Session {
            id: id.clone(),
            rect,
            items: Vec::new(),
            web_links: Vec::new(),
            cancel: Arc::new(AtomicBool::new(false)),
        });
        Ok(id)
    }
    fn is_current(&self, id: &str) -> bool {
        self.session
            .lock()
            .ok()
            .is_some_and(|session| session.as_ref().is_some_and(|session| session.id == id))
    }
    fn close(&self, id: &str) {
        if let Ok(mut session) = self.session.lock() {
            if session.as_ref().is_some_and(|session| session.id == id) {
                if let Some(current) = session.as_ref() {
                    current.cancel.store(true, Ordering::Release);
                }
                *session = None;
            }
        }
    }
    fn cancel(&self) {
        if let Ok(mut session) = self.session.lock() {
            if let Some(current) = session.as_ref() {
                current.cancel.store(true, Ordering::Release);
            }
            *session = None;
        }
    }
    fn result(&self, id: &str, item: &str) -> Result<ScanItem, String> {
        self.session
            .lock()
            .map_err(|_| "QR session unavailable")?
            .as_ref()
            .filter(|session| session.id == id)
            .and_then(|session| session.items.iter().find(|result| result.id == item))
            .cloned()
            .ok_or_else(|| "QR session expired".into())
    }
    fn web_link(&self, id: &str, item: &str) -> Result<crate::qr_links::WebLink, String> {
        self.session
            .lock()
            .map_err(|_| "QR session unavailable")?
            .as_ref()
            .filter(|session| session.id == id && !session.cancel.load(Ordering::Acquire))
            .and_then(|session| session.web_links.iter().find(|result| result.id == item))
            .cloned()
            .ok_or_else(|| "QR link expired".into())
    }
}

pub fn validate_shortcuts(search: &str, qr: &str) -> Result<(), String> {
    let search: Shortcut = search.parse().map_err(|_| "Invalid shortcut")?;
    if !qr.is_empty() {
        if qr.len() > 80 || !qr.contains('+') {
            return Err("QR shortcut is invalid".into());
        }
        let qr: Shortcut = qr.parse().map_err(|_| "QR shortcut is invalid")?;
        if search == qr {
            return Err("QR shortcut conflicts with search shortcut".into());
        }
    }
    Ok(())
}

pub fn active_shortcuts(settings: &crate::model::Settings) -> Vec<String> {
    let mut shortcuts = vec![settings.shortcut.clone()];
    if settings.qr_scan_enabled && !settings.qr_scan_shortcut.is_empty() {
        shortcuts.push(settings.qr_scan_shortcut.clone());
    }
    shortcuts
}

pub fn same_shortcut(a: &str, b: &str) -> bool {
    match (a.parse::<Shortcut>(), b.parse::<Shortcut>()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

fn enabled(app: &AppHandle) -> Result<(), String> {
    if app
        .state::<crate::service::AppState>()
        .store
        .lock()
        .map_err(|_| "Storage unavailable")?
        .settings()
        .qr_scan_enabled
    {
        Ok(())
    } else {
        Err("QR scan disabled".into())
    }
}

pub fn cancel(app: &AppHandle) {
    if let Some(state) = app.try_state::<ScanState>() {
        state.cancel();
    }
}

pub fn request_from_shortcut(app: &AppHandle) {
    if enabled(app).is_err() || app.state::<ScanState>().running.load(Ordering::Acquire) {
        return;
    }
    crate::service::show(app, false);
    let _ = app.emit("qr_scan_requested", ());
}

#[tauri::command]
pub fn prepare_qr_scan(app: AppHandle, state: State<ScanState>) -> Result<PreparedScan, String> {
    enabled(&app)?;
    // 隔离 UI 基准不能意外读取用户屏幕；真实捕获仅由日常显式操作触发。
    if app
        .try_state::<crate::ui_benchmark::BenchmarkState>()
        .is_some()
    {
        return Err("QR screen capture unavailable in benchmark".into());
    }
    Ok(PreparedScan {
        session_id: state.prepare(crate::qr_capture::cursor_monitor()?)?,
    })
}

#[tauri::command]
pub fn close_qr_scan(state: State<ScanState>, session_id: String) {
    state.close(&session_id);
}

async fn window_visibility(app: &AppHandle, id: &str, visible: bool) -> Result<(), String> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let handle = app.clone();
    let id = id.to_string();
    app.run_on_main_thread(move || {
        let result = (|| {
            let state = handle.state::<ScanState>();
            // 会话校验与窗口操作处于同一临界区，关闭命令不能在校验后插入一次过期恢复。
            let session = state
                .session
                .lock()
                .map_err(|_| "QR session unavailable".to_string())?;
            if !session
                .as_ref()
                .is_some_and(|session| session.id == id && !session.cancel.load(Ordering::Acquire))
            {
                return Err("QR session expired".to_string());
            }
            enabled(&handle)?;
            let window = handle
                .get_webview_window("main")
                .ok_or("QR window unavailable")?;
            if visible {
                window.show().map_err(|_| "QR window unavailable")?;
                window.set_focus().map_err(|_| "QR window unavailable")?;
            } else {
                window.hide().map_err(|_| "QR window unavailable")?;
                if window.is_visible().map_err(|_| "QR window unavailable")? {
                    return Err("QR window unavailable".into());
                }
            }
            Ok(())
        })();
        let _ = sender.send(result);
    })
    .map_err(|_| "QR window unavailable")?;
    receiver.await.map_err(|_| "QR window unavailable")?
}

struct RunningScan(AppHandle);
impl Drop for RunningScan {
    fn drop(&mut self) {
        self.0
            .state::<ScanState>()
            .running
            .store(false, Ordering::Release);
    }
}

#[tauri::command]
pub async fn scan_qr_screen(app: AppHandle, session_id: String) -> Result<ScanResult, String> {
    enabled(&app)?;
    let state = app.state::<ScanState>();
    if state
        .running
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("QR scan busy".into());
    }
    let _running = RunningScan(app.clone());
    let (rect, cancel) = state
        .session
        .lock()
        .map_err(|_| "QR session unavailable")?
        .as_ref()
        .filter(|session| session.id == session_id)
        .map(|session| (session.rect, session.cancel.clone()))
        .ok_or("QR session expired")?;
    let mut restored = false;
    let outcome = async {
        window_visibility(&app, &session_id, false).await?;
        // 让合成器完成一次画面提交，避免把刚隐藏的搜索框捕入截图。
        tokio::time::sleep(Duration::from_millis(60)).await;
        if !state.is_current(&session_id) {
            return Err("QR session expired".into());
        }
        let capture_cancel = cancel.clone();
        let image = tauri::async_runtime::spawn_blocking(move || {
            if capture_cancel.load(Ordering::Acquire) {
                return Err("QR session expired".into());
            }
            crate::qr_capture::capture(rect)
        })
        .await
        .map_err(|_| "QR capture unavailable")??;
        // 捕获完成就恢复进度面板，用户可以在 OCR 工作期间关闭并取消后台进程。
        window_visibility(&app, &session_id, true).await?;
        restored = true;
        let worker_app = app.clone();
        let (items, limited, web_links, web_links_limited, web_link_warning) =
            tauri::async_runtime::spawn_blocking(move || {
                if cancel.load(Ordering::Acquire) {
                    return Err("QR session expired".to_string());
                }
                let (items, limited) = decode(&image)?;
                let (web_links, web_links_limited, warning) =
                    match crate::conversion_components::recognize_screen_links(
                        &worker_app,
                        &image,
                        &cancel,
                    ) {
                        Ok(links) => (links.items, links.limited, None),
                        // OCR 失败不丢弃同一张截图中已经识别出的二维码，也不暴露底层日志。
                        Err(error) => (Vec::new(), false, Some(screen_link_warning(&error))),
                    };
                if cancel.load(Ordering::Acquire) {
                    return Err("QR session expired".to_string());
                }
                Ok((items, limited, web_links, web_links_limited, warning))
            })
            .await
            .map_err(|_| "QR decode unavailable")??;
        enabled(&app)?;
        let mut session = state.session.lock().map_err(|_| "QR session unavailable")?;
        let session = session
            .as_mut()
            .filter(|session| session.id == session_id)
            .ok_or("QR session expired")?;
        session.items = items.clone();
        session.web_links = web_links.clone();
        Ok(ScanResult {
            session_id: session_id.clone(),
            items,
            limited,
            web_links,
            web_links_limited,
            web_link_warning,
        })
    }
    .await;
    // 失败也恢复面板以显示错误；取消后绝不重新打开窗口或覆盖新视图。
    if !restored && state.is_current(&session_id) {
        window_visibility(&app, &session_id, true).await?;
    }
    outcome
}

fn screen_link_warning(error: &str) -> String {
    // 只保留明确列出的固定错误；未知引擎消息不会携带屏幕文字进入前端。
    match error {
        "Conversion component requires local package"
        | "Conversion component package is not published in this build"
        | "Conversion component ocr missing"
        | "Conversion timeout"
        | "Conversion component installation busy"
        | "Conversion components busy"
        | "Conversion component is in use"
        | "Conversion component checksum mismatch"
        | "Conversion component size mismatch" => error.to_string(),
        _ => "Conversion screen OCR unavailable".to_string(),
    }
}

pub(crate) fn decode(image: &RgbaImage) -> Result<(Vec<ScanItem>, bool), String> {
    crate::qr_capture::checked_bytes(image.width(), image.height())?;
    let view = zxingcpp::ImageView::from_slice(
        image.as_raw(),
        image.width(),
        image.height(),
        zxingcpp::ImageFormat::RGBA,
    )
    .map_err(|_| "QR decode unavailable")?;
    let mut codes = zxingcpp::read()
        .formats([zxingcpp::BarcodeFormat::QRCode])
        .try_invert(true)
        .max_number_of_symbols(33)
        .from(view)
        .map_err(|_| "QR decode unavailable")?;
    codes.retain(|code| code.is_valid());
    codes.sort_by_key(|code| {
        let p = code.position();
        let points = [p.top_left, p.top_right, p.bottom_right, p.bottom_left];
        (
            points.iter().map(|p| p.y).min(),
            points.iter().map(|p| p.x).min(),
        )
    });
    let mut limited = codes.len() > 32;
    let mut items = Vec::new();
    for code in codes.into_iter().take(32) {
        // 上游文本接口使用 C 字符串，原始载荷含 NUL 时必须拒绝，不能展示或打开截断后的网址。
        if code.bytes().contains(&0) {
            limited = true;
            continue;
        }
        let text = code.text();
        if text.len() > 16 * 1024 {
            limited = true;
            continue;
        }
        let p = code.position();
        let points = [p.top_left, p.top_right, p.bottom_right, p.bottom_left];
        // 沿二维码四边形内部采样，旋转码也不会把外接矩形里的周围屏幕内容传给前端。
        let crop = thumbnail(image, points);
        let mut png = Cursor::new(Vec::new());
        crop.write_to(&mut png, ImageFormat::Png)
            .map_err(|_| "QR decode unavailable")?;
        let thumbnail = format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png.into_inner())
        );
        let domain = web_url(&text).and_then(|url| url.host_str().map(str::to_owned));
        items.push(ScanItem {
            id: items.len().to_string(),
            text,
            thumbnail,
            domain,
        });
    }
    Ok((items, limited))
}

fn thumbnail(image: &RgbaImage, points: [zxingcpp::PointI; 4]) -> RgbaImage {
    RgbaImage::from_fn(160, 160, |x, y| {
        let u = (f64::from(x) + 0.5) / 160.0;
        let v = (f64::from(y) + 0.5) / 160.0;
        let weights = [(1.0 - u) * (1.0 - v), u * (1.0 - v), u * v, (1.0 - u) * v];
        let sx: f64 = points
            .iter()
            .zip(weights)
            .map(|(point, weight)| f64::from(point.x) * weight)
            .sum();
        let sy: f64 = points
            .iter()
            .zip(weights)
            .map(|(point, weight)| f64::from(point.y) * weight)
            .sum();
        *image.get_pixel(
            sx.clamp(0.0, f64::from(image.width() - 1)) as u32,
            sy.clamp(0.0, f64::from(image.height() - 1)) as u32,
        )
    })
}

#[tauri::command]
pub fn extract_qr_links(
    app: AppHandle,
    session_id: String,
    result_id: String,
) -> Result<crate::qr_links::ExtractedLinks, String> {
    enabled(&app)?;
    let item = app.state::<ScanState>().result(&session_id, &result_id)?;
    crate::qr_links::extract(&item.text)
}

fn action_text(item: &ScanItem, link_id: Option<&str>) -> Result<String, String> {
    match link_id {
        None => Ok(item.text.clone()),
        Some(id) => crate::qr_links::extract(&item.text)?
            .items
            .into_iter()
            .find(|link| link.id == id)
            .map(|link| link.url)
            .ok_or_else(|| "QR link expired".into()),
    }
}

#[tauri::command]
pub async fn qr_scan_action(
    app: AppHandle,
    session_id: String,
    result_id: String,
    action: String,
    link_id: Option<String>,
) -> Result<(), String> {
    enabled(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        enabled(&app)?;
        let state = app.state::<ScanState>();
        let item = state.result(&session_id, &result_id)?;
        let text = action_text(&item, link_id.as_deref())?;
        if text.is_empty()
            || text.len() > 16 * 1024
            || text
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
        {
            return Err("QR content cannot be copied or opened".into());
        }
        match action.as_str() {
            "copy" => crate::platform::copy_text(&text),
            "open" => crate::platform::open_qr_url(&text),
            _ => Err("QR invalid action".into()),
        }
    })
    .await
    .map_err(|_| "QR action unavailable")?
}

#[tauri::command]
pub async fn screen_web_link_action(
    app: AppHandle,
    session_id: String,
    link_id: String,
    action: String,
) -> Result<(), String> {
    enabled(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        enabled(&app)?;
        // 前端只能提交当前会话的链接 ID，不能把任意 URL 注入复制/打开操作。
        let link = app.state::<ScanState>().web_link(&session_id, &link_id)?;
        if web_url(&link.url).is_none() {
            return Err("QR link expired".into());
        }
        match action.as_str() {
            "copy" => crate::platform::copy_text(&link.url),
            "open" => crate::platform::open_qr_url(&link.url),
            _ => Err("QR invalid action".into()),
        }
    })
    .await
    .map_err(|_| "QR action unavailable")?
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::DynamicImage;

    #[test]
    fn screen_link_actions_are_native_session_scoped_and_close_cancels_ocr() {
        let state = ScanState::default();
        let rect = crate::qr_capture::CaptureRect {
            left: 0,
            top: 0,
            width: 100,
            height: 100,
        };
        let id = state.prepare(rect).unwrap();
        let signal = {
            let mut current = state.session.lock().unwrap();
            let session = current.as_mut().unwrap();
            session.web_links = crate::qr_links::extract_screen_text("https://example.com/screen")
                .unwrap()
                .items;
            session.cancel.clone()
        };
        assert_eq!(
            state.web_link(&id, "0").unwrap().url,
            "https://example.com/screen"
        );
        for forged in ["1", "https://forged.example/", "0\0", ""] {
            assert!(state.web_link(&id, forged).is_err());
        }
        assert!(state.web_link("old-session", "0").is_err());
        state.close(&id);
        assert!(signal.load(Ordering::Acquire));
        assert!(state.web_link(&id, "0").is_err());
    }

    #[test]
    fn screen_warning_preserves_only_known_error_categories() {
        for error in [
            "Conversion component requires local package",
            "Conversion component package is not published in this build",
            "Conversion timeout",
            "Conversion component installation busy",
            "Conversion component is in use",
        ] {
            assert_eq!(screen_link_warning(error), error);
        }
        for unknown in [
            "https://private.example/screen",
            "Conversion screenshot text private",
            "Conversion timeout\nsecret",
        ] {
            assert_eq!(
                screen_link_warning(unknown),
                "Conversion screen OCR unavailable"
            );
        }
    }

    #[test]
    fn stale_window_restores_do_not_target_the_replacement_session() {
        let state = ScanState::default();
        let rect = crate::qr_capture::CaptureRect {
            left: 0,
            top: 0,
            width: 100,
            height: 100,
        };
        let old = state.prepare(rect).unwrap();
        state.close(&old);
        let new = state.prepare(rect).unwrap();
        assert!(!state.is_current(&old));
        assert!(state.is_current(&new));
        state.close(&old);
        assert!(state.is_current(&new));
    }

    #[test]
    fn replacing_or_cancelling_a_scan_signals_the_old_ocr_worker() {
        let state = ScanState::default();
        let rect = crate::qr_capture::CaptureRect {
            left: 0,
            top: 0,
            width: 100,
            height: 100,
        };
        state.prepare(rect).unwrap();
        let old = state
            .session
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .cancel
            .clone();
        state.prepare(rect).unwrap();
        assert!(old.load(Ordering::Acquire));
        let current = state
            .session
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .cancel
            .clone();
        state.cancel();
        assert!(current.load(Ordering::Acquire));
    }

    #[test]
    fn qr_link_actions_resolve_only_current_native_text_and_known_link_ids() {
        let item = ScanItem {
            id: "source".into(),
            text: "网页：https://example.com/a\n备用：http://other.example/b".into(),
            thumbnail: String::new(),
            domain: None,
        };
        assert_eq!(action_text(&item, None).unwrap(), item.text);
        assert_eq!(
            action_text(&item, Some("0")).unwrap(),
            "https://example.com/a"
        );
        assert_eq!(
            action_text(&item, Some("1")).unwrap(),
            "http://other.example/b"
        );
        for forged in ["2", "-1", "https://forged.example/", "0\0", ""] {
            assert!(action_text(&item, Some(forged)).is_err());
        }
    }

    #[test]
    fn qr_urls_only_allow_explicit_web_addresses() {
        assert_eq!(
            web_url("https://example.com/中文?q=1").unwrap().host_str(),
            Some("example.com")
        );
        for text in [
            "example.com",
            "javascript:alert(1)",
            "file:///C:/Windows",
            "shell:AppsFolder",
            "https://",
            "https://user:pass@example.com",
            "https://example.com\n",
            "https://example.com\0.exe",
        ] {
            assert!(web_url(text).is_none(), "unexpected URL: {text:?}");
        }
    }

    #[test]
    fn qr_sessions_reject_old_ids_and_release_results() {
        let state = ScanState::default();
        let rect = crate::qr_capture::CaptureRect {
            left: -1920,
            top: 0,
            width: 1920,
            height: 1080,
        };
        let first = state.prepare(rect).unwrap();
        let second = state.prepare(rect).unwrap();
        assert!(!state.is_current(&first));
        assert!(state.is_current(&second));
        state.close(&first);
        assert!(state.is_current(&second));
        state.close(&second);
        assert!(!state.is_current(&second));
    }

    #[test]
    fn qr_actions_reject_unknown_or_expired_results() {
        let state = ScanState::default();
        let id = state
            .prepare(crate::qr_capture::CaptureRect {
                left: 0,
                top: 0,
                width: 100,
                height: 100,
            })
            .unwrap();
        assert!(state.result(&id, "missing").is_err());
        state.cancel();
        assert!(state.result(&id, "0").is_err());
    }

    #[test]
    fn qr_settings_are_opt_in_and_reject_equivalent_shortcuts() {
        let defaults: crate::model::Settings = serde_json::from_str("{}").unwrap();
        assert!(!defaults.qr_scan_enabled);
        assert!(defaults.qr_scan_shortcut.is_empty());
        assert!(validate_shortcuts("Control+Space", "Ctrl+Space").is_err());
        assert!(validate_shortcuts("Alt+Space", "Control+Shift+Q").is_ok());
        assert!(validate_shortcuts("Alt+Space", "").is_ok());
        for shortcut in ["Q", "Space", "Alt+", "Control+NotAKey", "Control+Q\0"] {
            assert!(validate_shortcuts("Alt+Space", shortcut).is_err());
        }
    }

    fn qr_image(text: &str) -> RgbaImage {
        let code = zxingcpp::create(zxingcpp::BarcodeFormat::QRCode)
            .from_str(text)
            .unwrap();
        let output = code
            .to_image_with(&zxingcpp::write().scale(5).add_quiet_zones(true))
            .unwrap();
        DynamicImage::ImageLuma8(
            image::GrayImage::from_raw(
                output.width() as u32,
                output.height() as u32,
                output.data(),
            )
            .unwrap(),
        )
        .to_rgba8()
    }

    #[test]
    fn qr_decode_real_chinese_and_url_fixtures() {
        for text in [
            "https://example.com/扫码?q=中文",
            "屏幕扫码测试\n第二行",
            "WIFI:T:WPA;S:Test;P:password;;",
        ] {
            let image = qr_image(text);
            let (items, limited) = decode(&image).unwrap();
            assert!(!limited);
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].text, text);
            assert_eq!(items[0].domain.is_some(), text.starts_with("https://"));
            let encoded = items[0]
                .thumbnail
                .strip_prefix("data:image/png;base64,")
                .unwrap();
            let thumbnail = image::load_from_memory(
                &base64::engine::general_purpose::STANDARD
                    .decode(encoded)
                    .unwrap(),
            )
            .unwrap();
            assert!(thumbnail.width() <= 160 && thumbnail.height() <= 160);
        }
    }

    #[test]
    fn qr_decode_rotated_inverted_and_multiple_positions() {
        let first = qr_image("https://example.com/first");
        let second = qr_image("中文内容");
        let mut canvas = RgbaImage::from_pixel(900, 600, image::Rgba([255, 255, 255, 255]));
        image::imageops::overlay(&mut canvas, &first, 20, 30);
        image::imageops::overlay(&mut canvas, &second, 520, 300);
        let (items, limited) = decode(&canvas).unwrap();
        assert!(!limited);
        assert_eq!(
            items
                .iter()
                .map(|item| item.text.as_str())
                .collect::<Vec<_>>(),
            ["https://example.com/first", "中文内容"]
        );
        let rotated = image::imageops::rotate90(&first);
        assert_eq!(
            decode(&rotated).unwrap().0[0].text,
            "https://example.com/first"
        );
        let mut inverted = first.clone();
        image::imageops::invert(&mut inverted);
        assert_eq!(
            decode(&inverted).unwrap().0[0].text,
            "https://example.com/first"
        );
        assert!(decode(&RgbaImage::from_pixel(
            320,
            240,
            image::Rgba([255, 255, 255, 255])
        ))
        .unwrap()
        .0
        .is_empty());
    }

    #[test]
    fn qr_decode_result_count_is_bounded_and_reports_overflow() {
        let code = qr_image("bounded");
        let step = code.width() + 30;
        let mut canvas =
            RgbaImage::from_pixel(step * 7, step * 5, image::Rgba([255, 255, 255, 255]));
        for i in 0..35 {
            image::imageops::overlay(
                &mut canvas,
                &code,
                i64::from((i % 7) * step),
                i64::from((i / 7) * step),
            );
        }
        let (items, limited) = decode(&canvas).unwrap();
        assert_eq!(items.len(), 32);
        assert!(limited);
    }

    #[test]
    fn qr_binary_nul_payload_is_not_silently_truncated_into_a_link() {
        let code = zxingcpp::create(zxingcpp::BarcodeFormat::QRCode)
            .from_slice(b"https://example.com\0hidden")
            .unwrap();
        let output = code
            .to_image_with(&zxingcpp::write().scale(5).add_quiet_zones(true))
            .unwrap();
        let image = DynamicImage::ImageLuma8(
            image::GrayImage::from_raw(
                output.width() as u32,
                output.height() as u32,
                output.data(),
            )
            .unwrap(),
        )
        .to_rgba8();
        let (items, limited) = decode(&image).unwrap();
        assert!(items.is_empty());
        assert!(limited);
    }

    #[test]
    fn qr_rotated_thumbnail_does_not_expose_bounding_box_corners() {
        let image = RgbaImage::from_fn(101, 101, |x, y| {
            if x.abs_diff(50) + y.abs_diff(50) <= 48 {
                image::Rgba([0, 0, 0, 255])
            } else {
                image::Rgba([255, 0, 0, 255])
            }
        });
        let points = [
            zxingcpp::PointI { x: 50, y: 3 },
            zxingcpp::PointI { x: 97, y: 50 },
            zxingcpp::PointI { x: 50, y: 97 },
            zxingcpp::PointI { x: 3, y: 50 },
        ];
        assert!(thumbnail(&image, points)
            .pixels()
            .all(|pixel| pixel.0 == [0, 0, 0, 255]));
    }

    #[test]
    fn qr_mixed_orientations_use_screen_position_for_sorting() {
        let first = image::imageops::rotate180(&qr_image("first"));
        let second = qr_image("second");
        let mut canvas = RgbaImage::from_pixel(700, 400, image::Rgba([255, 255, 255, 255]));
        image::imageops::overlay(&mut canvas, &first, 20, 20);
        image::imageops::overlay(&mut canvas, &second, 400, 35);
        let items = decode(&canvas).unwrap().0;
        assert_eq!(
            items
                .iter()
                .map(|item| item.text.as_str())
                .collect::<Vec<_>>(),
            ["first", "second"]
        );
    }

    #[test]
    fn qr_preparation_cannot_replace_running_capture() {
        let state = ScanState::default();
        let rect = crate::qr_capture::CaptureRect {
            left: 0,
            top: 0,
            width: 100,
            height: 100,
        };
        let id = state.prepare(rect).unwrap();
        state.running.store(true, Ordering::Release);
        assert!(state.prepare(rect).is_err());
        assert!(state.is_current(&id));
        state.cancel();
        assert!(!state.is_current(&id));
        assert!(state.running.load(Ordering::Acquire));
        state.running.store(false, Ordering::Release);
        assert!(state.prepare(rect).is_ok());
    }
}
