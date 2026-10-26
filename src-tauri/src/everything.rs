use crate::model::{Entry, ProviderStatus};
use crate::engine::{EngineOptions, ManagedEngine};
use serde_json::{json, Value};
use std::{cell::RefCell, ffi::c_void, ptr, sync::{atomic::{AtomicBool, Ordering}, mpsc, Arc, Mutex}, time::{Duration, Instant}};

pub struct FilePage {
    pub entries: Vec<Entry>,
    pub total: u32,
    pub status: ProviderStatus,
}

pub struct PendingQuery {
    reply: mpsc::Receiver<Result<FilePage, String>>,
    deadline: Instant,
}

impl PendingQuery {
    pub fn wait(self) -> Result<FilePage, String> {
        self.reply.recv_timeout(self.deadline.saturating_duration_since(Instant::now())).map_err(|_| "File search timed out.".to_string())?
    }
}

struct Job {
    expression: String,
    offset: u32,
    deadline: Instant,
    reply: mpsc::SyncSender<Result<FilePage, String>>,
}

struct Mailbox {
    query: Option<Job>,
    status: Option<mpsc::SyncSender<ProviderStatus>>,
    cached: ProviderStatus,
    options: Option<EngineOptions>,
    restart: bool,
    diagnostics: Value,
}

struct Shared {
    mailbox: Mutex<Mailbox>,
    stop: AtomicBool,
    running: AtomicBool,
    wake: usize,
}

pub struct EverythingClient { shared: Arc<Shared> }

fn provider(status: &str, message: &str) -> ProviderStatus {
    ProviderStatus { id: "everything".into(), status: status.into(), message: message.into() }
}

impl EverythingClient {
    pub fn new() -> Self {
        Self::create(None)
    }

    pub fn managed(options: EngineOptions) -> Self {
        Self::create(Some(options))
    }

    fn create(options: Option<EngineOptions>) -> Self {
        // 自动复位事件和原生消息共同唤醒线程；隐藏空闲时没有计时轮询。
        let wake = unsafe { CreateEventW(ptr::null(), 0, 0, ptr::null()) } as usize;
        let shared = Arc::new(Shared {
            mailbox: Mutex::new(Mailbox { query: None, status: None, cached: if options.is_some() { provider("indexLoading", "Bundled file index is loading. First-time folder indexing may take several minutes.") } else { provider("unavailable", "File search has not been checked.") }, options: options.clone(), restart: false, diagnostics: if options.is_some() { json!({"mode":"bundled","version":crate::engine::VERSION,"indexing":"starting"}) } else { json!({"mode":"external"}) } }),
            stop: AtomicBool::new(false), running: AtomicBool::new(wake != 0), wake,
        });
        if wake != 0 {
            let state = shared.clone();
            if std::thread::Builder::new().name("everything-ipc".into()).spawn(move || worker(state, options)).is_err() {
                shared.running.store(false, Ordering::Release);
                unsafe { CloseHandle(wake as *mut c_void); }
            }
        }
        Self { shared }
    }

    pub fn retry(&self, scope: Vec<String>) -> Result<(), String> {
        if !self.shared.running.load(Ordering::Acquire) { return Err("File search worker is unavailable. Restart SuperSearch.".into()); }
        let mut mailbox = self.shared.mailbox.lock().map_err(|_| "File search worker is unavailable.")?;
        let options = mailbox.options.as_mut().ok_or("Bundled file search is unavailable in this diagnostic mode.")?;
        options.scope = scope;
        mailbox.restart = true;
        mailbox.cached = provider("indexLoading", "Bundled file index is loading. First-time folder indexing may take several minutes.");
        drop(mailbox);
        unsafe { SetEvent(self.shared.wake as *mut c_void); }
        Ok(())
    }

    pub fn diagnostics(&self) -> Value {
        let Ok(mailbox) = self.shared.mailbox.try_lock() else { return json!({"mode":"bundled","version":crate::engine::VERSION,"indexing":"checking"}); };
        let mut diagnostics = mailbox.diagnostics.clone();
        diagnostics["status"] = json!(mailbox.cached.status);
        diagnostics["message"] = json!(mailbox.cached.message);
        diagnostics
    }

    pub fn shutdown(&self) {
        self.shared.stop.store(true, Ordering::Release);
        if self.shared.running.load(Ordering::Acquire) { unsafe { SetEvent(self.shared.wake as *mut c_void); } }
        let deadline = Instant::now() + Duration::from_secs(2);
        while self.shared.running.load(Ordering::Acquire) && Instant::now() < deadline { std::thread::sleep(Duration::from_millis(10)); }
    }

    pub fn query(&self, expression: String, offset: u32, deadline: Instant) -> Result<FilePage, String> {
        self.submit_query(expression, offset, deadline)?.wait()
    }

    pub fn submit_query(&self, expression: String, offset: u32, deadline: Instant) -> Result<PendingQuery, String> {
        page_limit(offset)?;
        if expression.contains('\0') || expression.encode_utf16().count() > 8192 {
            return Err("Invalid file search expression.".into());
        }
        let deadline = deadline.min(Instant::now() + Duration::from_millis(250));
        if Instant::now() >= deadline { return Err("File search timed out.".into()); }
        if !self.shared.running.load(Ordering::Acquire) { return Err("File search worker is unavailable.".into()); }
        let (tx, rx) = mpsc::sync_channel(1);
        {
            // 不等待 SDK 状态检查持有的邮箱锁，保证查询期限也覆盖提交阶段。
            let mut mailbox = self.shared.mailbox.try_lock().map_err(|error| match error {
                std::sync::TryLockError::WouldBlock => "File search worker is busy. Try again.",
                std::sync::TryLockError::Poisoned(_) => "File search worker is unavailable.",
            })?;
            if let Some(old) = mailbox.query.replace(Job { expression, offset, deadline, reply: tx }) {
                let _ = old.reply.send(Err("File search was superseded.".into()));
            }
        }
        unsafe { SetEvent(self.shared.wake as *mut c_void); }
        Ok(PendingQuery { reply: rx, deadline })
    }

    pub fn cached_status(&self) -> ProviderStatus {
        // 只读取已知状态；SDK 正在持锁检查时立即返回，首批应用结果不等待 IPC。
        if !self.shared.running.load(Ordering::Acquire) { return provider("unavailable", "File search worker is unavailable."); }
        match self.shared.mailbox.try_lock() {
            Ok(mailbox) if mailbox.restart => provider("indexLoading", "Bundled file index is loading. First-time folder indexing may take several minutes."),
            Ok(mailbox) => mailbox.cached.clone(),
            Err(std::sync::TryLockError::WouldBlock) => provider("loading", "File search status is being checked."),
            Err(_) => provider("unavailable", "File search worker is unavailable."),
        }
    }

    pub fn status(&self) -> ProviderStatus {
        if !self.shared.running.load(Ordering::Acquire) { return provider("unavailable", "File search worker is unavailable."); }
        let (tx, rx) = mpsc::sync_channel(1);
        let mut mailbox = match self.shared.mailbox.try_lock() {
            Ok(mailbox) => mailbox,
            Err(std::sync::TryLockError::WouldBlock) => return provider("loading", "File search status is being checked."),
            Err(_) => return provider("unavailable", "File search worker is unavailable."),
        };
        let was_loading = mailbox.restart || matches!(mailbox.cached.status.as_str(), "loading" | "indexLoading");
        if let Some(old) = mailbox.status.replace(tx) { let _ = old.send(mailbox.cached.clone()); }
        drop(mailbox);
        unsafe { SetEvent(self.shared.wake as *mut c_void); }
        rx.recv_timeout(Duration::from_millis(250)).unwrap_or_else(|_| {
            // 重启旧进程和加载数据库可能超过单次 IPC 期限；保留已知加载态，允许可见设置继续检查。
            let known = self.cached_status();
            if matches!(known.status.as_str(), "loading" | "indexLoading") || (was_loading && known.status == "ready") { known }
            else { provider("unavailable", "File search status timed out.") }
        })
    }
}

impl Drop for EverythingClient {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        if self.shared.running.load(Ordering::Acquire) { unsafe { SetEvent(self.shared.wake as *mut c_void); } }
    }
}

#[derive(Default)]
struct ReplySlot {
    active: Option<(u32, Instant)>,
    result: Option<Result<FilePage, String>>,
}

thread_local! { static REPLIES: RefCell<ReplySlot> = RefCell::new(ReplySlot::default()); }

struct RecoveryBudget { window: Instant, attempts: u8 }

impl RecoveryBudget {
    fn new(now: Instant) -> Self { Self { window: now, attempts: 0 } }

    fn allow(&mut self, now: Instant) -> bool {
        // 仅查询或状态请求触发恢复；每 30 秒最多两次，空闲时不轮询。
        if now.saturating_duration_since(self.window) >= Duration::from_secs(30) {
            self.window = now;
            self.attempts = 0;
        }
        if self.attempts >= 2 { return false; }
        self.attempts += 1;
        true
    }
}

unsafe extern "system" fn window_proc(hwnd: *mut c_void, message: u32, wparam: usize, lparam: isize) -> isize {
    if message == 0x004a {
        // 先核验当前 ID 和截止时间，旧回复永远不进入 SDK 的结果快照。
        let accepted = REPLIES.with(|slot| {
            let mut slot = slot.borrow_mut();
            if let Some((id, deadline)) = slot.active {
                if Instant::now() < deadline && SuperSearch_ValidateReply(lparam, id) != 0
                    && Everything_IsQueryReply(message, wparam, lparam, id) != 0 {
                    slot.result = Some(snapshot());
                    return true;
                }
            }
            false
        });
        // 回复窗口始终受理 COPYDATA，过期数据只丢弃，不阻塞外部客户端。
        return if accepted { 1 } else { 0 };
    }
    DefWindowProcW(hwnd, message, wparam, lparam)
}

fn worker(shared: Arc<Shared>, options: Option<EngineOptions>) {
    unsafe {
        let name: Vec<u16> = options.as_ref().map(|options| options.instance.as_str()).unwrap_or("").encode_utf16().chain(Some(0)).collect();
        if Everything_SetInstanceNameW(name.as_ptr()) == 0 { shared.running.store(false, Ordering::Release); CloseHandle(shared.wake as *mut c_void); return; }
        let (mut engine, mut engine_error) = launch_engine(&shared, options);
        let instance = GetModuleHandleW(ptr::null());
        let class: Vec<u16> = "SuperSearch.Everything.Reply\0".encode_utf16().collect();
        let wc = WindowClass { style: 0, proc: Some(window_proc), class_extra: 0, window_extra: 0,
            instance, icon: ptr::null_mut(), cursor: ptr::null_mut(), background: ptr::null_mut(), menu: ptr::null(), name: class.as_ptr() };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(0, class.as_ptr(), class.as_ptr(), 0, 0, 0, 0, 0,
            -3isize as *mut c_void, ptr::null_mut(), instance, ptr::null());
        if hwnd.is_null() {
            shared.running.store(false, Ordering::Release);
            CloseHandle(shared.wake as *mut c_void);
            return;
        }
        let mut active: Option<Job> = None;
        let mut pending: Option<Job> = None;
        let mut reply_id = 0;
        let mut failures = 0u32;
        let mut backoff_until = Instant::now();
        let mut recovery = RecoveryBudget::new(Instant::now());
        while !shared.stop.load(Ordering::Acquire) {
            let (restart, incoming, status_reply) = shared.mailbox.lock().ok().map(|mut mailbox| {
                let restart = if mailbox.restart {
                    mailbox.restart = false;
                    mailbox.cached = provider("indexLoading", "Bundled file index is loading. First-time folder indexing may take several minutes.");
                    mailbox.options.clone()
                } else { None };
                (restart, mailbox.query.take(), mailbox.status.take())
            }).unwrap_or((None, None, None));
            let demanded = incoming.is_some() || status_reply.is_some();
            if let Some(job) = incoming {
                if let Some(old) = pending.replace(job) { let _ = old.reply.send(Err("File search was superseded.".into())); }
            }
            let automatic = demanded && restart.is_none()
                && (engine_error.is_some() || engine.as_mut().is_some_and(|engine| engine.needs_restart()))
                && recovery.allow(Instant::now());
            let restart = restart.map(|options| {
                recovery = RecoveryBudget::new(Instant::now());
                options
            }).or_else(|| {
                if automatic { shared.mailbox.lock().ok().and_then(|mailbox| mailbox.options.clone()) } else { None }
            });
            if let Some(options) = restart {
                if let Ok(mut mailbox) = shared.mailbox.lock() {
                    mailbox.cached = provider("indexLoading", "Bundled file index is loading. First-time folder indexing may take several minutes.");
                }
                REPLIES.with(|slot| *slot.borrow_mut() = ReplySlot::default());
                // 保留最新待发查询；旧活动查询失效，迟到回复不能进入重启后的快照。
                if let Some(job) = active.take() { let _ = job.reply.send(Err("Bundled file search is restarting.".into())); }
                drop(engine.take());
                (engine, engine_error) = launch_engine(&shared, Some(options));
                failures = 0;
                backoff_until = Instant::now();
            }
            if let Some(reply) = status_reply {
                // SDK 状态调用可能等待 IPC；等待期间释放邮箱，允许新搜索立即入队。
                SuperSearch_SetIpcTimeout(180);
                let status = managed_status(engine.as_mut(), engine_error.as_deref());
                if let Ok(mut mailbox) = shared.mailbox.lock() { mailbox.cached = status.clone(); }
                let _ = reply.send(status);
            }
            let mut message = Message::default();
            while PeekMessageW(&mut message, ptr::null_mut(), 0, 0, 1) != 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            let result = REPLIES.with(|slot| slot.borrow_mut().result.take());
            if let Some(result) = result {
                let result = result.map(|mut page| {
                    if let Some(engine) = engine.as_mut() { page.status = engine.status(page.status); }
                    if let Ok(mut mailbox) = shared.mailbox.lock() { mailbox.cached = page.status.clone(); }
                    page
                });
                if let Some(job) = active.take() {
                    let on_time = Instant::now() < job.deadline;
                    failures = if result.is_ok() && on_time { 0 } else { failures.saturating_add(1) };
                    let _ = job.reply.send(if on_time { result } else { Err("File search timed out.".into()) });
                }
                REPLIES.with(|slot| slot.borrow_mut().active = None);
            }
            if active.as_ref().is_some_and(|job| Instant::now() >= job.deadline) {
                if let Some(job) = active.take() { let _ = job.reply.send(Err("File search timed out.".into())); }
                REPLIES.with(|slot| slot.borrow_mut().active = None);
                failures = failures.saturating_add(1);
                backoff_until = Instant::now() + Duration::from_millis((25u64 << failures.min(4)).min(400));
            }
            if active.is_none() {
                if let Some(job) = pending.take() {
                    if Instant::now() >= job.deadline {
                        let _ = job.reply.send(Err("File search timed out.".into()));
                    } else if Instant::now() < backoff_until {
                        pending = Some(job);
                    } else {
                        let budget = job.deadline.saturating_duration_since(Instant::now()).as_millis().min(250) as u32;
                        SuperSearch_SetIpcTimeout(budget);
                        let state = managed_status(engine.as_mut(), engine_error.as_deref());
                        if let Ok(mut mailbox) = shared.mailbox.lock() { mailbox.cached = state.clone(); }
                        if Instant::now() >= job.deadline {
                            let _ = job.reply.send(Err("File search timed out.".into()));
                        } else if state.status != "ready" {
                            let _ = job.reply.send(Ok(FilePage { entries: vec![], total: 0, status: state }));
                        } else if let Ok(id) = next_reply_id(reply_id) {
                            reply_id = id;
                            Everything_Reset();
                            let expression: Vec<u16> = job.expression.encode_utf16().chain(Some(0)).collect();
                            Everything_SetInstanceNameW(name.as_ptr());
                            Everything_SetSearchW(expression.as_ptr());
                            Everything_SetMatchPath(0);
                            Everything_SetMatchCase(0);
                            Everything_SetMatchWholeWord(0);
                            Everything_SetRegex(0);
                            Everything_SetSort(1);
                            Everything_SetRequestFlags(3);
                            Everything_SetMax(page_limit(job.offset).unwrap_or(0));
                            Everything_SetOffset(job.offset);
                            Everything_SetReplyWindow(hwnd);
                            Everything_SetReplyID(id);
                            REPLIES.with(|slot| slot.borrow_mut().active = Some((id, job.deadline)));
                            if Everything_QueryW(0) != 0 { active = Some(job); }
                            else {
                                REPLIES.with(|slot| slot.borrow_mut().active = None);
                                let _ = job.reply.send(Err("File search IPC is unavailable.".into()));
                                failures = failures.saturating_add(1);
                                backoff_until = Instant::now() + Duration::from_millis((25u64 << failures.min(4)).min(400));
                            }
                        } else { let _ = job.reply.send(Err("File search reply identifiers exhausted. Restart SuperSearch.".into())); }
                    }
                }
            }
            let wait = if let Some(job) = active.as_ref() {
                job.deadline.saturating_duration_since(Instant::now()).as_millis().min(250) as u32
            } else if let Some(job) = pending.as_ref() {
                backoff_until.min(job.deadline).saturating_duration_since(Instant::now()).as_millis().min(400) as u32
            } else { u32::MAX };
            let handle = shared.wake as *mut c_void;
            MsgWaitForMultipleObjectsEx(1, &handle, wait, 0x04ff, 0x0004);
        }
        REPLIES.with(|slot| *slot.borrow_mut() = ReplySlot::default());
        Everything_CleanUp();
        DestroyWindow(hwnd);
        UnregisterClassW(class.as_ptr(), instance);
        drop(engine);
        shared.running.store(false, Ordering::Release);
        CloseHandle(shared.wake as *mut c_void);
    }
}

fn launch_engine(shared: &Shared, options: Option<EngineOptions>) -> (Option<ManagedEngine>, Option<String>) {
    let Some(options) = options else { return (None, None); };
    let started = Instant::now();
    match ManagedEngine::start(options) {
        Ok(engine) => {
            if let Ok(mut mailbox) = shared.mailbox.lock() {
                mailbox.diagnostics = engine.diagnostics();
                mailbox.diagnostics["startupMs"] = json!(started.elapsed().as_millis());
            }
            (Some(engine), None)
        }
        Err(error) => {
            if let Ok(mut mailbox) = shared.mailbox.lock() { mailbox.cached = provider("unavailable", &error); mailbox.diagnostics["indexing"] = json!("unavailable"); }
            (None, Some(error))
        }
    }
}

unsafe fn managed_status(engine: Option<&mut ManagedEngine>, error: Option<&str>) -> ProviderStatus {
    if let Some(error) = error { return provider("unavailable", error); }
    let status = sdk_status();
    if let Some(engine) = engine { engine.status(status) } else { status }
}

unsafe fn sdk_status() -> ProviderStatus {
    let major = Everything_GetMajorVersion();
    if SuperSearch_IpcFailed() != 0 { return provider("unavailable", "Everything IPC timed out."); }
    if major == 0 {
        return if Everything_GetLastError() == 2 {
            provider("notRunning", "Everything client is not running or does not provide IPC.")
        } else { provider("unavailable", "Everything IPC is unavailable.") };
    }
    if major != 1 || Everything_GetMinorVersion() < 4 {
        return provider("unavailable", "This Everything version is not supported. Use the full 1.4 release.");
    }
    if Everything_IsDBLoaded() == 0 {
        if SuperSearch_IpcFailed() != 0 { return provider("unavailable", "Everything IPC timed out."); }
        return if Everything_GetLastError() == 0 { provider("indexLoading", "File index is loading.") }
            else { provider("unavailable", "Everything index status is unavailable.") };
    }
    provider("ready", "Everything is ready.")
}

unsafe fn snapshot() -> Result<FilePage, String> {
    let count = Everything_GetNumResults().min(200);
    let total = Everything_GetTotResults();
    let mut entries = Vec::with_capacity(count as usize);
    let mut bytes = 0usize;
    for index in 0..count {
        let size = Everything_GetResultFullPathNameW(index, ptr::null_mut(), 0);
        if size == 0 || size > 32767 { return Err("Everything returned an invalid path length.".into()); }
        let mut path = vec![0u16; size as usize + 1];
        let written = Everything_GetResultFullPathNameW(index, path.as_mut_ptr(), path.len() as u32);
        if written != size || path[size as usize] != 0 { return Err("Everything returned an incomplete path.".into()); }
        let path = String::from_utf16(&path[..size as usize]).map_err(|_| "Everything returned a path that cannot be represented safely.")?;
        bytes += path.len() * 4;
        if bytes > 16 * 1024 * 1024 { return Err("File search result memory limit reached.".into()); }
        let title = std::path::Path::new(&path).file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_else(|| path.clone());
        let subtitle = std::path::Path::new(&path).parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        entries.push(Entry { entity_id: crate::platform::entity_id("file", &path), kind: if Everything_IsFolderResult(index) != 0 { "folder" } else { "file" }.into(),
            title, subtitle, locator: path.clone(), launch_kind: "path".into(), aliases: vec![], launch_key: path });
    }
    Ok(FilePage { entries, total, status: provider("ready", "Everything is ready.") })
}

#[repr(C)]
struct WindowClass { style: u32, proc: Option<unsafe extern "system" fn(*mut c_void, u32, usize, isize) -> isize>, class_extra: i32,
    window_extra: i32, instance: *mut c_void, icon: *mut c_void, cursor: *mut c_void, background: *mut c_void, menu: *const u16, name: *const u16 }

#[repr(C)]
#[derive(Default)]
struct Message { hwnd: *mut c_void, message: u32, wparam: usize, lparam: isize, time: u32, point: [i32; 2], private: u32 }

#[link(name = "kernel32")]
extern "system" {
    fn CreateEventW(attributes: *const c_void, manual: i32, initial: i32, name: *const u16) -> *mut c_void;
    fn SetEvent(event: *mut c_void) -> i32;
    fn CloseHandle(handle: *mut c_void) -> i32;
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
}
#[link(name = "user32")]
extern "system" {
    fn RegisterClassW(class: *const WindowClass) -> u16;
    fn UnregisterClassW(name: *const u16, instance: *mut c_void) -> i32;
    fn CreateWindowExW(exstyle: u32, class: *const u16, name: *const u16, style: u32, x: i32, y: i32, width: i32, height: i32, parent: *mut c_void, menu: *mut c_void, instance: *mut c_void, parameter: *const c_void) -> *mut c_void;
    fn DestroyWindow(window: *mut c_void) -> i32;
    fn DefWindowProcW(window: *mut c_void, message: u32, wparam: usize, lparam: isize) -> isize;
    fn PeekMessageW(message: *mut Message, window: *mut c_void, min: u32, max: u32, remove: u32) -> i32;
    fn TranslateMessage(message: *const Message) -> i32;
    fn DispatchMessageW(message: *const Message) -> isize;
    fn MsgWaitForMultipleObjectsEx(count: u32, handles: *const *mut c_void, milliseconds: u32, wake_mask: u32, flags: u32) -> u32;
}
#[link(name = "Everything", kind = "static")]
extern "system" {
    fn SuperSearch_SetIpcTimeout(milliseconds: u32);
    fn SuperSearch_IpcFailed() -> i32;
    fn SuperSearch_ValidateReply(parameter: isize, reply_id: u32) -> i32;
    #[cfg(test)]
    fn SuperSearch_ValidatePayload(data: *const u8, size: u32, version: u32, offset: u32, maximum: u32) -> i32;
    fn Everything_SetInstanceNameW(instance: *const u16) -> i32;
    #[cfg(test)]
    fn SuperSearch_InstanceWindowClass(instance: *const u16, output: *mut u16, capacity: u32) -> i32;
    fn Everything_Reset();
    fn Everything_CleanUp();
    fn Everything_SetSearchW(search: *const u16);
    fn Everything_SetMatchPath(enabled: i32);
    fn Everything_SetMatchCase(enabled: i32);
    fn Everything_SetMatchWholeWord(enabled: i32);
    fn Everything_SetRegex(enabled: i32);
    fn Everything_SetSort(sort: u32);
    fn Everything_SetRequestFlags(flags: u32);
    fn Everything_SetMax(max: u32);
    fn Everything_SetOffset(offset: u32);
    fn Everything_SetReplyWindow(window: *mut c_void);
    fn Everything_SetReplyID(id: u32);
    fn Everything_QueryW(wait: i32) -> i32;
    fn Everything_IsQueryReply(message: u32, wparam: usize, lparam: isize, id: u32) -> i32;
    fn Everything_GetNumResults() -> u32;
    fn Everything_GetTotResults() -> u32;
    fn Everything_GetResultFullPathNameW(index: u32, path: *mut u16, capacity: u32) -> u32;
    fn Everything_IsFolderResult(index: u32) -> i32;
    fn Everything_GetMajorVersion() -> u32;
    fn Everything_GetMinorVersion() -> u32;
    fn Everything_IsDBLoaded() -> i32;
    fn Everything_GetLastError() -> u32;
}

fn page_limit(offset: u32) -> Result<u32, String> {
    if offset >= 1000 { return Err("File candidate limit reached.".into()); }
    Ok(200.min(1000 - offset))
}

fn next_reply_id(current: u32) -> Result<u32, String> {
    current.checked_add(1).ok_or_else(|| "File search reply identifiers exhausted. Restart SuperSearch.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manual_restart_status_timeout_preserves_loading_until_worker_replies() {
        let client = EverythingClient { shared: Arc::new(Shared {
            mailbox: Mutex::new(Mailbox { query: None, status: None, cached: provider("ready", "Everything is ready."), options: Some(EngineOptions::new("unused-status-fixture".into(), vec![])), restart: false, diagnostics: json!({"mode":"bundled"}) }),
            stop: AtomicBool::new(false), running: AtomicBool::new(true), wake: 0,
        }) };
        client.retry(vec![]).unwrap();
        let started = Instant::now();
        let status = client.status();
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(status.status, "indexLoading");
        assert!(client.shared.mailbox.lock().unwrap().restart);
        // 模拟重试前的旧查询晚到：待处理重启必须覆盖过期的就绪快照。
        client.shared.mailbox.lock().unwrap().cached = provider("ready", "Everything is ready.");
        assert_eq!(client.cached_status().status, "indexLoading");
    }

    #[test]
    fn channel_deadline_does_not_turn_a_loading_to_ready_transition_into_failure() {
        use std::os::windows::io::{FromRawHandle, OwnedHandle};

        let wake = unsafe { CreateEventW(ptr::null(), 0, 0, ptr::null()) };
        assert!(!wake.is_null());
        let _wake = unsafe { OwnedHandle::from_raw_handle(wake) };
        let client = EverythingClient { shared: Arc::new(Shared {
            mailbox: Mutex::new(Mailbox { query: None, status: None, cached: provider("indexLoading", "Bundled file index is loading."), options: None, restart: false, diagnostics: json!({}) }),
            stop: AtomicBool::new(false), running: AtomicBool::new(true), wake: wake as usize,
        }) };
        std::thread::scope(|scope| {
            let shared = client.shared.clone();
            scope.spawn(move || {
                // 等请求入队后再取邮箱，避免轮询抢锁触发合法的“正在检查”分支。
                use windows::Win32::{Foundation::{HANDLE, WAIT_OBJECT_0}, System::Threading::WaitForSingleObject};
                assert_eq!(unsafe { WaitForSingleObject(HANDLE(shared.wake as *mut c_void), 5000) }, WAIT_OBJECT_0);
                let mut mailbox = shared.mailbox.lock().unwrap();
                let reply = mailbox.status.take().expect("Status request must be queued before wake");
                mailbox.cached = provider("ready", "Everything is ready.");
                // 先释放邮箱再断开回复，保证回退读取能观察到已发布的就绪状态。
                drop(mailbox);
                drop(reply);
            });
            assert_eq!(client.status().status, "ready");
        });
    }

    #[test]
    fn genuine_worker_failure_is_not_changed_into_loading_on_status_timeout() {
        let client = EverythingClient { shared: Arc::new(Shared {
            mailbox: Mutex::new(Mailbox { query: None, status: None, cached: provider("unavailable", "Unable to start bundled file search."), options: None, restart: false, diagnostics: json!({"mode":"external"}) }),
            stop: AtomicBool::new(false), running: AtomicBool::new(true), wake: 0,
        }) };
        assert_eq!(client.status().status, "unavailable");
        client.shared.running.store(false, Ordering::Release);
        assert_eq!(client.status().status, "unavailable");
    }
    #[test]
    fn automatic_recovery_is_bounded_and_manual_retry_resets_the_budget() {
        let now = Instant::now();
        let mut budget = RecoveryBudget::new(now);
        assert!(budget.allow(now));
        assert!(budget.allow(now + Duration::from_secs(1)));
        assert!(!budget.allow(now + Duration::from_secs(2)));
        assert!(!budget.allow(now + Duration::from_secs(29)));
        assert!(budget.allow(now + Duration::from_secs(30)));
        assert!(budget.allow(now + Duration::from_secs(31)));
        assert!(!budget.allow(now + Duration::from_secs(32)));
        budget = RecoveryBudget::new(now + Duration::from_secs(32));
        assert!(budget.allow(now + Duration::from_secs(32)));
    }

    #[test]
    fn status_never_waits_for_busy_sdk_mailbox() {
        let client = EverythingClient { shared: Arc::new(Shared {
            mailbox: Mutex::new(Mailbox { query: None, status: None, cached: provider("ready", "Everything is ready."), options: None, restart: false, diagnostics: json!({"mode":"external"}) }),
            stop: AtomicBool::new(false), running: AtomicBool::new(true), wake: 0,
        }) };
        std::thread::scope(|scope| {
            let mailbox = client.shared.mailbox.lock().unwrap();
            let (tx, rx) = mpsc::sync_channel(1);
            let client = &client;
            scope.spawn(move || { let _ = tx.send(client.status()); });
            let result = rx.recv_timeout(Duration::from_millis(150));
            assert!(mailbox.status.is_none());
            drop(mailbox);
            let status = result.expect("Status submission must not wait for the SDK mailbox");
            assert_eq!(status.status, "loading");
            assert!(status.message.contains("being checked"));
        });
    }

    #[test]
    fn query_never_waits_for_busy_sdk_mailbox_or_replaces_pending_work() {
        let client = EverythingClient { shared: Arc::new(Shared {
            mailbox: Mutex::new(Mailbox { query: None, status: None, cached: provider("ready", "Everything is ready."), options: None, restart: false, diagnostics: json!({"mode":"external"}) }),
            stop: AtomicBool::new(false), running: AtomicBool::new(true), wake: 0,
        }) };
        std::thread::scope(|scope| {
            let mailbox = client.shared.mailbox.lock().unwrap();
            let (tx, rx) = mpsc::sync_channel(1);
            let client = &client;
            scope.spawn(move || { let _ = tx.send(client.query("folder:".into(), 0, Instant::now() + Duration::from_millis(60)).map(|_| ())); });
            let result = rx.recv_timeout(Duration::from_millis(150));
            assert!(mailbox.query.is_none());
            drop(mailbox);
            let error = result.expect("Query deadline must include mailbox acquisition").unwrap_err();
            assert!(error.contains("busy"), "{error}");
        });
    }

    #[test]
    fn instance_class_keeps_default_and_names_only_the_owned_ipc_window() {
        let class = |name: &str| {
            let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
            let mut output = [0u16; 256];
            assert_ne!(unsafe { SuperSearch_InstanceWindowClass(name.as_ptr(), output.as_mut_ptr(), 256) }, 0);
            String::from_utf16(&output[..output.iter().position(|value| *value == 0).unwrap()]).unwrap()
        };
        assert_eq!(class(""), "EVERYTHING_TASKBAR_NOTIFICATION");
        assert_eq!(class("SuperSearch-owned"), "EVERYTHING_TASKBAR_NOTIFICATION_(SuperSearch-owned)");
        let long: Vec<u16> = "x".repeat(129).encode_utf16().chain(Some(0)).collect();
        let mut output = [0u16; 256];
        assert_eq!(unsafe { SuperSearch_InstanceWindowClass(long.as_ptr(), output.as_mut_ptr(), 256) }, 0);
    }

    #[test]
    fn cached_status_does_not_enqueue_sdk_status_request() {
        let client = EverythingClient { shared: Arc::new(Shared {
            mailbox: Mutex::new(Mailbox { query: None, status: None, cached: provider("notRunning", "Start Everything."), options: None, restart: false, diagnostics: json!({"mode":"external"}) }),
            stop: AtomicBool::new(false), running: AtomicBool::new(true), wake: 0,
        }) };
        let status = client.cached_status();
        assert_eq!(status.status, "notRunning");
        assert_eq!(status.message, "Start Everything.");
        let mailbox = client.shared.mailbox.lock().unwrap();
        assert!(mailbox.status.is_none());
        assert!(mailbox.query.is_none());
    }

    #[test]
    fn cached_status_never_waits_for_sdk_mailbox() {
        let client = EverythingClient { shared: Arc::new(Shared {
            mailbox: Mutex::new(Mailbox { query: None, status: None, cached: provider("ready", "Everything is ready."), options: None, restart: false, diagnostics: json!({"mode":"external"}) }),
            stop: AtomicBool::new(false), running: AtomicBool::new(true), wake: 0,
        }) };
        std::thread::scope(|scope| {
            let mailbox = client.shared.mailbox.lock().unwrap();
            let (tx, rx) = mpsc::sync_channel(1);
            let client = &client;
            scope.spawn(move || { let _ = tx.send(client.cached_status()); });
            let result = rx.recv_timeout(Duration::from_secs(1));
            drop(mailbox);
            let status = result.expect("Cached status must not wait for an SDK status request");
            assert_eq!(status.status, "loading");
            assert!(status.message.contains("being checked"));
        });
    }

    #[test]
    fn pagination_never_exceeds_total_candidate_budget() {
        assert_eq!(page_limit(0).unwrap(), 200);
        assert_eq!(page_limit(999).unwrap(), 1);
        assert!(page_limit(1000).is_err());
        assert!(page_limit(u32::MAX).is_err());
    }

    #[test]
    fn reply_identifiers_never_wrap_or_reuse() {
        assert_eq!(next_reply_id(0).unwrap(), 1);
        assert_eq!(next_reply_id(100).unwrap(), 101);
        assert!(next_reply_id(u32::MAX).is_err());
    }

    #[test]
    fn ipc_payload_rejects_untrusted_lengths_offsets_and_missing_terminators() {
        let mut payload = vec![0u8; 52];
        for (offset, value) in [(4, 1u32), (8, 1), (16, 1), (20, 1), (32, 40), (36, 44)] {
            payload[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        payload[40..44].copy_from_slice(&[b'x', 0, 0, 0]);
        payload[44..52].copy_from_slice(&[b'C', 0, b':', 0, b'\\', 0, 0, 0]);
        let check = |bytes: &[u8]| unsafe { SuperSearch_ValidatePayload(bytes.as_ptr(), bytes.len() as u32, 1, 0, 200) != 0 };
        assert!(check(&payload));
        assert!(!check(&payload[..20]));
        let mut bad = payload.clone(); bad[20..24].copy_from_slice(&201u32.to_le_bytes()); assert!(!check(&bad));
        let mut bad = payload.clone(); bad[32..36].copy_from_slice(&u32::MAX.to_le_bytes()); assert!(!check(&bad));
        let mut bad = payload.clone(); bad[28..32].copy_from_slice(&128u32.to_le_bytes()); assert!(!check(&bad));
        let mut bad = payload.clone(); bad[50] = 1; assert!(!check(&bad));
        let mut bad = payload.clone(); bad[24..28].copy_from_slice(&1u32.to_le_bytes()); assert!(!check(&bad));
        assert_eq!(unsafe { SuperSearch_ValidatePayload(payload.as_ptr(), 17 * 1024 * 1024, 1, 0, 200) }, 0);
    }

    #[test]
    #[ignore = "Requires a running Everything client and the project's native smoke fixture."]
    fn live_everything_fixture_probe() {
        let client = EverythingClient::new();
        let status = client.status();
        assert_eq!(status.status, "ready", "{}", status.message);
        let root = std::env::current_dir().unwrap().parent().unwrap().join(".tools/smoke-fixtures");
        let path_filter = format!("\"{}\\\"", root.display());
        let probes = [
            ("literal", "\"超级搜索验收报告\"".to_string()),
            ("file", "file: \"超级搜索验收报告\"".to_string()),
            ("directory", format!("file: \"超级搜索验收报告\" {path_filter}")),
            ("hidden", "file: \"超级搜索验收报告\" !attrib:H".to_string()),
            ("system", "file: \"超级搜索验收报告\" !attrib:S".to_string()),
        ];
        for (name, expression) in probes {
            let page = client.query(expression, 0, Instant::now() + Duration::from_millis(250)).unwrap();
            println!("probe={name} count={} total={} status={}", page.entries.len(), page.total, page.status.status);
        }
    }
}
