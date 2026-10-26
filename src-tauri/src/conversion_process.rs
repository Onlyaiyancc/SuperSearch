use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::Read,
    os::windows::{
        fs::{MetadataExt, OpenOptionsExt},
        io::AsRawHandle,
    },
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};
use windows::{
    core::{PCWSTR, PWSTR},
    Win32::{
        Foundation::{
            CloseHandle, SetHandleInformation, HANDLE, HANDLE_FLAGS, HANDLE_FLAG_INHERIT,
            WAIT_OBJECT_0, WAIT_TIMEOUT,
        },
        System::{
            JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JobObjectBasicAccountingInformation,
                JobObjectExtendedLimitInformation, QueryInformationJobObject,
                SetInformationJobObject, TerminateJobObject,
                JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JOB_OBJECT_LIMIT_JOB_MEMORY, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            },
            Threading::{
                CreateProcessW, DeleteProcThreadAttributeList, GetExitCodeProcess,
                InitializeProcThreadAttributeList, ResumeThread, UpdateProcThreadAttribute,
                WaitForSingleObject, CREATE_NO_WINDOW, CREATE_SUSPENDED,
                CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT,
                LPPROC_THREAD_ATTRIBUTE_LIST, PROCESS_INFORMATION,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST, STARTF_USESTDHANDLES, STARTUPINFOEXW,
            },
        },
    },
};

pub struct ProcessOutput {
    pub stdout: Vec<u8>,
}

pub fn run(
    executable: &Path,
    arguments: &[OsString],
    directory: &Path,
    input: Option<&[u8]>,
    cancel: &AtomicBool,
    timeout: Duration,
) -> Result<ProcessOutput, String> {
    if cancel.load(Ordering::Acquire) {
        return Err("Conversion cancelled".into());
    }
    if !executable.is_absolute() || !executable.is_file() || !directory.is_dir() {
        return Err("Conversion component unavailable".into());
    }
    if input.is_some_and(|data| data.len() > 64 * 1024) {
        return Err("Conversion invalid process input".into());
    }
    let serial = PROCESS_SERIAL.fetch_add(1, Ordering::Relaxed);
    let scratch = directory.join(format!(".process-{}-{serial}", std::process::id()));
    fs::create_dir(&scratch).map_err(|_| "Conversion process storage unavailable")?;
    let _scratch = Scratch(scratch.clone());
    let _directory_release = DirectoryRelease(directory.to_path_buf());
    let input_path = scratch.join("input");
    fs::write(&input_path, input.unwrap_or_default())
        .map_err(|_| "Conversion process storage unavailable")?;
    let stdin = File::open(input_path).map_err(|_| "Conversion process storage unavailable")?;
    let stdout = process_file(&scratch.join("stdout"))?;
    let stderr = process_file(&scratch.join("stderr"))?;
    let handles = [
        file_handle(&stdin),
        file_handle(&stdout),
        file_handle(&stderr),
    ];
    let spawn_guard = PROCESS_SPAWN
        .lock()
        .map_err(|_| "Conversion process handles unavailable")?;
    let _inheritable = Inheritable::new(handles)?;
    let attributes = Attributes::new(&handles)?;
    let mut command = std::iter::once(executable.as_os_str().to_os_string())
        .chain(arguments.iter().cloned())
        .map(|value| {
            value
                .to_str()
                .filter(|value| !value.contains('\0'))
                .ok_or("Conversion invalid process argument")
                .map(quote_argument)
        })
        .collect::<Result<Vec<_>, _>>()?
        .join(" ")
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let application = wide(
        executable
            .as_os_str()
            .to_str()
            .ok_or("Conversion invalid executable")?,
    );
    let current = wide(
        directory
            .as_os_str()
            .to_str()
            .ok_or("Conversion invalid directory")?,
    );
    let mut startup = STARTUPINFOEXW::default();
    startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = handles[0];
    startup.StartupInfo.hStdOutput = handles[1];
    startup.StartupInfo.hStdError = handles[2];
    startup.lpAttributeList = attributes.list;
    let mut process = PROCESS_INFORMATION::default();
    let job = Job::new()?;
    unsafe {
        CreateProcessW(
            PCWSTR(application.as_ptr()),
            Some(PWSTR(command.as_mut_ptr())),
            None,
            None,
            true,
            CREATE_NO_WINDOW
                | CREATE_SUSPENDED
                | CREATE_UNICODE_ENVIRONMENT
                | EXTENDED_STARTUPINFO_PRESENT,
            None,
            PCWSTR(current.as_ptr()),
            &mut startup.StartupInfo,
            &mut process,
        )
        .map_err(|_| "Conversion component could not start")?;
    }
    let process_handle = OwnedHandle(process.hProcess);
    let thread_handle = OwnedHandle(process.hThread);
    unsafe {
        // 子进程保持挂起直到加入作业，避免它提前派生不受取消约束的子进程。
        if AssignProcessToJobObject(job.0 .0, process_handle.0).is_err() {
            let _ = windows::Win32::System::Threading::TerminateProcess(process_handle.0, 1);
            return Err("Conversion process isolation unavailable".into());
        }
        if ResumeThread(thread_handle.0) == u32::MAX {
            return Err("Conversion component could not start".into());
        }
    }
    drop(_inheritable);
    drop(spawn_guard);
    let started = Instant::now();
    let mut checked = Instant::now() - Duration::from_secs(1);
    loop {
        if cancel.load(Ordering::Acquire) {
            return Err("Conversion cancelled".into());
        }
        if started.elapsed() >= timeout {
            return Err("Conversion timeout".into());
        }
        if checked.elapsed() >= Duration::from_millis(200) {
            workspace_bytes(directory)?;
            checked = Instant::now();
        }
        if stdout
            .metadata()
            .map_err(|_| "Conversion process output unavailable")?
            .len()
            > OUTPUT_LIMIT
            || stderr
                .metadata()
                .map_err(|_| "Conversion process output unavailable")?
                .len()
                > OUTPUT_LIMIT
        {
            return Err("Conversion process output limit".into());
        }
        let wait = unsafe { WaitForSingleObject(process_handle.0, 40) };
        if wait == WAIT_OBJECT_0 {
            break;
        }
        if wait != WAIT_TIMEOUT {
            return Err("Conversion process wait failed".into());
        }
    }
    if cancel.load(Ordering::Acquire) {
        return Err("Conversion cancelled".into());
    }
    workspace_bytes(directory)?;
    let mut code = 1;
    unsafe {
        GetExitCodeProcess(process_handle.0, &mut code)
            .map_err(|_| "Conversion process exit unavailable")?;
    }
    // 终止仍存活的辅助子进程，再读取有界结果；错误日志不会传出文档路径或正文。
    drop(job);
    let mut data = Vec::new();
    File::open(scratch.join("stdout"))
        .map_err(|_| "Conversion process output unavailable")?
        .take(OUTPUT_LIMIT + 1)
        .read_to_end(&mut data)
        .map_err(|_| "Conversion process output unavailable")?;
    if data.len() as u64 > OUTPUT_LIMIT {
        return Err("Conversion process output limit".into());
    }
    if code != 0 {
        // helper 使用固定错误标识；只允许有界 ASCII 错误，避免传播其输入内容。
        if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&data) {
            if let Some(error) = value["error"].as_str().filter(|value| {
                value.starts_with("Conversion ")
                    && value.len() <= 160
                    && value
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.'))
            }) {
                return Err(error.to_string());
            }
        }
        return Err("Conversion engine failed".into());
    }
    Ok(ProcessOutput { stdout: data })
}

const OUTPUT_LIMIT: u64 = 1024 * 1024;
static PROCESS_SERIAL: AtomicU64 = AtomicU64::new(0);
static PROCESS_SPAWN: Mutex<()> = Mutex::new(());

fn process_file(path: &Path) -> Result<File, String> {
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .read(true)
        .open(path)
        .map_err(|_| "Conversion process storage unavailable".into())
}
fn file_handle(file: &File) -> HANDLE {
    HANDLE(file.as_raw_handle())
}
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

struct OwnedHandle(HANDLE);
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
struct Job(OwnedHandle);
impl Job {
    fn new() -> Result<Self, String> {
        let job = Self(OwnedHandle(unsafe {
            CreateJobObjectW(None, PCWSTR::null())
                .map_err(|_| "Conversion process isolation unavailable")?
        }));
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags =
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_JOB_MEMORY;
        limits.JobMemoryLimit = 1536 * 1024 * 1024;
        unsafe {
            SetInformationJobObject(
                job.0 .0,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as _,
                std::mem::size_of_val(&limits) as u32,
            )
            .map_err(|_| "Conversion process isolation unavailable")?;
        }
        Ok(job)
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            let _ = TerminateJobObject(self.0 .0, 1);
            // TerminateJobObject 异步收尾；等拥有的进程释放文件后才清理临时目录。
            let started = Instant::now();
            loop {
                let mut usage = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
                if QueryInformationJobObject(
                    Some(self.0 .0),
                    JobObjectBasicAccountingInformation,
                    &mut usage as *mut _ as _,
                    std::mem::size_of_val(&usage) as u32,
                    None,
                )
                .is_err()
                    || usage.ActiveProcesses == 0
                    || started.elapsed() > Duration::from_secs(3)
                {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}
struct Inheritable([HANDLE; 3]);
impl Inheritable {
    fn new(handles: [HANDLE; 3]) -> Result<Self, String> {
        let guard = Self(handles);
        for handle in handles {
            unsafe {
                SetHandleInformation(handle, HANDLE_FLAG_INHERIT.0, HANDLE_FLAG_INHERIT)
                    .map_err(|_| "Conversion process handles unavailable")?;
            }
        }
        Ok(guard)
    }
}
impl Drop for Inheritable {
    fn drop(&mut self) {
        for handle in self.0 {
            unsafe {
                let _ = SetHandleInformation(handle, HANDLE_FLAG_INHERIT.0, HANDLE_FLAGS(0));
            }
        }
    }
}
struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
struct DirectoryRelease(PathBuf);
impl Drop for DirectoryRelease {
    fn drop(&mut self) {
        // Windows 进程退出信号可能早于当前目录句柄的内核收尾；确认它不再阻止后续原子移动/清理。
        let started = Instant::now();
        loop {
            let result = OpenOptions::new()
                .access_mode(0x00010000)
                .custom_flags(0x02000000)
                .open(&self.0);
            match result {
                Ok(_) => break,
                Err(error)
                    if error.raw_os_error() == Some(32)
                        && started.elapsed() < Duration::from_secs(3) =>
                {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Err(_) => break,
            }
        }
    }
}

struct Attributes {
    _storage: Vec<usize>,
    list: LPPROC_THREAD_ATTRIBUTE_LIST,
}
impl Attributes {
    fn new(handles: &[HANDLE; 3]) -> Result<Self, String> {
        let mut bytes = 0;
        unsafe {
            let _ = InitializeProcThreadAttributeList(None, 1, None, &mut bytes);
        }
        if bytes == 0 || bytes > 65536 {
            return Err("Conversion process handles unavailable".into());
        }
        let mut storage = vec![0usize; bytes.div_ceil(std::mem::size_of::<usize>())];
        let list = LPPROC_THREAD_ATTRIBUTE_LIST(storage.as_mut_ptr().cast());
        unsafe {
            InitializeProcThreadAttributeList(Some(list), 1, None, &mut bytes)
                .map_err(|_| "Conversion process handles unavailable")?;
        }
        let result = Self {
            _storage: storage,
            list,
        };
        // 明确仅继承本次请求的三条流，排除应用或其它并发任务的可继承句柄。
        unsafe {
            UpdateProcThreadAttribute(
                list,
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                Some(handles.as_ptr().cast()),
                std::mem::size_of_val(handles),
                None,
                None,
            )
            .map_err(|_| "Conversion process handles unavailable")?;
        }
        Ok(result)
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        unsafe {
            DeleteProcThreadAttributeList(self.list);
        }
    }
}

fn workspace_bytes(directory: &Path) -> Result<u64, String> {
    let mut pending = vec![(directory.to_path_buf(), 0u8)];
    let mut count = 0u32;
    let mut bytes = 0u64;
    while let Some((parent, depth)) = pending.pop() {
        if depth > 20 {
            return Err("Conversion workspace limit".into());
        }
        for entry in fs::read_dir(parent).map_err(|_| "Conversion workspace unavailable")? {
            let path = entry
                .map_err(|_| "Conversion workspace unavailable")?
                .path();
            let metadata = match fs::symlink_metadata(&path) {
                Ok(value) => value,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(_) => return Err("Conversion workspace unavailable".into()),
            };
            count += 1;
            if count > 20000 || metadata.file_attributes() & 0x400 != 0 {
                return Err("Conversion workspace limit".into());
            }
            if metadata.is_dir() {
                pending.push((path, depth + 1));
            } else {
                bytes = bytes
                    .checked_add(metadata.len())
                    .filter(|size| *size <= 2 * 1024 * 1024 * 1024)
                    .ok_or("Conversion output size limit")?;
            }
        }
    }
    Ok(bytes)
}

fn quote_argument(value: &str) -> String {
    let mut output = String::from("\"");
    let mut slashes = 0;
    for character in value.chars() {
        if character == '\\' {
            slashes += 1;
            continue;
        }
        output.extend(std::iter::repeat_n(
            '\\',
            if character == '"' {
                slashes * 2 + 1
            } else {
                slashes
            },
        ));
        slashes = 0;
        output.push(character);
    }
    output.extend(std::iter::repeat_n('\\', slashes * 2));
    output.push('"');
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::fs::OpenOptionsExt;
    fn fixture_directory(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "supersearch-process-{name}-{}-{}",
            std::process::id(),
            PROCESS_SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        directory
    }
    fn powershell() -> PathBuf {
        PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe")
    }
    fn script_args(script: &str) -> Vec<OsString> {
        [
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            script,
        ]
        .into_iter()
        .map(OsString::from)
        .collect()
    }

    #[test]
    fn windows_arguments_preserve_spaces_quotes_and_trailing_slashes() {
        assert_eq!(quote_argument(""), "\"\"");
        assert_eq!(quote_argument("C:\\a b\\"), "\"C:\\a b\\\\\"");
        assert_eq!(quote_argument("a\"b"), "\"a\\\"b\"");
    }

    #[test]
    fn cancelled_process_is_rejected_before_any_executable_is_opened() {
        let result = run(
            Path::new("C:\\missing.exe"),
            &[],
            Path::new("C:\\"),
            None,
            &AtomicBool::new(true),
            Duration::from_secs(1),
        );
        assert_eq!(result.err().as_deref(), Some("Conversion cancelled"));
    }
    #[test]
    fn actual_child_stdout_and_timeout_are_bounded() {
        let directory = fixture_directory("output");
        let output = run(
            &powershell(),
            &script_args("[Console]::Out.Write('runner-ok')"),
            &directory,
            None,
            &AtomicBool::new(false),
            Duration::from_secs(20),
        )
        .unwrap();
        assert_eq!(String::from_utf8(output.stdout).unwrap(), "runner-ok");
        let started = Instant::now();
        let result = run(
            &powershell(),
            &script_args("Start-Sleep -Seconds 30"),
            &directory,
            None,
            &AtomicBool::new(false),
            Duration::from_millis(150),
        );
        assert_eq!(result.err().as_deref(), Some("Conversion timeout"));
        assert!(started.elapsed() < Duration::from_secs(5));
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 0);
        fs::remove_dir(directory).unwrap();
    }
    #[test]
    fn strict_handle_list_excludes_an_unrelated_inheritable_file() {
        if std::env::var_os("SUPERSEARCH_ISOLATED_HANDLE_PROBE").is_none() {
            use std::os::windows::process::CommandExt;
            // 独占子测试进程，避免故意创建的可继承句柄被其它并行测试的普通启动器继承。
            let output=std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact","conversion_process::tests::strict_handle_list_excludes_an_unrelated_inheritable_file","--nocapture"]).env("SUPERSEARCH_ISOLATED_HANDLE_PROBE","1").creation_flags(0x08000000).output().unwrap();
            assert!(
                output.status.success(),
                "isolated handle probe failed: {} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        let directory = fixture_directory("handles");
        let sentinel = directory.join("exclusive-file");
        let file = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .share_mode(0)
            .open(&sentinel)
            .unwrap();
        unsafe {
            SetHandleInformation(
                file_handle(&file),
                HANDLE_FLAG_INHERIT.0,
                HANDLE_FLAG_INHERIT,
            )
            .unwrap();
        }
        let cancel = std::sync::Arc::new(AtomicBool::new(false));
        let signal = cancel.clone();
        let working = directory.clone();
        let child = std::thread::spawn(move || {
            run(&powershell(),&script_args("[Console]::Out.Write('started'); [Console]::Out.Flush(); Start-Sleep -Seconds 30"),&working,None,&signal,Duration::from_secs(20))
        });
        let started = Instant::now();
        loop {
            let ready = fs::read_dir(&directory)
                .unwrap()
                .filter_map(Result::ok)
                .any(|entry| {
                    entry.path().is_dir()
                        && fs::read(entry.path().join("stdout"))
                            .is_ok_and(|value| value == b"started")
                });
            if ready {
                break;
            }
            assert!(started.elapsed() < Duration::from_secs(15));
            std::thread::sleep(Duration::from_millis(20));
        }
        drop(file);
        let removed = fs::remove_file(&sentinel);
        cancel.store(true, Ordering::Release);
        assert_eq!(
            child.join().unwrap().err().as_deref(),
            Some("Conversion cancelled")
        );
        assert!(
            removed.is_ok(),
            "unrelated inheritable file remained locked by child"
        );
        fs::remove_dir(directory).unwrap();
    }
    #[test]
    fn cancellation_terminates_owned_descendants() {
        use windows::Win32::System::Threading::{
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
        };
        let directory = fixture_directory("descendants");
        let pid_file = directory.join("child-pid");
        let script=format!("$child=Start-Process -FilePath ($PSHOME+'\\powershell.exe') -WindowStyle Hidden -PassThru -ArgumentList '-NoProfile -NonInteractive -Command Start-Sleep -Seconds 30'; [IO.File]::WriteAllText('{}',[string]$child.Id); Start-Sleep -Seconds 30",pid_file.to_string_lossy().replace('\'',"''"));
        let signal = std::sync::Arc::new(AtomicBool::new(false));
        let cancel = signal.clone();
        let working = directory.clone();
        let child = std::thread::spawn(move || {
            run(
                &powershell(),
                &script_args(&script),
                &working,
                None,
                &cancel,
                Duration::from_secs(25),
            )
        });
        let started = Instant::now();
        while !pid_file.exists() {
            assert!(started.elapsed() < Duration::from_secs(20));
            std::thread::sleep(Duration::from_millis(20));
        }
        let pid = fs::read_to_string(&pid_file)
            .unwrap()
            .parse::<u32>()
            .unwrap();
        let handle = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                false,
                pid,
            )
        }
        .unwrap();
        let owned = OwnedHandle(handle);
        signal.store(true, Ordering::Release);
        assert_eq!(
            child.join().unwrap().err().as_deref(),
            Some("Conversion cancelled")
        );
        assert_eq!(unsafe { WaitForSingleObject(owned.0, 2000) }, WAIT_OBJECT_0);
        drop(owned);
        fs::remove_file(pid_file).unwrap();
        fs::remove_dir(directory).unwrap();
    }
}
