use crate::model::ProviderStatus;
use serde_json::{json, Value};
use std::{ffi::{c_void, OsString}, fs, os::windows::{process::CommandExt, io::AsRawHandle, fs::MetadataExt}, path::{Path, PathBuf}, process::{Child, Command, Stdio}, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};

pub const VERSION: &str = "1.4.1.1032";
const BINARY: &[u8] = include_bytes!("../resources/everything/Everything.exe");
const LICENSE: &[u8] = include_bytes!("../resources/everything/License.txt");
pub const SERVICE_NAME: &str = "Everything (SuperSearch)";
const SERVICE_PIPE: &str = r"\\.\PIPE\SuperSearch Everything Service";

#[derive(Clone)]
pub struct EngineOptions {
    pub directory: PathBuf,
    pub scope: Vec<String>,
    pub instance: String,
    pub force_folders: bool,
}

impl EngineOptions {
    pub fn new(directory: PathBuf, scope: Vec<String>) -> Self {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
        Self { directory: directory.join("engine").join(VERSION), scope, instance: format!("SuperSearch-{}-{nonce}", std::process::id()), force_folders: false }
    }
}

pub struct ManagedEngine {
    child: Child,
    options: EngineOptions,
    job: usize,
    fast: bool,
    startup_deadline: Option<Instant>,
}

impl ManagedEngine {
    pub fn start(mut options: EngineOptions) -> Result<Self, String> {
        if options.scope.is_empty() { options.scope = fixed_drive_roots(); }
        options.scope.retain(|path| crate::query::local_directory(path).is_ok() && fs::read_dir(path).is_ok());
        let fast = !options.force_folders && service_running();
        let ntfs = fixed_drive_roots().into_iter().filter(|root| is_ntfs(root)).collect::<Vec<_>>();
        let ini = indexing_configuration(&options.scope, fast, &ntfs)?;
        fs::create_dir_all(&options.directory).map_err(|e| format!("Unable to prepare bundled file search: {e}"))?;
        extract_exact(&options.directory.join("Everything.exe"), BINARY)?;
        extract_exact(&options.directory.join("License.txt"), LICENSE)?;
        fs::write(options.directory.join("Everything.ini"), ini).map_err(|e| format!("Unable to prepare bundled file search: {e}"))?;
        // 独立进程名和目录只属于本次 SuperSearch；从不修改已有 Everything 的配置。
        let job = unsafe { SuperSearch_CreateEngineJob() } as usize;
        if job == 0 { return Err("Unable to create the bundled file search lifecycle guard.".into()); }
        let child = command(&options.directory.join("Everything.exe"))
            .args(launch_arguments(&options.instance, &options.directory)).current_dir(&options.directory).spawn();
        let mut child = match child {
            Ok(child) => child,
            Err(error) => { unsafe { CloseHandle(job as *mut c_void); } return Err(format!("Unable to start bundled file search: {error}")); }
        };
        if unsafe { SuperSearch_AssignEngineJob(job as *mut c_void, child.as_raw_handle()) } == 0 {
            let _ = child.kill(); let _ = child.wait();
            unsafe { CloseHandle(job as *mut c_void); }
            return Err("Unable to guard the bundled file search process.".into());
        }
        Ok(Self { child, options, job, fast, startup_deadline: Some(Instant::now() + Duration::from_secs(30)) })
    }

    pub fn needs_restart(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(Some(_))) || service_mode_changed(self.fast, self.options.force_folders, service_running())
    }

    pub fn status(&mut self, mut status: ProviderStatus) -> ProviderStatus {
        if self.fast && !service_running() {
            status.status = "unavailable".into();
            status.message = "Fast indexing service stopped. Restart the bundled engine to use folder fallback.".into();
            return status;
        }
        match self.child.try_wait() {
            Ok(Some(_)) => { status.status = "unavailable".into(); status.message = "Bundled file search stopped unexpectedly. Retry the bundled engine in Settings.".into(); return status; }
            Err(_) => { status.status = "unavailable".into(); status.message = "Bundled file search process status is unavailable.".into(); return status; }
            _ => {}
        }
        status = startup_status(status, &mut self.startup_deadline, Instant::now());
        status.message = match status.status.as_str() {
            "ready" if self.fast => "Bundled file search is ready. NTFS/USN fast indexing service is active.".into(),
            "ready" => "Bundled file search is ready. Folder fallback is active; install the machine setup to enable NTFS/USN fast indexing.".into(),
            "notRunning" | "indexLoading" if self.fast => { status.status = "indexLoading".into(); "NTFS/USN fast index is loading.".into() },
            "notRunning" | "indexLoading" => { status.status = "indexLoading".into(); "Bundled file index is loading. Folder fallback may take several minutes; install the machine setup for fast indexing.".into() },
            _ => status.message.replace("Everything", "Bundled file search"),
        };
        status
    }

    pub fn diagnostics(&self) -> Value {
        json!({"mode":"bundled", "version":VERSION, "indexing":if self.fast { "ntfs-usn" } else { "folders" }, "serviceName":SERVICE_NAME, "serviceRunning":service_running(), "instance":self.options.instance, "scope":self.options.scope, "processId":self.child.id()})
    }
}

fn startup_status(mut status: ProviderStatus, deadline: &mut Option<Instant>, now: Instant) -> ProviderStatus {
    // 启动早期仅把 SDK 超时视为加载；进程/服务故障已先检查，就绪或超期后保留真实超时。
    if status.status == "ready" { *deadline = None; }
    if status.status == "unavailable" && status.message == "Everything IPC timed out."
        && deadline.is_some_and(|until| now < until) {
        status.status = "indexLoading".into();
    }
    status
}

fn service_mode_changed(fast: bool, force_folders: bool, available: bool) -> bool {
    // 服务恢复后按需切回高速索引；隔离诊断始终保留显式目录模式。
    !force_folders && fast != available
}

impl Drop for ManagedEngine {
    fn drop(&mut self) {
        // 通过已验证的自有 PID 发退出 IPC；兜底只终止持有的子进程句柄。
        unsafe { SuperSearch_StopOwnedEngine(self.options.instance.encode_utf16().chain(Some(0)).collect::<Vec<_>>().as_ptr(), self.child.id()); }
        let deadline = Instant::now() + Duration::from_millis(800);
        while matches!(self.child.try_wait(), Ok(None)) && Instant::now() < deadline { std::thread::sleep(Duration::from_millis(20)); }
        if !matches!(self.child.try_wait(), Ok(Some(_))) { let _ = self.child.kill(); let _ = self.child.wait(); }
        unsafe { CloseHandle(self.job as *mut c_void); }
    }
}

fn command(path: &Path) -> Command {
    let mut cmd = Command::new(path);
    cmd.creation_flags(0x08000000).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    cmd
}

#[cfg(test)]
fn ini_configuration(scope: &[String]) -> Result<String, String> {
    indexing_configuration(scope, false, &[])
}

fn indexing_configuration(scope: &[String], fast: bool, ntfs_roots: &[String]) -> Result<String, String> {
    if scope.is_empty() || scope.iter().any(|path| crate::query::local_directory(path).is_err()) {
        return Err("No valid local folders are available for bundled file search.".into());
    }
    // NTFS 由卷索引覆盖；只对其它文件系统扫描目录，避免同一目录重复索引。
    let scope = scope.iter().filter(|path| !fast || needs_folder_index(path, ntfs_roots)).collect::<Vec<_>>();
    let folders = scope.iter().map(|path| format!("\"{}\"", path.replace('\\', "\\\\"))).collect::<Vec<_>>().join(",");
    let monitors = vec!["1"; scope.len()].join(",");
    let updates = vec!["1"; scope.len()].join(",");
    let intervals = vec!["15"; scope.len()].join(",");
    let units = vec!["0"; scope.len()].join(",");
    let volumes = if fast { 1 } else { 0 };
    let pipe = if fast { SERVICE_PIPE } else { r"\\.\PIPE\SuperSearch-No-Service" };
    Ok(format!("[Everything]\napp_data=0\nrun_as_admin=0\nrun_in_background=1\nshow_tray_icon=0\ncheck_for_updates_on_startup=0\nsearch_history_enabled=0\nrun_history_enabled=0\nhttp_server_enabled=0\netp_server_enabled=0\nauto_include_fixed_volumes={volumes}\nauto_include_removable_volumes=0\nauto_include_fixed_refs_volumes=0\nauto_include_removable_refs_volumes=0\nntfs_volume_guids=\nntfs_volume_paths=\nntfs_volume_includes=\nrefs_volume_guids=\nrefs_volume_paths=\nrefs_volume_includes=\nservice_pipe_name={pipe}\nfolders={folders}\nfolder_monitor_changes={monitors}\nfolder_update_types={updates}\nfolder_update_intervals={intervals}\nfolder_update_interval_types={units}\nindex_attributes=1\nexclude_hidden_files_and_folders=0\nexclude_system_files_and_folders=0\n"))
}

pub fn service_running() -> bool {
    // 只查询专属服务，不启动、停止或复用用户原有的 Everything 服务。
    unsafe { SuperSearch_FastServiceRunning() != 0 }
}

fn needs_folder_index(path: &str, ntfs_roots: &[String]) -> bool {
    // 显式目录经重解析点进入其它位置时，卷索引的原始路径不能满足原目录筛选。
    let Ok(normalized) = crate::query::local_directory(path) else { return true; };
    !ntfs_roots.iter().any(|root| normalized.to_ascii_lowercase().starts_with(&root.to_ascii_lowercase()))
        || Path::new(&normalized).ancestors().any(|part| {
            if !fs::symlink_metadata(part).is_ok_and(|metadata| metadata.file_attributes() & 0x400 != 0) { return false; }
            let wide: Vec<u16> = part.as_os_str().to_string_lossy().encode_utf16().chain(Some(0)).collect();
            unsafe { SuperSearch_DirectoryRedirects(wide.as_ptr()) != 0 }
        })
}

fn is_ntfs(root: &str) -> bool {
    let wide: Vec<u16> = root.encode_utf16().chain(Some(0)).collect();
    let mut name = [0u16; 32];
    (unsafe { GetVolumeInformationW(wide.as_ptr(), std::ptr::null_mut(), 0, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), name.as_mut_ptr(), name.len() as u32) != 0 })
        && String::from_utf16_lossy(&name[..name.iter().position(|value| *value == 0).unwrap_or(name.len())]).eq_ignore_ascii_case("NTFS")
}

fn launch_arguments(instance: &str, directory: &Path) -> Vec<OsString> {
    vec!["-startup".into(), "-instance".into(), instance.into(), "-config".into(), directory.join("Everything.ini").into_os_string(), "-db".into(), directory.join("Everything.db").into_os_string()]
}

fn extract_exact(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if fs::read(path).ok().as_deref() != Some(bytes) { fs::write(path, bytes).map_err(|e| format!("Unable to prepare bundled file search: {e}"))?; }
    if fs::read(path).map_err(|e| format!("Unable to verify bundled file search: {e}"))? != bytes { return Err("Bundled file search integrity check failed.".into()); }
    Ok(())
}

fn fixed_drive_roots() -> Vec<String> {
    let drives = unsafe { GetLogicalDrives() };
    (0..26).filter_map(|index| {
        if drives & (1 << index) == 0 { return None; }
        let root = format!("{}:\\", (b'A' + index) as char);
        let wide: Vec<u16> = root.encode_utf16().chain(Some(0)).collect();
        (unsafe { GetDriveTypeW(wide.as_ptr()) } == 3 && fs::read_dir(&root).is_ok()).then_some(root)
    }).collect()
}

#[link(name = "kernel32")]
extern "system" {
    fn GetLogicalDrives() -> u32;
    fn GetDriveTypeW(root: *const u16) -> u32;
    fn GetVolumeInformationW(root: *const u16, volume: *mut u16, volume_size: u32, serial: *mut u32, maximum: *mut u32, flags: *mut u32, filesystem: *mut u16, filesystem_size: u32) -> i32;
    fn CloseHandle(handle: *mut c_void) -> i32;
}
#[link(name = "Everything", kind = "static")]
extern "system" {
    fn SuperSearch_CreateEngineJob() -> *mut c_void;
    fn SuperSearch_AssignEngineJob(job: *mut c_void, process: *mut c_void) -> i32;
    fn SuperSearch_StopOwnedEngine(instance: *const u16, pid: u32) -> i32;
    fn SuperSearch_FastServiceRunning() -> i32;
    #[cfg(test)]
    fn SuperSearch_ReparseTagRedirects(tag: u32) -> i32;
    fn SuperSearch_DirectoryRedirects(path: *const u16) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_ipc_timeouts_have_bounded_grace_and_ready_ends_grace() {
        let now = Instant::now();
        let mut deadline = Some(now + Duration::from_secs(30));
        let timeout = || ProviderStatus { id: "everything".into(), status: "unavailable".into(), message: "Everything IPC timed out.".into() };
        assert_eq!(startup_status(timeout(), &mut deadline, now).status, "indexLoading");
        let failed = ProviderStatus { message: "Unable to start bundled file search.".into(), ..timeout() };
        assert_eq!(startup_status(failed, &mut deadline, now).status, "unavailable");
        assert_eq!(startup_status(timeout(), &mut deadline, now + Duration::from_secs(30)).status, "unavailable");
        let ready = ProviderStatus { status: "ready".into(), message: "Everything is ready.".into(), ..timeout() };
        assert_eq!(startup_status(ready, &mut deadline, now).status, "ready");
        assert_eq!(startup_status(timeout(), &mut deadline, now).status, "unavailable");
    }

    #[test]
    fn service_restoration_upgrades_fallback_on_demand_but_preserves_isolated_scope() {
        assert!(service_mode_changed(false, false, true));
        assert!(service_mode_changed(true, false, false));
        assert!(!service_mode_changed(false, false, false));
        assert!(!service_mode_changed(true, false, true));
        assert!(!service_mode_changed(false, true, true));
        assert!(!service_mode_changed(false, true, false));
    }
    use std::sync::atomic::{AtomicU64, Ordering};
    static ID: AtomicU64 = AtomicU64::new(0);
    #[test]
    fn reparse_fallback_distinguishes_path_links_from_cloud_placeholders() {
        unsafe {
            assert_ne!(SuperSearch_ReparseTagRedirects(0xA000000C), 0);
            assert_ne!(SuperSearch_ReparseTagRedirects(0xA0000003), 0);
            assert_eq!(SuperSearch_ReparseTagRedirects(0x9000001A), 0);
            assert_eq!(SuperSearch_ReparseTagRedirects(0x80000023), 0);
        }
    }
    #[test]
    fn fast_indexing_uses_private_pipe_and_avoids_ntfs_folder_scan() {
        let ini = indexing_configuration(&[r"C:\Work".into(), r"D:\Archive".into()], true, &[r"C:\".into()]).unwrap();
        assert!(ini.lines().any(|line| line == "auto_include_fixed_volumes=1"));
        assert!(ini.contains(r"service_pipe_name=\\.\PIPE\SuperSearch Everything Service"));
        assert!(ini.contains(r#"folders="D:\\Archive""#));
        assert!(!ini.contains(r#"folders="C:\\Work"#));
        assert!(ini.contains("run_as_admin=0\n"));
        assert!(ini.contains("show_tray_icon=0\n"));
        let volumes_only = indexing_configuration(&[r"C:\Work".into()], true, &[r"C:\".into()]).unwrap();
        assert!(volumes_only.contains("folders=\n"));
        assert!(volumes_only.contains("folder_monitor_changes=\n"));
    }
    #[test]
    fn missing_service_retains_all_folder_scopes_and_blocks_bad_input() {
        let ini = indexing_configuration(&[r"C:\Work".into(), r"D:\Archive".into()], false, &[r"C:\".into()]).unwrap();
        assert!(ini.contains(r#"folders="C:\\Work","D:\\Archive""#));
        assert!(ini.contains("auto_include_fixed_volumes=0\n"));
        assert!(indexing_configuration(&["C:\\Work\nrun_as_admin=1".into()], true, &[r"C:\".into()]).is_err());
        assert!(indexing_configuration(&[], false, &[]).is_err());
    }
    #[test]
    fn portable_configuration_is_hidden_offline_and_never_elevates() {
        let ini = ini_configuration(&[r"C:\Work".into()]).unwrap();
        for setting in ["app_data=0", "run_as_admin=0", "show_tray_icon=0", "http_server_enabled=0", "etp_server_enabled=0", "auto_include_fixed_volumes=0", "folder_monitor_changes=1", "index_attributes=1"] {
            assert!(ini.lines().any(|line| line == setting), "Missing {setting}");
        }
        assert!(ini.contains(r#"folders="C:\\Work""#));
    }
    #[test]
    fn folder_lists_preserve_commas_and_reject_ini_injection() {
        let ini = ini_configuration(&[r"C:\A,B".into(), r"D:\中文".into()]).unwrap();
        assert!(ini.contains(r#"folders="C:\\A,B","D:\\中文""#));
        assert!(ini.contains("folder_monitor_changes=1,1"));
        assert!(ini_configuration(&["C:\\Work\nrun_as_admin=1".into()]).is_err());
        assert!(ini_configuration(&["C:\\Work\0".into()]).is_err());
        assert!(ini_configuration(&[]).is_err());
        assert!(ini_configuration(&[r"\\server\share".into()]).is_err());
        assert!(ini_configuration(&[r"C:\Work\..\other".into()]).is_err());
    }
    #[test]
    fn configuration_never_enables_volume_or_service_indexing() {
        let ini = ini_configuration(&[r"C:\Work".into()]).unwrap();
        for setting in ["auto_include_fixed_volumes=0", "auto_include_removable_volumes=0", "auto_include_fixed_refs_volumes=0", "auto_include_removable_refs_volumes=0", r"service_pipe_name=\\.\PIPE\SuperSearch-No-Service"] {
            assert!(ini.lines().any(|line| line == setting), "Unsafe indexing configuration: missing {setting}");
        }
        assert!(!ini.contains(r"\\.\PIPE\Everything Service"));
        assert!(ini.contains("ntfs_volume_includes=\n"));
        assert!(ini.contains("refs_volume_includes=\n"));
    }
    #[test]
    fn launch_always_scopes_instance_config_and_database() {
        let args: Vec<_> = launch_arguments("SuperSearch-owned", Path::new(r"C:\App Data\engine")).into_iter().map(|value| value.to_string_lossy().into_owned()).collect();
        assert_eq!(args, vec!["-startup", "-instance", "SuperSearch-owned", "-config", r"C:\App Data\engine\Everything.ini", "-db", r"C:\App Data\engine\Everything.db"]);
    }
    #[test]
    fn extraction_repairs_corruption_even_when_length_matches() {
        let path: PathBuf = std::env::temp_dir().join(format!("supersearch-engine-{}-{}", std::process::id(), ID.fetch_add(1, Ordering::Relaxed)));
        std::fs::write(&path, b"evil").unwrap();
        extract_exact(&path, b"safe").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"safe");
        extract_exact(&path, b"safe").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"safe");
        std::fs::remove_file(path).unwrap();
    }
}
