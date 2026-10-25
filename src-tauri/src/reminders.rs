use crate::service::AppState;
use serde::Serialize;
use std::{sync::{atomic::{AtomicBool, Ordering}, Arc, Condvar, Mutex}, time::{Duration, SystemTime, UNIX_EPOCH}};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Reminder {
    pub id: String,
    pub title: String,
    pub due_at: u64,
    pub completed: bool,
    pub notification_error: bool,
}

pub(crate) fn validate_input(seconds: u64, title: &str) -> Result<String, String> {
    if !(1..=604_800).contains(&seconds) { return Err("Reminder duration must be between 1 second and 7 days".into()); }
    if title.chars().any(char::is_control) || title.chars().count() > 100 {
        return Err("Reminder title must contain at most 100 characters and no control characters".into());
    }
    let title = title.trim();
    Ok(if title.is_empty() { "倒计时结束" } else { title }.into())
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis().min(u64::MAX as u128) as u64
}

#[tauri::command]
pub fn create_reminder(app: AppHandle, state: State<'_, AppState>, seconds: u64, title: String) -> Result<Reminder, String> {
    let reminder = state.store.lock().map_err(|_| "Reminder storage is unavailable")?.create_reminder(seconds, &title, now_ms())?;
    changed(&app);
    Ok(reminder)
}

#[tauri::command]
pub fn list_reminders(state: State<'_, AppState>) -> Result<Vec<Reminder>, String> {
    let store = state.store.lock().map_err(|_| "Reminder storage is unavailable")?;
    store.require_reminders_enabled()?;
    store.reminder_list()
}

#[tauri::command]
pub fn cancel_reminder(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.store.lock().map_err(|_| "Reminder storage is unavailable")?.cancel_reminder(&id)?;
    changed(&app);
    Ok(())
}

#[derive(Default)]
struct Signal {
    generation: Mutex<u64>,
    wake: Condvar,
    started: AtomicBool,
}

#[derive(Default)]
pub struct SchedulerState(Arc<Signal>);

pub fn changed(app: &AppHandle) {
    if let Some(scheduler) = app.try_state::<SchedulerState>() {
        if let Ok(mut generation) = scheduler.0.generation.lock() {
            *generation = generation.wrapping_add(1);
            scheduler.0.wake.notify_one();
        }
    }
    let _ = app.emit("reminders_changed", ());
}

fn wait_duration(next_due: Option<u64>, now: u64) -> Option<Duration> {
    // 仅有待执行提醒时有界等待，以便睡眠恢复或系统时钟变化后重新计算。
    next_due.map(|due| Duration::from_millis(due.saturating_sub(now).clamp(1, 30_000)))
}

pub fn start(app: AppHandle) {
    let signal = app.state::<SchedulerState>().0.clone();
    if signal.started.swap(true, Ordering::AcqRel) { return; }
    std::thread::spawn(move || {
        let mut startup = true;
        loop {
            let generation = match signal.generation.lock() { Ok(value) => *value, Err(_) => return };
            let batch = {
                let state = app.state::<AppState>();
                let Ok(mut store) = state.store.lock() else { return; };
                if !store.settings().reminders_enabled {
                    // 启动时同时清理上次禁用后意外退出所遗留的待执行项。
                    let _ = store.disable_pending_reminders();
                    Ok((vec![], None))
                } else {
                    store.claim_due_reminders(now_ms()).and_then(|due| store.next_reminder_due().map(|next| (due, next)))
                }
            };
            let next_due = match batch {
                Ok((due, next)) => {
                    if !due.is_empty() {
                        notify_due(&app, &due, startup);
                        let _ = app.emit("reminders_changed", ());
                    }
                    next
                }
                Err(error) => { eprintln!("Reminder scheduling failed: {error}"); None }
            };
            startup = false;
            let guard = match signal.generation.lock() { Ok(value) => value, Err(_) => return };
            // 计算期间若有创建或取消，直接重算，避免通知发生在等待前造成丢失唤醒。
            if *guard != generation { continue; }
            if let Some(duration) = wait_duration(next_due, now_ms()) {
                if signal.wake.wait_timeout(guard, duration).is_err() { return; }
            } else if signal.wake.wait(guard).is_err() { return; }
        }
    });
}

fn notify_due(app: &AppHandle, due: &[Reminder], startup: bool) {
    // 通知调用在存储锁外执行；系统关闭通知时，完成记录仍可从列表读取。
    let groups: Vec<&[Reminder]> = if startup { vec![due] } else { due.chunks(1).collect() };
    for group in groups {
        let body = if startup {
            format!("有 {} 个提醒已到期：{}", group.len(), group.iter().take(3).map(|r| r.title.as_str()).collect::<Vec<_>>().join("、"))
        } else { group[0].title.clone() };
        let outcome = app.path().app_local_data_dir().map_err(|_| "Notification storage is unavailable".to_string())
            .and_then(|directory| crate::notifications::show(&directory, &body));
        if outcome.is_err() {
            let state = app.state::<AppState>();
            if let Ok(mut store) = state.store.lock() {
                for reminder in group { let _ = store.reminder_notification_failed(&reminder.id); }
            };
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn reminder_duration_and_title_limits_are_enforced() {
        assert!(super::validate_input(0, "休息").is_err());
        assert!(super::validate_input(604_801, "休息").is_err());
        assert!(super::validate_input(1, &"中".repeat(101)).is_err());
        assert!(super::validate_input(1, "休息\n提示").is_err());
        assert!(super::validate_input(604_800, &"中".repeat(100)).is_ok());
        assert_eq!(super::validate_input(1, "  ").unwrap(), "倒计时结束");
    }
    #[test]
    fn idle_scheduler_has_no_poll_and_active_wait_is_bounded_for_resume() {
        assert_eq!(super::wait_duration(None, 100), None);
        assert_eq!(super::wait_duration(Some(110), 100), Some(std::time::Duration::from_millis(10)));
        assert_eq!(super::wait_duration(Some(1_000_000), 100), Some(std::time::Duration::from_secs(30)));
        assert_eq!(super::wait_duration(Some(99), 100), Some(std::time::Duration::from_millis(1)));
    }
}
