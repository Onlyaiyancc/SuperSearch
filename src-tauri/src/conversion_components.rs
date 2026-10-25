use crate::{conversion::ConversionOptions, conversion_process};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::windows::{fs::MetadataExt, process::CommandExt},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager};
use windows::{
    core::{w, PCWSTR},
    Win32::{
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER,
            COINIT_APARTMENTTHREADED,
        },
        UI::Shell::{
            Common::COMDLG_FILTERSPEC, FileOpenDialog, IFileOpenDialog, FOS_FILEMUSTEXIST,
            FOS_FORCEFILESYSTEM, FOS_NOCHANGEDIR, FOS_PATHMUSTEXIST, SIGDN_FILESYSPATH,
        },
    },
};

const ARCHIVE_LIMIT: u64 = 512 * 1024 * 1024;
const PACKAGE_LIMIT: u64 = 2 * 1024 * 1024 * 1024;
static CONVERTING: AtomicUsize = AtomicUsize::new(0);
static COMPONENT_GATE: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Download {
    url: String,
    bytes: u64,
    sha256: String,
    hosts: Vec<String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    id: String,
    name: String,
    description: String,
    version: String,
    license: String,
    source: String,
    installed_bytes: u64,
    #[serde(default)]
    unpacked_limit: Option<u64>,
    entry: String,
    kind: String,
    #[serde(default)]
    download: Option<Download>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionComponent {
    pub id: String,
    pub name: String,
    pub description: String,
    pub status: String,
    pub version: String,
    pub download_bytes: u64,
    pub installed_bytes: u64,
    pub source: String,
    pub license: String,
    pub error: String,
    pub managed: bool,
    pub can_install: bool,
    pub installation_source: String,
}
#[derive(Clone)]
struct Install {
    status: String,
    error: String,
    cancel: Arc<AtomicBool>,
}
pub struct ComponentState {
    root: PathBuf,
    installs: Mutex<HashMap<String, Install>>,
}
impl ComponentState {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        let known = manifests()?;
        fs::create_dir_all(root.join("components"))
            .map_err(|_| "Conversion component storage unavailable")?;
        cleanup_install_temporary(&root, &known)?;
        Ok(Self {
            root,
            installs: Mutex::new(HashMap::new()),
        })
    }
    pub fn cancel_all(&self) {
        if let Ok(installs) = self.installs.lock() {
            for item in installs.values() {
                item.cancel.store(true, Ordering::Release);
            }
        }
    }
    pub fn has_active(&self) -> bool {
        self.installs.lock().is_ok_and(|items| {
            items
                .values()
                .any(|item| matches!(item.status.as_str(), "downloading" | "installing"))
        })
    }
    fn phase(&self, app: &AppHandle, id: &str, status: &str, error: &str) {
        if let Ok(mut installs) = self.installs.lock() {
            if let Some(item) = installs.get_mut(id) {
                item.status = status.into();
                item.error = error.into();
            }
        }
        let _ = app.emit("conversion_components_changed", ());
    }
    fn begin(&self, id: &str) -> Result<Arc<AtomicBool>, String> {
        let mut installs = self
            .installs
            .lock()
            .map_err(|_| "Conversion components unavailable")?;
        if installs
            .values()
            .any(|item| matches!(item.status.as_str(), "downloading" | "installing"))
        {
            return Err("Conversion component installation busy".into());
        }
        let cancel = Arc::new(AtomicBool::new(false));
        installs.insert(
            id.into(),
            Install {
                status: "installing".into(),
                error: String::new(),
                cancel: cancel.clone(),
            },
        );
        Ok(cancel)
    }
}
fn manifests() -> Result<Vec<Manifest>, String> {
    let values: Vec<Manifest> = serde_json::from_str(include_str!("../conversion-components.json"))
        .map_err(|_| "Conversion invalid component manifest")?;
    let mut ids = HashSet::new();
    for item in &values {
        if !matches!(item.id.as_str(), "office" | "pandoc" | "pdf" | "ocr")
            || !ids.insert(item.id.clone())
            || item.version.is_empty()
            || !item
                .version
                .bytes()
                .all(|c| c.is_ascii_digit() || c == b'.')
            || !matches!(item.kind.as_str(), "zip" | "msi")
        {
            return Err("Conversion invalid component manifest".into());
        }
        package_path(&item.entry)?;
        if let Some(download) = &item.download {
            if !download.url.is_empty() {
                let url = reqwest::Url::parse(&download.url)
                    .map_err(|_| "Conversion invalid component source")?;
                if url.scheme() != "https"
                    || !download
                        .hosts
                        .iter()
                        .any(|host| Some(host.as_str()) == url.host_str())
                {
                    return Err("Conversion invalid component source".into());
                }
            }
            if download.bytes == 0
                || download.bytes > ARCHIVE_LIMIT
                || download.sha256.len() != 64
                || !download.sha256.bytes().all(|c| c.is_ascii_hexdigit())
            {
                return Err("Conversion invalid component manifest".into());
            }
        }
    }
    Ok(values)
}
fn manifest(id: &str) -> Result<Manifest, String> {
    manifests()?
        .into_iter()
        .find(|item| item.id == id)
        .ok_or("Conversion invalid component".into())
}
fn enabled(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<crate::service::AppState>();
    if state
        .store
        .lock()
        .map_err(|_| "Storage unavailable")?
        .settings()
        .conversion_enabled
    {
        Ok(())
    } else {
        Err("Conversion disabled".into())
    }
}
fn install_dir(root: &Path, item: &Manifest) -> PathBuf {
    root.join("components").join(&item.id).join(&item.version)
}
fn is_plain_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|meta| meta.is_file() && meta.file_attributes() & 0x400 == 0)
}

fn adjacent_package(executable: &Path, item: &Manifest) -> Result<Option<PathBuf>, String> {
    // 只读取与内置清单版本一致的固定随附包；不搜索工作目录或用户目录。
    let filename = match (item.id.as_str(), item.version.as_str()) {
        ("pandoc", "3.12.1") => "pandoc-3.12.1-windows-x86_64.zip",
        ("pdf", "1.0.0") => "supersearch-pdf-component-1.0.0-windows-x64.zip",
        ("ocr", "1.0.1") => "supersearch-ocr-component-1.0.1-windows-x64.zip",
        _ => return Ok(None),
    };
    let directory = executable
        .parent()
        .ok_or("Conversion invalid executable directory")?
        .join("components");
    for ancestor in directory.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(meta) if meta.is_dir() && meta.file_attributes() & 0x400 == 0 => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            _ => return Err("Conversion unsafe adjacent package directory".into()),
        }
    }
    let path = directory.join(filename);
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Ok(meta) if meta.is_file() && meta.file_attributes() & 0x400 == 0 => {}
        _ => return Err("Conversion invalid adjacent package".into()),
    }
    // 安全打开会再次检查每层目录与实际句柄位置；完整哈希仅在安装工作线程中计算。
    crate::file_preview::open_bounded_file(
        path.to_str().ok_or("Conversion invalid package path")?,
        ARCHIVE_LIMIT,
    )
    .map_err(|_| "Conversion invalid adjacent package")?;
    Ok(Some(path))
}

enum InstallSource {
    Adjacent(PathBuf),
    Download(Download),
}
impl InstallSource {
    fn label(&self) -> &'static str {
        match self {
            Self::Adjacent(_) => "adjacent",
            Self::Download(_) => "download",
        }
    }
}
fn install_source(executable: &Path, item: &Manifest) -> Result<InstallSource, String> {
    let download = item
        .download
        .as_ref()
        .ok_or("Conversion component package is not published in this build")?;
    if let Some(path) = adjacent_package(executable, item)? {
        return Ok(InstallSource::Adjacent(path));
    }
    if !download.url.is_empty() {
        return Ok(InstallSource::Download(download.clone()));
    }
    Err("Conversion component requires local package".into())
}

fn office_executable() -> Option<PathBuf> {
    let mut roots = vec![];
    for key in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(root) = std::env::var_os(key) {
            roots.push(PathBuf::from(root).join("LibreOffice"));
        }
    }
    unsafe {
        use windows::Win32::System::Registry::{
            RegGetValueW, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ,
        };
        for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
            let mut buffer = vec![0u16; 32768];
            let mut size = (buffer.len() * 2) as u32;
            if RegGetValueW(
                hive,
                w!("SOFTWARE\\LibreOffice\\UNO\\InstallPath"),
                PCWSTR::null(),
                RRF_RT_REG_SZ,
                None,
                Some(buffer.as_mut_ptr().cast()),
                Some(&mut size),
            )
            .is_ok()
            {
                let value = String::from_utf16_lossy(
                    &buffer[..buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len())],
                );
                let path = PathBuf::from(value);
                roots.push(
                    if path
                        .file_name()
                        .is_some_and(|n| n.eq_ignore_ascii_case("program"))
                    {
                        path.parent().unwrap_or(&path).into()
                    } else {
                        path
                    },
                );
            }
        }
    }
    roots
        .into_iter()
        .map(|root| root.join("program/soffice.com"))
        .find(|path| path.is_absolute() && is_plain_file(path))
}
fn private_entry(root: &Path, item: &Manifest) -> Option<PathBuf> {
    let download = item.download.as_ref()?;
    let directory = install_dir(root, item);
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(directory.join(".supersearch-verified.json")).ok()?)
            .ok()?;
    if value["sha256"].as_str() != Some(&download.sha256)
        || value["version"].as_str() != Some(&item.version)
    {
        return None;
    }
    let entry = directory.join(&item.entry);
    if !is_plain_file(&entry) {
        return None;
    }
    Some(entry)
}
fn executable(root: &Path, id: &str) -> Result<PathBuf, String> {
    if id == "office" {
        return office_executable().ok_or("Conversion component office missing".into());
    }
    private_entry(root, &manifest(id)?).ok_or_else(|| format!("Conversion component {id} missing"))
}

#[tauri::command]
pub async fn get_conversion_components(app: AppHandle) -> Result<Vec<ConversionComponent>, String> {
    enabled(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<ComponentState>();
        let installs = state
            .installs
            .lock()
            .map_err(|_| "Conversion components unavailable")?
            .clone();
        let executable =
            std::env::current_exe().map_err(|_| "Conversion executable unavailable")?;
        manifests()?
            .iter()
            .map(|item| {
                let ready = if item.id == "office" {
                    office_executable().is_some()
                } else {
                    private_entry(&state.root, item).is_some()
                };
                let flight = installs.get(&item.id);
                let active = flight.filter(|flight| {
                    matches!(flight.status.as_str(), "downloading" | "installing")
                });
                let status = active.map(|i| i.status.as_str()).unwrap_or(if ready {
                    "ready"
                } else if flight.is_some_and(|i| i.status == "failed") {
                    "failed"
                } else {
                    "missing"
                });
                let source = install_source(&executable, item);
                Ok(ConversionComponent {
                    id: item.id.clone(),
                    name: item.name.clone(),
                    description: item.description.clone(),
                    status: status.into(),
                    version: item.version.clone(),
                    download_bytes: item.download.as_ref().map_or(0, |d| d.bytes),
                    installed_bytes: item.installed_bytes,
                    source: item.source.clone(),
                    license: item.license.clone(),
                    error: flight.map_or_else(String::new, |i| i.error.clone()),
                    managed: item.id != "office",
                    can_install: source.is_ok(),
                    installation_source: source
                        .as_ref()
                        .map_or("unavailable", InstallSource::label)
                        .into(),
                })
            })
            .collect()
    })
    .await
    .map_err(|_| "Conversion components unavailable")?
}

#[tauri::command]
pub fn install_conversion_component(app: AppHandle, id: String) -> Result<(), String> {
    enabled(&app)?;
    let item = manifest(&id)?;
    let cancel = app.state::<ComponentState>().begin(&id)?;
    let _ = app.emit("conversion_components_changed", ());
    tauri::async_runtime::spawn(async move {
        let root = app.state::<ComponentState>().root.clone();
        let downloads = root.join("components/.downloads");
        let path = downloads.join(format!("{}.partial", item.id));
        let result = async {
            let source_item = item.clone();
            let source = tauri::async_runtime::spawn_blocking(move || {
                let executable =
                    std::env::current_exe().map_err(|_| "Conversion executable unavailable")?;
                install_source(&executable, &source_item)
            })
            .await
            .map_err(|_| "Conversion component installation failed")??;
            let input = match source {
                InstallSource::Adjacent(path) => path,
                InstallSource::Download(download) => {
                    app.state::<ComponentState>()
                        .phase(&app, &id, "downloading", "");
                    fs::create_dir_all(&downloads)
                        .map_err(|_| "Conversion component storage unavailable")?;
                    let _ = fs::remove_file(&path);
                    download_package(&download, &path, &cancel).await?;
                    path.clone()
                }
            };
            app.state::<ComponentState>()
                .phase(&app, &id, "installing", "");
            let signal = cancel.clone();
            tauri::async_runtime::spawn_blocking(move || {
                activate_package(&root, &item, &input, &signal)
            })
            .await
            .map_err(|_| "Conversion component installation failed")?
        }
        .await;
        let _ = fs::remove_file(&path);
        let state = app.state::<ComponentState>();
        match result {
            Ok(()) => state.phase(
                &app,
                &id,
                "missing",
                if id == "office" {
                    "Conversion installer launched; finish installation and refresh"
                } else {
                    ""
                },
            ),
            Err(error) => state.phase(
                &app,
                &id,
                if cancel.load(Ordering::Acquire) {
                    "missing"
                } else {
                    "failed"
                },
                &error,
            ),
        }
    });
    Ok(())
}
#[tauri::command]
pub fn cancel_component_install(app: AppHandle, id: String) -> Result<(), String> {
    manifest(&id)?;
    if let Some(item) = app
        .state::<ComponentState>()
        .installs
        .lock()
        .map_err(|_| "Conversion components unavailable")?
        .get(&id)
    {
        item.cancel.store(true, Ordering::Release);
    }
    Ok(())
}
#[tauri::command]
pub async fn import_conversion_component(app: AppHandle, id: String) -> Result<(), String> {
    enabled(&app)?;
    let item = manifest(&id)?;
    if item.download.is_none() {
        return Err("Conversion component package is not published in this build".into());
    }
    let selected = tauri::async_runtime::spawn_blocking(choose_package)
        .await
        .map_err(|_| "Conversion component picker unavailable")??;
    let Some(path) = selected else { return Ok(()) };
    let cancel = app.state::<ComponentState>().begin(&id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<ComponentState>();
        state.phase(&app, &id, "installing", "");
        let result = activate_package(&state.root, &item, &path, &cancel);
        match result {
            Ok(()) => state.phase(
                &app,
                &id,
                "missing",
                if id == "office" {
                    "Conversion installer launched; finish installation and refresh"
                } else {
                    ""
                },
            ),
            Err(error) => state.phase(
                &app,
                &id,
                if cancel.load(Ordering::Acquire) {
                    "missing"
                } else {
                    "failed"
                },
                &error,
            ),
        }
    });
    Ok(())
}
#[tauri::command]
pub async fn remove_conversion_component(app: AppHandle, id: String) -> Result<(), String> {
    enabled(&app)?;
    let item = manifest(&id)?;
    if id == "office" {
        return Err("Conversion component is managed by Windows".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let _gate = COMPONENT_GATE
            .lock()
            .map_err(|_| "Conversion components unavailable")?;
        let state = app.state::<ComponentState>();
        if CONVERTING.load(Ordering::Acquire) > 0 || state.has_active() {
            return Err("Conversion component is in use".into());
        }
        remove_private_dir(
            &state.root.join("components"),
            &install_dir(&state.root, &item),
        )?;
        if let Ok(mut items) = state.installs.lock() {
            items.remove(&id);
        }
        let _ = app.emit("conversion_components_changed", ());
        Ok(())
    })
    .await
    .map_err(|_| "Conversion component removal failed")?
}

async fn wait_cancel(cancel: &AtomicBool) {
    while !cancel.load(Ordering::Acquire) {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

pub async fn diagnose_pandoc_download(path: &Path) -> Result<(), String> {
    let item = manifest("pandoc")?;
    let download = item
        .download
        .ok_or("Conversion component package is not published in this build")?;
    download_package(&download, path, &AtomicBool::new(false)).await
}

fn download_failure(stage: &str, error: reqwest::Error) -> String {
    // 不回传含签名令牌的重定向 URL；只保留请求阶段与安全的状态分类。
    let reason = if let Some(status) = error.status() {
        format!("HTTP {}", status.as_u16())
    } else if error.is_timeout() {
        "network timeout".into()
    } else if error.is_redirect() {
        "redirect rejected".into()
    } else if error.is_connect() {
        "network connection failed".into()
    } else {
        "request failed".into()
    };
    format!("Conversion component download {stage}: {reason}")
}
async fn download_package(item: &Download, path: &Path, cancel: &AtomicBool) -> Result<(), String> {
    let allowed = item.hosts.clone();
    let client = reqwest::Client::builder()
        .https_only(true)
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(900))
        .redirect(reqwest::redirect::Policy::custom(move |attempt| {
            if attempt.previous().len() > 5
                || attempt.url().scheme() != "https"
                || !allowed
                    .iter()
                    .any(|host| Some(host.as_str()) == attempt.url().host_str())
            {
                attempt.error("Untrusted component redirect")
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|error| download_failure("client", error))?;
    let mut response = tokio::select! {biased;_=wait_cancel(cancel)=>return Err("Conversion cancelled".into()),result=client.get(&item.url).send()=>result.and_then(|response|response.error_for_status()).map_err(|error|download_failure("request",error))?};
    if response
        .content_length()
        .is_some_and(|size| size != item.bytes)
    {
        return Err("Conversion component size mismatch".into());
    }
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| "Conversion component storage unavailable")?;
    let mut count = 0u64;
    let mut hash = Sha256::new();
    loop {
        let chunk = tokio::select! {biased;_=wait_cancel(cancel)=>return Err("Conversion cancelled".into()),result=response.chunk()=>result.map_err(|error|download_failure("body",error))?};
        let Some(chunk) = chunk else { break };
        count += chunk.len() as u64;
        if count > item.bytes {
            return Err("Conversion component size mismatch".into());
        }
        output
            .write_all(&chunk)
            .map_err(|_| "Conversion component disk full")?;
        hash.update(&chunk);
    }
    output
        .sync_all()
        .map_err(|_| "Conversion component storage unavailable")?;
    if count != item.bytes || format!("{:x}", hash.finalize()) != item.sha256 {
        return Err("Conversion component checksum mismatch".into());
    }
    Ok(())
}
fn verified_file(path: &Path, item: &Download, cancel: &AtomicBool) -> Result<File, String> {
    let mut file = crate::file_preview::open_bounded_file(
        path.to_str().ok_or("Conversion invalid package path")?,
        ARCHIVE_LIMIT,
    )
    .map_err(|_| "Conversion invalid local package")?;
    if file
        .metadata()
        .map_err(|_| "Conversion invalid package")?
        .len()
        != item.bytes
    {
        return Err("Conversion component size mismatch".into());
    }
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        if cancel.load(Ordering::Acquire) {
            return Err("Conversion cancelled".into());
        }
        let read = file
            .read(&mut buffer)
            .map_err(|_| "Conversion package read failed")?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    if format!("{:x}", hash.finalize()) != item.sha256 {
        return Err("Conversion component checksum mismatch".into());
    }
    use std::io::Seek;
    file.rewind()
        .map_err(|_| "Conversion package read failed")?;
    Ok(file)
}

fn activate_package(
    root: &Path,
    item: &Manifest,
    path: &Path,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let _gate = COMPONENT_GATE
        .lock()
        .map_err(|_| "Conversion components unavailable")?;
    activate_package_locked(root, item, path, cancel)
}

fn activate_package_locked(
    root: &Path,
    item: &Manifest,
    path: &Path,
    cancel: &AtomicBool,
) -> Result<(), String> {
    if CONVERTING.load(Ordering::Acquire) > 0 {
        return Err("Conversion component is in use".into());
    }
    let download = item
        .download
        .as_ref()
        .ok_or("Conversion component package is not published in this build")?;
    let file = verified_file(path, download, cancel)?;
    if item.kind == "msi" {
        drop(file);
        let installer = root
            .join("components")
            .join(format!("LibreOffice-{}.msi", item.version));
        if !installer.exists() {
            let mut input = verified_file(path, download, cancel)?;
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&installer)
                .map_err(|_| "Conversion component installer unavailable")?;
            std::io::copy(&mut input, &mut output).map_err(|_| "Conversion component disk full")?;
            output
                .sync_all()
                .map_err(|_| "Conversion component storage unavailable")?;
        } else {
            verified_file(&installer, download, cancel)?;
        }
        let system = std::env::var_os("SystemRoot").ok_or("Conversion installer unavailable")?;
        // 安装器保留正常的用户界面，不静默安装、不自行提升权限。
        std::process::Command::new(PathBuf::from(system).join("System32/msiexec.exe"))
            .arg("/i")
            .arg(&installer)
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|_| "Conversion installer could not start")?;
        return Ok(());
    }
    let parent = root.join("components").join(&item.id);
    fs::create_dir_all(&parent).map_err(|_| "Conversion component storage unavailable")?;
    let staging = parent.join(format!(".{}-staging", item.version));
    remove_private_dir(&root.join("components"), &staging)?;
    fs::create_dir(&staging).map_err(|_| "Conversion component storage unavailable")?;
    let result = (|| {
        unpack(
            file,
            &staging,
            item.unpacked_limit
                .unwrap_or(item.installed_bytes)
                .min(PACKAGE_LIMIT),
            cancel,
        )?;
        let entry = staging.join(&item.entry);
        if !is_plain_file(&entry) {
            return Err("Conversion component entry missing".into());
        }
        if matches!(item.id.as_str(), "pdf" | "ocr") {
            let package: serde_json::Value = serde_json::from_slice(
                &fs::read(staging.join("component.json"))
                    .map_err(|_| "Conversion component manifest missing")?,
            )
            .map_err(|_| "Conversion component manifest invalid")?;
            if package["id"] != item.id
                || package["version"] != item.version
                || package["entry"] != item.entry
            {
                return Err("Conversion component manifest mismatch".into());
            }
        }
        let probe = conversion_process::run(
            &entry,
            &[OsString::from("--version")],
            &staging,
            None,
            cancel,
            Duration::from_secs(20),
        )?;
        if probe.stdout.is_empty() {
            return Err("Conversion component probe failed".into());
        }
        fs::write(
            staging.join(".supersearch-verified.json"),
            serde_json::to_vec(&json!({"version":item.version,"sha256":download.sha256}))
                .map_err(|_| "Conversion component manifest invalid")?,
        )
        .map_err(|_| "Conversion component storage unavailable")?;
        if cancel.load(Ordering::Acquire) {
            return Err("Conversion cancelled".into());
        }
        let destination = install_dir(root, item);
        let backup = parent.join(format!(".{}-backup", item.version));
        remove_private_dir(&root.join("components"), &backup)?;
        let previous = destination.exists();
        if previous {
            rename_private(&destination, &backup)
                .map_err(|_| "Conversion component replacement failed")?;
        }
        if rename_private(&staging, &destination).is_err() {
            if previous {
                let _ = rename_private(&backup, &destination);
            }
            return Err("Conversion component activation failed".into());
        }
        let _ = remove_private_dir(&root.join("components"), &backup);
        Ok(())
    })();
    if result.is_err() {
        let _ = remove_private_dir(&root.join("components"), &staging);
    }
    result
}

fn cleanup_install_temporary(root: &Path, known: &[Manifest]) -> Result<(), String> {
    let components = root.join("components");
    for item in known {
        let partial = components
            .join(".downloads")
            .join(format!("{}.partial", item.id));
        if partial.exists() {
            if !is_plain_file(&partial) {
                return Err("Conversion unsafe component directory".into());
            }
            fs::remove_file(partial).map_err(|_| "Conversion component cleanup failed")?;
        }
        let parent = components.join(&item.id);
        let backup = parent.join(format!(".{}-backup", item.version));
        let destination = install_dir(root, item);
        if backup.exists() {
            if destination.exists() {
                remove_private_dir(&components, &backup)?;
            } else {
                let resolved = backup
                    .canonicalize()
                    .map_err(|_| "Conversion component recovery failed")?;
                let confinement = components
                    .canonicalize()
                    .map_err(|_| "Conversion component storage unavailable")?;
                if !resolved.starts_with(confinement)
                    || fs::symlink_metadata(&backup)
                        .map_err(|_| "Conversion component recovery failed")?
                        .file_attributes()
                        & 0x400
                        != 0
                {
                    return Err("Conversion unsafe component directory".into());
                }
                fs::rename(&backup, &destination)
                    .map_err(|_| "Conversion component recovery failed")?;
            }
        }
        if parent.is_dir() {
            let mut count = 0;
            for entry in fs::read_dir(&parent).map_err(|_| "Conversion component cleanup failed")? {
                count += 1;
                if count > 128 {
                    return Err("Conversion component directory limit".into());
                }
                let entry = entry.map_err(|_| "Conversion component cleanup failed")?;
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if let Some(version) = name
                    .strip_prefix('.')
                    .and_then(|value| value.strip_suffix("-staging"))
                {
                    if !version.is_empty()
                        && version.bytes().all(|c| c.is_ascii_digit() || c == b'.')
                    {
                        remove_private_dir(&components, &entry.path())?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn rename_private(source: &Path, destination: &Path) -> std::io::Result<()> {
    let started = std::time::Instant::now();
    loop {
        match fs::rename(source, destination) {
            Ok(()) => return Ok(()),
            Err(error)
                if matches!(error.raw_os_error(), Some(5 | 32 | 33))
                    && started.elapsed() < Duration::from_secs(3) =>
            {
                std::thread::sleep(Duration::from_millis(20))
            }
            Err(error) => return Err(error),
        }
    }
}

fn package_path(value: &str) -> Result<PathBuf, String> {
    if value.is_empty()
        || value.len() > 1024
        || value.starts_with('/')
        || value.contains(['\\', ':', '\0'])
    {
        return Err("Conversion invalid package path".into());
    }
    let trimmed = value.trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("Conversion invalid package path".into());
    }
    for part in trimmed.split('/') {
        let stem = part
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.ends_with(['.', ' '])
            || part.chars().any(char::is_control)
            || matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || ((stem.starts_with("COM") || stem.starts_with("LPT"))
                && stem.len() == 4
                && stem.as_bytes()[3].is_ascii_digit())
        {
            return Err("Conversion invalid package path".into());
        }
    }
    Ok(PathBuf::from(trimmed))
}
fn unpack(file: File, directory: &Path, limit: u64, cancel: &AtomicBool) -> Result<(), String> {
    let mut archive = zip::ZipArchive::new(file).map_err(|_| "Conversion invalid package ZIP")?;
    if archive.len() > 20000 {
        return Err("Conversion package entry limit".into());
    }
    let mut seen = HashSet::new();
    let mut total = 0u64;
    for index in 0..archive.len() {
        if cancel.load(Ordering::Acquire) {
            return Err("Conversion cancelled".into());
        }
        let mut item = archive
            .by_index(index)
            .map_err(|_| "Conversion invalid package ZIP")?;
        let relative = package_path(&item.name().map_err(|_| "Conversion invalid package name")?)?;
        if !seen.insert(relative.to_string_lossy().to_ascii_lowercase())
            || item.unix_mode().is_some_and(|mode| {
                mode & 0o170000 != 0 && mode & 0o170000 != 0o100000 && mode & 0o170000 != 0o040000
            })
        {
            return Err("Conversion unsafe package entry".into());
        }
        total = total
            .checked_add(item.size())
            .filter(|sum| *sum <= limit)
            .ok_or("Conversion package size limit")?;
        let destination = directory.join(relative);
        if item.is_dir() {
            fs::create_dir_all(&destination).map_err(|_| "Conversion component disk full")?;
            continue;
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|_| "Conversion component disk full")?;
        }
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
            .map_err(|_| "Conversion unsafe package entry")?;
        let expected = item.size();
        let count = std::io::copy(&mut item.by_ref().take(expected + 1), &mut output)
            .map_err(|_| "Conversion package extraction failed")?;
        if count != expected {
            return Err("Conversion package size mismatch".into());
        }
    }
    Ok(())
}
fn remove_private_dir(root: &Path, directory: &Path) -> Result<(), String> {
    if !directory.exists() {
        return Ok(());
    }
    let root = root
        .canonicalize()
        .map_err(|_| "Conversion component storage unavailable")?;
    let resolved = directory
        .canonicalize()
        .map_err(|_| "Conversion unsafe component directory")?;
    if resolved == root
        || !resolved.starts_with(&root)
        || fs::symlink_metadata(directory)
            .map_err(|_| "Conversion unsafe component directory")?
            .file_attributes()
            & 0x400
            != 0
    {
        return Err("Conversion unsafe component directory".into());
    }
    fs::remove_dir_all(directory).map_err(|_| "Conversion component removal failed".into())
}
struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
fn choose_package() -> Result<Option<PathBuf>, String> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .map_err(|_| "Conversion picker unavailable")?;
        let _com = Apartment;
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
            .map_err(|_| "Conversion picker unavailable")?;
        dialog
            .SetTitle(w!("安装本地转换组件包"))
            .map_err(|_| "Conversion picker unavailable")?;
        dialog
            .SetOptions(
                FOS_FILEMUSTEXIST | FOS_PATHMUSTEXIST | FOS_FORCEFILESYSTEM | FOS_NOCHANGEDIR,
            )
            .map_err(|_| "Conversion picker unavailable")?;
        dialog
            .SetFileTypes(&[COMDLG_FILTERSPEC {
                pszName: w!("转换组件包 (*.zip;*.msi)"),
                pszSpec: w!("*.zip;*.msi"),
            }])
            .map_err(|_| "Conversion picker unavailable")?;
        if let Err(error) = dialog.Show(None) {
            if error.code().0 as u32 == 0x800704c7 {
                return Ok(None);
            }
            return Err("Conversion picker unavailable".into());
        }
        let selected = dialog
            .GetResult()
            .map_err(|_| "Conversion picker unavailable")?
            .GetDisplayName(SIGDN_FILESYSPATH)
            .map_err(|_| "Conversion picker unavailable")?;
        let path = selected
            .to_string()
            .map_err(|_| "Conversion picker unavailable");
        CoTaskMemFree(Some(selected.0.cast()));
        Ok(Some(PathBuf::from(path?)))
    }
}

struct Lease;

pub fn recognize_screen_links(
    app: &AppHandle,
    image: &image::RgbaImage,
    cancel: &AtomicBool,
) -> Result<crate::qr_links::ExtractedLinks, String> {
    let executable = std::env::current_exe().map_err(|_| "Conversion executable unavailable")?;
    recognize_screen_links_at(
        &app.state::<ComponentState>().root,
        &executable,
        image,
        cancel,
    )
}

pub fn recognize_screen_links_at(
    root: &Path,
    executable: &Path,
    image: &image::RgbaImage,
    cancel: &AtomicBool,
) -> Result<crate::qr_links::ExtractedLinks, String> {
    if cancel.load(Ordering::Acquire) {
        return Err("Conversion cancelled".into());
    }
    let item = manifest("ocr")?;
    if private_entry(root, &item).is_none() {
        // 屏幕识别只启用清单锁定的随附 OCR 包，不依赖格式转换开关，也不发起下载。
        let package = adjacent_package(executable, &item)?
            .ok_or("Conversion component requires local package")?;
        // 扫描不等待其它组件安装；立即返回可重试状态，关闭面板无需等待长时间解包。
        let _gate = COMPONENT_GATE
            .try_lock()
            .map_err(|_| "Conversion components busy")?;
        if cancel.load(Ordering::Acquire) {
            return Err("Conversion cancelled".into());
        }
        activate_package_locked(root, &item, &package, cancel)?;
    }
    // 安装结束后再申请租约，并在租约内重新解析私有入口，避免与卸载/替换组件竞态。
    let _lease = Lease::try_acquire()?;
    if cancel.load(Ordering::Acquire) {
        return Err("Conversion cancelled".into());
    }
    let entry = private_entry(root, &item).ok_or("Conversion screen OCR component unavailable")?;
    let runtime = entry
        .parent()
        .ok_or("Conversion screen OCR component unavailable")?;
    let tasks = root.join("screen-ocr-tasks");
    fs::create_dir_all(&tasks).map_err(|_| "Conversion screen OCR workspace unavailable")?;
    crate::screen_ocr::recognize(image, runtime, &tasks, cancel)
}

#[cfg(test)]
mod screen_links_tests {
    use super::*;

    #[test]
    fn screen_ocr_lease_fails_immediately_while_the_component_gate_is_busy() {
        let _gate = COMPONENT_GATE.lock().unwrap();
        assert_eq!(
            Lease::try_acquire().err().as_deref(),
            Some("Conversion components busy")
        );
    }

    #[test]
    fn cancelled_screen_ocr_does_not_install_or_create_workspace() {
        let root = std::env::temp_dir().join(format!(
            "supersearch-screen-ocr-cancel-{}",
            std::process::id()
        ));
        let result = recognize_screen_links_at(
            &root,
            Path::new("C:\\missing.exe"),
            &image::RgbaImage::new(1, 1),
            &AtomicBool::new(true),
        );
        assert_eq!(result.err().as_deref(), Some("Conversion cancelled"));
        assert!(!root.exists());
    }
}

impl Lease {
    fn try_acquire() -> Result<Self, String> {
        let _gate = COMPONENT_GATE
            .try_lock()
            .map_err(|_| "Conversion components busy")?;
        CONVERTING.fetch_add(1, Ordering::AcqRel);
        Ok(Self)
    }

    fn acquire() -> Result<Self, String> {
        let _gate = COMPONENT_GATE
            .lock()
            .map_err(|_| "Conversion components unavailable")?;
        CONVERTING.fetch_add(1, Ordering::AcqRel);
        Ok(Self)
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        CONVERTING.fetch_sub(1, Ordering::AcqRel);
    }
}

pub fn convert(
    root: &Path,
    source: &Path,
    output_dir: &Path,
    options: &ConversionOptions,
    cancel: &AtomicBool,
) -> Result<Vec<PathBuf>, String> {
    convert_many(root, &[source.to_path_buf()], output_dir, options, cancel)
}
pub fn convert_many(
    root: &Path,
    sources: &[PathBuf],
    output_dir: &Path,
    options: &ConversionOptions,
    cancel: &AtomicBool,
) -> Result<Vec<PathBuf>, String> {
    let _lease = Lease::acquire()?;
    if cancel.load(Ordering::Acquire) {
        return Err("Conversion cancelled".into());
    }
    if sources.is_empty() || sources.len() > 100 {
        return Err("Conversion invalid input count".into());
    }
    fs::create_dir_all(output_dir).map_err(|_| "Conversion output unavailable")?;
    let source = &sources[0];
    let source_ext = extension(source);
    if sources.len() > 1
        || source_ext == "pdf"
        || matches!(
            source_ext.as_str(),
            "png" | "jpg" | "jpeg" | "webp" | "bmp" | "tiff" | "tif" | "gif" | "ico"
        )
    {
        return helper_convert(root, sources, output_dir, options, cancel);
    }
    if matches!(
        source_ext.as_str(),
        "md" | "markdown" | "html" | "htm" | "txt"
    ) || (source_ext == "docx" && options.target == "md")
    {
        return pandoc_convert(root, source, output_dir, options, cancel);
    }
    office_convert(root, source, output_dir, options, cancel, None)
}
fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn helper_convert(
    root: &Path,
    sources: &[PathBuf],
    output: &Path,
    options: &ConversionOptions,
    cancel: &AtomicBool,
) -> Result<Vec<PathBuf>, String> {
    let entry = executable(root, "pdf")?;
    let mut request = json!({"source":sources[0],"outputDir":output,"options":options});
    if options.merge {
        request["sources"] = json!(sources);
    }
    if options.ocr {
        let ocr = executable(root, "ocr")?;
        request["ocrExecutable"] = json!(ocr);
        request["ocrData"] = json!(ocr
            .parent()
            .ok_or("Conversion OCR unavailable")?
            .join("tessdata"));
    }
    let input = serde_json::to_vec(&request).map_err(|_| "Conversion invalid helper request")?;
    let result = conversion_process::run(
        &entry,
        &[],
        output,
        Some(&input),
        cancel,
        Duration::from_secs(600),
    )?;
    let response: serde_json::Value =
        serde_json::from_slice(&result.stdout).map_err(|_| "Conversion invalid helper response")?;
    if response
        .get("error")
        .is_some_and(|value| !value.is_null() && value != "")
    {
        return Err("Conversion document extraction failed".into());
    }
    let files = response["outputs"]
        .as_array()
        .filter(|files| !files.is_empty() && files.len() <= 1000)
        .ok_or("Conversion empty output")?;
    let directory = output
        .canonicalize()
        .map_err(|_| "Conversion output unavailable")?;
    files
        .iter()
        .map(|value| {
            let path = PathBuf::from(value.as_str().ok_or("Conversion invalid helper response")?);
            let resolved = path
                .canonicalize()
                .map_err(|_| "Conversion output unavailable")?;
            if !resolved.starts_with(&directory) || !is_plain_file(&resolved) {
                return Err("Conversion invalid helper output".into());
            }
            let relative = resolved
                .strip_prefix(&directory)
                .map_err(|_| "Conversion invalid helper output")?;
            let ordinary = output.join(relative);
            validate_output(&ordinary)?;
            Ok(ordinary)
        })
        .collect()
}

fn office_convert(
    root: &Path,
    source: &Path,
    output: &Path,
    options: &ConversionOptions,
    cancel: &AtomicBool,
    override_executable: Option<&Path>,
) -> Result<Vec<PathBuf>, String> {
    let selected = if let Some(path) = override_executable {
        path.to_path_buf()
    } else {
        executable(root, "office")?
    };
    reject_remote_resources(source)?;
    let profile = output.join(".office-profile");
    fs::create_dir_all(profile.join("user")).map_err(|_| "Conversion profile unavailable")?;
    fs::write(profile.join("user/registrymodifications.xcu"),br#"<?xml version="1.0" encoding="UTF-8"?><oor:items xmlns:oor="http://openoffice.org/2001/registry"><item oor:path="/org.openoffice.Office.Common/Security/Scripting"><prop oor:name="MacroSecurityLevel" oor:op="fuse"><value>3</value></prop><prop oor:name="DisableMacrosExecution" oor:op="fuse"><value>true</value></prop></item><item oor:path="/org.openoffice.Office.Common/Security/Links"><prop oor:name="LinkUpdateMode" oor:op="fuse"><value>0</value></prop></item><item oor:path="/org.openoffice.Office.Calc/Content/Update"><prop oor:name="Link" oor:op="fuse"><value>0</value></prop></item><item oor:path="/org.openoffice.Office.Writer/Content/Update"><prop oor:name="Link" oor:op="fuse"><value>0</value></prop></item></oor:items>"#).map_err(|_|"Conversion profile unavailable")?;
    let profile_url = reqwest::Url::from_directory_path(
        profile
            .canonicalize()
            .map_err(|_| "Conversion profile unavailable")?,
    )
    .map_err(|_| "Conversion profile unavailable")?;
    let filter = office_filter(&extension(source), &options.target, options.sheet)?;
    let mut args: Vec<OsString> = vec![
        format!("-env:UserInstallation={profile_url}").into(),
        "--headless".into(),
        "--nologo".into(),
        "--nodefault".into(),
        "--norestore".into(),
        "--convert-to".into(),
        filter.into(),
        "--outdir".into(),
        output.as_os_str().to_owned(),
    ];
    if extension(source) == "csv" {
        args.push("--infilter=Text - txt - csv (StarCalc):44,34,76,1,,0,true,false,false,false,false,0,false,true,false".into());
        let normalized = output.join(".normalized.csv");
        crate::conversion::normalize_csv(source, &normalized, options)?;
        args.push(normalized.into_os_string());
    } else {
        args.push(source.as_os_str().to_owned());
    }
    conversion_process::run(
        &selected,
        &args,
        output,
        None,
        cancel,
        Duration::from_secs(120),
    )?;
    let mut outputs = collect_outputs(output, &options.target)?;
    if options.target == "html" {
        for path in &outputs {
            sanitize_html_file(path, true)?;
        }
        for path in collect_resource_files(output, 0, &outputs)? {
            if !path
                .strip_prefix(output)
                .map_err(|_| "Conversion invalid output location")?
                .components()
                .any(|part| part.as_os_str().to_string_lossy().starts_with('.'))
                && !outputs.contains(&path)
            {
                outputs.push(path);
            }
        }
    }
    Ok(outputs)
}
fn office_filter(source: &str, target: &str, sheet: Option<u32>) -> Result<String, String> {
    let word = matches!(source, "doc" | "docx" | "odt" | "rtf");
    let calc = matches!(source, "xls" | "xlsx" | "ods" | "csv");
    let slides = matches!(source, "ppt" | "pptx" | "odp");
    let filter = match target {
        "pdf" if word => "pdf:writer_pdf_Export".into(),
        "pdf" if calc => "pdf:calc_pdf_Export".into(),
        "pdf" if slides => "pdf:impress_pdf_Export".into(),
        "doc" if word => "doc:MS Word 97".into(),
        "docx" if word => "docx:Office Open XML Text".into(),
        "odt" if word => "odt:writer8".into(),
        "rtf" if word => "rtf:Rich Text Format".into(),
        "txt" if word => "txt:Text (encoded):UTF8".into(),
        "html" if word => "html:HTML (StarWriter)".into(),
        "xls" if calc => "xls:MS Excel 97".into(),
        "xlsx" if calc => "xlsx:Calc Office Open XML".into(),
        "ods" if calc => "ods:calc8".into(),
        "csv" if calc => {
            let selected = sheet
                .filter(|value| *value > 0 && *value <= 16384)
                .ok_or("Conversion CSV requires selected sheet")?;
            format!("csv:Text - txt - csv (StarCalc):44,34,76,1,,0,false,true,true,false,false,{selected},false,true,false")
        }
        "ppt" if slides => "ppt:MS PowerPoint 97".into(),
        "pptx" if slides => "pptx:Impress MS PowerPoint 2007 XML".into(),
        "odp" if slides => "odp:impress8".into(),
        _ => return Err("Conversion unsupported direction".into()),
    };
    Ok(filter)
}

fn pandoc_convert(
    root: &Path,
    source: &Path,
    output: &Path,
    options: &ConversionOptions,
    cancel: &AtomicBool,
) -> Result<Vec<PathBuf>, String> {
    let selected = executable(root, "pandoc")?;
    if !matches!(extension(source).as_str(), "html" | "htm") {
        reject_remote_resources(source)?;
    }
    let target = if options.target == "pdf" {
        "docx"
    } else {
        options.target.as_str()
    };
    if !matches!(target, "md" | "txt" | "html" | "docx") {
        return Err("Conversion unsupported direction".into());
    }
    let mut input = source.to_path_buf();
    let source_type = extension(source);
    let from = match source_type.as_str() {
        "md" | "markdown" => "gfm-raw_html",
        "html" | "htm" => {
            let text =
                fs::read_to_string(source).map_err(|_| "Conversion invalid text encoding")?;
            input = output.join(".sanitized-input.html");
            fs::write(&input, clean_html(&text, false))
                .map_err(|_| "Conversion output unavailable")?;
            "html"
        }
        "docx" => "docx",
        "txt" => {
            let text =
                fs::read_to_string(source).map_err(|_| "Conversion invalid text encoding")?;
            let escaped = text
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            input = output.join(".plain-text.html");
            fs::write(
                &input,
                format!("<!doctype html><meta charset=\"utf-8\"><pre>{escaped}</pre>"),
            )
            .map_err(|_| "Conversion output unavailable")?;
            "html"
        }
        _ => return Err("Conversion unsupported direction".into()),
    };
    let destination = output.join(format!("converted.{target}"));
    let to = match target {
        "md" => "gfm",
        "txt" => "plain",
        other => other,
    };
    let mut args: Vec<OsString> = vec![
        "--sandbox".into(),
        "--standalone".into(),
        "--from".into(),
        from.into(),
        "--to".into(),
        to.into(),
        "--output".into(),
        destination.as_os_str().to_owned(),
        input.as_os_str().to_owned(),
    ];
    if source_type == "docx" && target == "md" {
        args.push("--extract-media=converted-assets".into());
    }
    conversion_process::run(
        &selected,
        &args,
        output,
        None,
        cancel,
        Duration::from_secs(120),
    )?;
    if options.target == "pdf" {
        let pdf_output = output.join(".pdf-stage");
        fs::create_dir(&pdf_output).map_err(|_| "Conversion output unavailable")?;
        let files = office_convert(root, &destination, &pdf_output, options, cancel, None)?;
        let mut outputs = Vec::new();
        for path in files {
            let final_path = output.join(path.file_name().ok_or("Conversion invalid output name")?);
            fs::rename(path, &final_path).map_err(|_| "Conversion output unavailable")?;
            outputs.push(final_path);
        }
        return Ok(outputs);
    }
    if target == "html" {
        sanitize_html_file(&destination, true)?;
    }
    validate_output(&destination)?;
    let mut result = vec![destination];
    if output.join("converted-assets").is_dir() {
        result.extend(collect_resource_files(
            &output.join("converted-assets"),
            0,
            &[],
        )?);
    }
    Ok(result)
}
fn clean_html(value: &str, local_images: bool) -> String {
    let mut sanitizer = ammonia::Builder::default();
    sanitizer.attribute_filter(move |_, attribute, value| {
        if matches!(
            attribute,
            "src" | "srcset" | "poster" | "background" | "data"
        ) && (!local_images
            || external_reference(value)
            || value.starts_with('/')
            || value.trim().is_empty())
        {
            None
        } else {
            Some(value.into())
        }
    });
    let fragment = sanitizer.clean(value).to_string();
    format!("<!doctype html><html><head><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src 'self'; base-uri 'none'; form-action 'none'\"></head><body>{fragment}</body></html>")
}
fn sanitize_html_file(path: &Path, local_images: bool) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|_| "Conversion invalid HTML encoding")?;
    fs::write(path, clean_html(&text, local_images))
        .map_err(|_| "Conversion output unavailable".into())
}
fn collect_resource_files(
    root: &Path,
    depth: u8,
    excluded: &[PathBuf],
) -> Result<Vec<PathBuf>, String> {
    if depth > 8 {
        return Err("Conversion resource limit".into());
    }
    let mut result = vec![];
    for entry in fs::read_dir(root).map_err(|_| "Conversion output unavailable")? {
        let path = entry.map_err(|_| "Conversion output unavailable")?.path();
        if excluded.contains(&path) {
            continue;
        }
        if path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with('.'))
        {
            continue;
        }
        let meta = fs::symlink_metadata(&path).map_err(|_| "Conversion output unavailable")?;
        if meta.file_attributes() & 0x400 != 0 {
            return Err("Conversion unsafe output".into());
        }
        if meta.is_dir() {
            result.extend(collect_resource_files(&path, depth + 1, excluded)?);
        } else {
            validate_raster_resource(&path)?;
            result.push(path);
        }
        if result.len() > 1000 {
            return Err("Conversion resource limit".into());
        }
    }
    Ok(result)
}
fn validate_raster_resource(path: &Path) -> Result<(), String> {
    let expected = image::ImageFormat::from_extension(extension(path))
        .filter(|format| {
            matches!(
                format,
                image::ImageFormat::Png
                    | image::ImageFormat::Jpeg
                    | image::ImageFormat::WebP
                    | image::ImageFormat::Bmp
                    | image::ImageFormat::Tiff
                    | image::ImageFormat::Gif
                    | image::ImageFormat::Ico
            )
        })
        .ok_or("Conversion unsafe document resource")?;
    if fs::metadata(path)
        .map_err(|_| "Conversion resource unavailable")?
        .len()
        > 200 * 1024 * 1024
    {
        return Err("Conversion resource size limit".into());
    }
    let mut reader = image::ImageReader::open(path)
        .map_err(|_| "Conversion resource unavailable")?
        .with_guessed_format()
        .map_err(|_| "Conversion invalid image resource")?;
    if reader.format() != Some(expected) {
        return Err("Conversion invalid image resource".into());
    }
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(256 * 1024 * 1024);
    limits.max_image_width = Some(50_000_000);
    limits.max_image_height = Some(50_000_000);
    reader.limits(limits);
    let decoder = reader
        .into_decoder()
        .map_err(|_| "Conversion invalid image resource")?;
    use image::ImageDecoder;
    let (width, height) = decoder.dimensions();
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 50_000_000 {
        return Err("Conversion resource pixel limit".into());
    }
    image::DynamicImage::from_decoder(decoder).map_err(|_| "Conversion invalid image resource")?;
    Ok(())
}
fn collect_outputs(directory: &Path, target: &str) -> Result<Vec<PathBuf>, String> {
    let mut result = vec![];
    for entry in fs::read_dir(directory).map_err(|_| "Conversion output unavailable")? {
        let path = entry.map_err(|_| "Conversion output unavailable")?.path();
        if extension(&path) == target && is_plain_file(&path) {
            validate_output(&path)?;
            result.push(path);
        }
    }
    if result.is_empty() {
        return Err("Conversion empty output".into());
    }
    result.sort();
    Ok(result)
}
fn validate_output(path: &Path) -> Result<(), String> {
    let mut file = File::open(path).map_err(|_| "Conversion output unavailable")?;
    let length = file
        .metadata()
        .map_err(|_| "Conversion output unavailable")?
        .len();
    if length == 0 || length > PACKAGE_LIMIT {
        return Err("Conversion output size limit".into());
    }
    let mut bytes = [0u8; 8];
    let count = file
        .read(&mut bytes)
        .map_err(|_| "Conversion output unavailable")?;
    let header = &bytes[..count];
    let valid = match extension(path).as_str() {
        "pdf" => header.starts_with(b"%PDF-"),
        "docx" | "xlsx" | "pptx" | "odt" | "ods" | "odp" => header.starts_with(b"PK\x03\x04"),
        "doc" | "xls" | "ppt" => {
            header.starts_with(&[0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1])
        }
        "png" => header.starts_with(b"\x89PNG\r\n\x1a\n"),
        "jpg" | "jpeg" => header.starts_with(&[0xff, 0xd8]),
        "rtf" => header.starts_with(b"{\\rtf"),
        _ => true,
    };
    if valid {
        Ok(())
    } else {
        Err("Conversion invalid output format".into())
    }
}
fn reject_remote_resources(source: &Path) -> Result<(), String> {
    match extension(source).as_str() {
        "docx" | "xlsx" | "pptx" | "odt" | "ods" | "odp" => {
            let mut archive = zip::ZipArchive::new(
                File::open(source).map_err(|_| "Conversion source unavailable")?,
            )
            .map_err(|_| "Conversion invalid document")?;
            if archive.len() > 20000 {
                return Err("Conversion document entry limit".into());
            }
            let mut expanded = 0u64;
            for index in 0..archive.len() {
                let item = archive
                    .by_index(index)
                    .map_err(|_| "Conversion invalid document")?;
                let name = item
                    .name()
                    .map_err(|_| "Conversion invalid document name")?
                    .into_owned();
                if name.ends_with(".rels") || name.ends_with(".xml") {
                    if item.size() > 8 * 1024 * 1024 {
                        return Err("Conversion document relationship limit".into());
                    }
                    expanded = expanded
                        .checked_add(item.size())
                        .filter(|size| *size <= 64 * 1024 * 1024)
                        .ok_or("Conversion document XML limit")?;
                    let mut text = String::new();
                    item.take(8 * 1024 * 1024 + 1)
                        .read_to_string(&mut text)
                        .map_err(|_| "Conversion invalid document encoding")?;
                    validate_document_part(
                        &text,
                        if name.ends_with(".rels") {
                            name.split('/').count().saturating_sub(2)
                        } else {
                            0
                        },
                    )?;
                }
            }
        }
        "html" | "htm" | "md" | "markdown" => {
            let text =
                fs::read_to_string(source).map_err(|_| "Conversion invalid text encoding")?;
            let lower = text.to_ascii_lowercase();
            if lower.contains("<script")
                || lower.contains("<iframe")
                || lower.contains("<object")
                || lower.contains("<embed")
                || lower.contains("<link")
                || lower.contains("<base")
                || lower.contains("<meta http-equiv")
                || lower.contains("<img")
                || lower.contains("![")
                || lower.contains("@import")
                || lower.contains("url(")
                || lower.contains("file:")
            {
                return Err("Conversion document external resources blocked".into());
            }
        }
        _ => {}
    }
    Ok(())
}

fn external_reference(value: &str) -> bool {
    let value = value.trim();
    value.contains(':')
        || value.contains('\\')
        || value.starts_with("//")
        || value.split('/').any(|part| part == "..")
}
fn network_formula(value: &str) -> bool {
    let value = value
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect::<String>()
        .to_ascii_uppercase();
    [
        "WEBSERVICE(",
        "FILTERXML(",
        "IMPORTXML(",
        "IMPORTDATA(",
        "DDE(",
        "DDEAUTO",
        "INCLUDEPICTURE",
        "INCLUDETEXT",
    ]
    .iter()
    .any(|token| value.contains(token))
}
#[cfg(test)]
fn validate_document_xml(text: &str) -> Result<(), String> {
    validate_document_part(text, 0)
}
fn relationship_reference(value: &str, mut depth: usize) -> bool {
    let value = value.trim();
    if value.contains(':') || value.contains('\\') || value.starts_with("//") {
        return true;
    }
    if value.starts_with('/') {
        depth = 0;
    }
    for part in value.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                let Some(next) = depth.checked_sub(1) else {
                    return true;
                };
                depth = next;
            }
            _ => depth += 1,
        }
    }
    false
}
fn validate_document_part(text: &str, relationship_depth: usize) -> Result<(), String> {
    use quick_xml::{events::Event, Reader, XmlVersion};
    let mut reader = Reader::from_str(text);
    let mut content = String::new();
    loop {
        match reader
            .read_event()
            .map_err(|_| "Conversion invalid document XML")?
        {
            Event::Start(element) | Event::Empty(element) => {
                for attribute in element.attributes() {
                    let attribute = attribute.map_err(|_| "Conversion invalid document XML")?;
                    let key = String::from_utf8_lossy(attribute.key.as_ref());
                    if key == "xmlns" || key.starts_with("xmlns:") {
                        continue;
                    }
                    let local = key
                        .rsplit(':')
                        .next()
                        .unwrap_or_default()
                        .to_ascii_lowercase();
                    let value = attribute
                        .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
                        .map_err(|_| "Conversion invalid document XML")?;
                    if (local == "targetmode" && !value.eq_ignore_ascii_case("internal"))
                        || (local == "target" && relationship_reference(&value, relationship_depth))
                        || (matches!(
                            local.as_str(),
                            "href"
                                | "src"
                                | "action"
                                | "datasource"
                                | "connection"
                                | "connectionstring"
                                | "sourcefile"
                        ) && external_reference(&value))
                        || network_formula(&value)
                    {
                        return Err("Conversion document external resources blocked".into());
                    }
                }
            }
            Event::Text(value) => {
                let decoded = value
                    .xml_content(XmlVersion::Implicit1_0)
                    .map_err(|_| "Conversion invalid document XML")?;
                let text = quick_xml::escape::unescape(&decoded)
                    .map_err(|_| "Conversion invalid document XML")?;
                content.push_str(&text);
            }
            Event::CData(value) => {
                content.push_str(
                    &value
                        .xml_content(XmlVersion::Implicit1_0)
                        .map_err(|_| "Conversion invalid document XML")?,
                );
            }
            Event::GeneralRef(value) => {
                if let Some(character) = value
                    .resolve_char_ref()
                    .map_err(|_| "Conversion invalid document XML")?
                {
                    content.push(character);
                } else {
                    let reference: &[u8] = value.as_ref();
                    match reference {
                        b"amp" => content.push('&'),
                        b"lt" => content.push('<'),
                        b"gt" => content.push('>'),
                        b"quot" => content.push('"'),
                        b"apos" => content.push('\''),
                        _ => return Err("Conversion invalid document XML".into()),
                    }
                }
            }
            Event::DocType(_) => {
                return Err("Conversion document external resources blocked".into())
            }
            Event::Eof => {
                return if network_formula(&content) {
                    Err("Conversion document external resources blocked".into())
                } else {
                    Ok(())
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn test_directory() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "supersearch-components-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        path
    }
    fn crafted_media_document(source: &Path, destination: &Path) {
        let mut archive = zip::ZipArchive::new(File::open(source).unwrap()).unwrap();
        let mut writer = zip::ZipWriter::new(File::create(destination).unwrap());
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).unwrap();
            let mut name = entry.name().unwrap().into_owned();
            if entry.is_dir() {
                continue;
            }
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            if name == "word/media/image1.png" {
                name = "word/media/sidecar.exe".into();
                bytes = b"MZinert-malicious-fixture".to_vec();
            } else if name == "word/_rels/document.xml.rels" || name == "[Content_Types].xml" {
                let value = String::from_utf8(bytes).unwrap();
                bytes = value
                    .replace("media/image1.png", "media/sidecar.exe")
                    .replace("Extension=\"png\"", "Extension=\"exe\"")
                    .into_bytes();
            }
            writer
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(&bytes).unwrap();
        }
        writer.finish().unwrap();
    }

    #[test]
    fn published_screen_ocr_has_a_pinned_download_and_new_companion_version() {
        let root = test_directory();
        let executable = root.join("SuperSearch.exe");
        let item = manifest("ocr").unwrap();
        assert_eq!(item.version, "1.0.1");
        assert!(matches!(install_source(&executable, &item).unwrap(), InstallSource::Download(_)));
        fs::create_dir(root.join("components")).unwrap();
        let path = root.join("components/supersearch-ocr-component-1.0.1-windows-x64.zip");
        fs::write(&path, b"fixture; activation checks exact hash").unwrap();
        assert_eq!(adjacent_package(&executable, &item).unwrap(), Some(path));
        remove_private_dir(root.parent().unwrap(), &root).unwrap();
    }

    #[test]
    fn adjacent_install_uses_only_the_pinned_companion_filename() {
        let root = test_directory();
        let executable = root.join("SuperSearch.exe");
        let item = manifest("pdf").unwrap();
        fs::create_dir(root.join("components")).unwrap();
        let path = root.join("components/supersearch-pdf-component-1.0.0-windows-x64.zip");
        fs::write(&path, b"fixture; integrity is checked during activation").unwrap();
        assert_eq!(
            adjacent_package(&executable, &item).unwrap(),
            Some(path.clone())
        );
        fs::rename(&path, root.join("components/renamed.zip")).unwrap();
        assert_eq!(adjacent_package(&executable, &item).unwrap(), None);
        fs::write(
            root.join("supersearch-pdf-component-1.0.0-windows-x64.zip"),
            b"outside fixed directory",
        )
        .unwrap();
        assert_eq!(adjacent_package(&executable, &item).unwrap(), None);
        remove_private_dir(root.parent().unwrap(), &root).unwrap();
    }

    #[test]
    fn adjacent_install_rejects_directory_and_junction_instead_of_following_it() {
        let root = test_directory();
        let executable = root.join("SuperSearch.exe");
        let item = manifest("ocr").unwrap();
        fs::create_dir(root.join("components")).unwrap();
        let path = root.join("components/supersearch-ocr-component-1.0.1-windows-x64.zip");
        fs::create_dir(&path).unwrap();
        assert!(adjacent_package(&executable, &item).is_err());
        fs::remove_dir(path).unwrap();
        fs::remove_dir(root.join("components")).unwrap();
        let outside = test_directory();
        let result = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(root.join("components"))
            .arg(&outside)
            .creation_flags(0x08000000)
            .output()
            .unwrap();
        assert!(result.status.success());
        // 联接目录即使尚无同名文件，也不能被视作可信安装源。
        assert!(adjacent_package(&executable, &item).is_err());
        fs::remove_dir(root.join("components")).unwrap();
        remove_private_dir(root.parent().unwrap(), &root).unwrap();
        remove_private_dir(outside.parent().unwrap(), &outside).unwrap();
    }

    #[test]
    fn companion_activation_rejects_tampering_and_keeps_the_ready_version() {
        let root = test_directory();
        let item = manifest("pdf").unwrap();
        let ready = install_dir(&root, &item);
        fs::create_dir_all(&ready).unwrap();
        fs::write(ready.join("previous"), b"keep verified version").unwrap();
        let package = root.join("tampered.zip");
        fs::write(&package, b"tampered").unwrap();
        let cancel = AtomicBool::new(false);
        assert_eq!(
            activate_package(&root, &item, &package, &cancel)
                .err()
                .as_deref(),
            Some("Conversion component size mismatch")
        );
        assert_eq!(
            fs::read(ready.join("previous")).unwrap(),
            b"keep verified version"
        );
        let mut same_size = item.clone();
        same_size.download.as_mut().unwrap().bytes = 8;
        assert_eq!(
            activate_package(&root, &same_size, &package, &cancel)
                .err()
                .as_deref(),
            Some("Conversion component checksum mismatch")
        );
        assert!(ready.join("previous").is_file());
        remove_private_dir(root.parent().unwrap(), &root).unwrap();
    }

    #[test]
    fn install_source_prefers_companion_then_pinned_download_and_reports_unpublished() {
        let root = test_directory();
        let executable = root.join("SuperSearch.exe");
        let item = manifest("pandoc").unwrap();
        assert!(matches!(
            install_source(&executable, &item).unwrap(),
            InstallSource::Download(_)
        ));
        fs::create_dir(root.join("components")).unwrap();
        let companion = root.join("components/pandoc-3.12.1-windows-x86_64.zip");
        fs::write(&companion, b"fixture").unwrap();
        assert!(
            matches!(install_source(&executable, &item).unwrap(), InstallSource::Adjacent(path) if path == companion)
        );
        let mut unpublished = manifest("pdf").unwrap();
        unpublished.download.as_mut().unwrap().url.clear();
        assert_eq!(
            install_source(&executable, &unpublished).err().as_deref(),
            Some("Conversion component requires local package")
        );
        let mut absent = unpublished;
        absent.download = None;
        assert_eq!(
            install_source(&executable, &absent).err().as_deref(),
            Some("Conversion component package is not published in this build")
        );
        remove_private_dir(root.parent().unwrap(), &root).unwrap();
    }

    fn fixture_archive(path: &Path, name: &str, content: &[u8]) {
        let mut writer = zip::ZipWriter::new(File::create(path).unwrap());
        writer
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(content).unwrap();
        writer.finish().unwrap();
    }

    #[test]
    fn extraction_keeps_nested_files_and_rejects_escape_paths() {
        let root = test_directory();
        let package = root.join("fixture.zip");
        let output = root.join("output");
        fs::create_dir(&output).unwrap();
        let cancel = AtomicBool::new(false);
        fixture_archive(
            &package,
            "pandoc-3.12.1/pandoc.exe",
            b"inert executable fixture",
        );
        unpack(File::open(&package).unwrap(), &output, 100, &cancel).unwrap();
        assert_eq!(
            fs::read(output.join("pandoc-3.12.1/pandoc.exe")).unwrap(),
            b"inert executable fixture"
        );
        fixture_archive(&package, "../escaped.exe", b"do not write");
        assert_eq!(
            unpack(File::open(&package).unwrap(), &output, 100, &cancel)
                .err()
                .as_deref(),
            Some("Conversion invalid package path")
        );
        assert!(!root.join("escaped.exe").exists());
        remove_private_dir(root.parent().unwrap(), &root).unwrap();
    }

    #[test]
    fn invalid_archives_or_missing_entries_do_not_replace_a_ready_component() {
        let root = test_directory();
        let mut item = manifest("pdf").unwrap();
        let ready = install_dir(&root, &item);
        fs::create_dir_all(&ready).unwrap();
        fs::write(ready.join("previous"), b"keep verified version").unwrap();
        let package = root.join("fixture.zip");
        let cancel = AtomicBool::new(false);
        for (valid_zip, expected) in [
            (false, "Conversion invalid package ZIP"),
            (true, "Conversion component entry missing"),
        ] {
            if valid_zip {
                fixture_archive(&package, "unrelated.txt", b"not the helper");
            } else {
                fs::write(&package, b"invalid zip").unwrap();
            }
            let bytes = fs::read(&package).unwrap();
            let download = item.download.as_mut().unwrap();
            download.bytes = bytes.len() as u64;
            download.sha256 = format!("{:x}", Sha256::digest(&bytes));
            assert_eq!(
                activate_package(&root, &item, &package, &cancel)
                    .err()
                    .as_deref(),
                Some(expected)
            );
            assert_eq!(
                fs::read(ready.join("previous")).unwrap(),
                b"keep verified version"
            );
            assert!(!root.join("components/pdf/.1.0.0-staging").exists());
        }
        remove_private_dir(root.parent().unwrap(), &root).unwrap();
    }

    #[test]
    fn startup_removes_only_owned_install_temporary_and_recovers_backup() {
        let root = test_directory();
        let known = manifests().unwrap();
        let folder = root.join("components/pdf");
        fs::create_dir_all(folder.join(".1.0.0-staging")).unwrap();
        fs::create_dir(folder.join(".1.0.0-backup")).unwrap();
        fs::write(folder.join(".1.0.0-backup/kept"), b"verified old").unwrap();
        fs::create_dir_all(root.join("components/.downloads")).unwrap();
        fs::write(
            root.join("components/.downloads/pdf.partial"),
            b"incomplete",
        )
        .unwrap();
        fs::write(root.join("components/keep-user-file"), b"keep").unwrap();
        cleanup_install_temporary(&root, &known).unwrap();
        assert!(!folder.join(".1.0.0-staging").exists());
        assert!(folder.join("1.0.0/kept").exists());
        assert!(!root.join("components/.downloads/pdf.partial").exists());
        assert!(root.join("components/keep-user-file").exists());
        remove_private_dir(root.parent().unwrap(), &root).unwrap();
    }

    #[test]
    fn cancelled_network_wait_returns_without_waiting_for_server() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let server = std::thread::spawn(move || {
            let started = std::time::Instant::now();
            while started.elapsed() < Duration::from_secs(3) {
                match listener.accept() {
                    Ok((_stream, _)) => {
                        std::thread::sleep(Duration::from_secs(2));
                        return;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10))
                    }
                    Err(_) => return,
                }
            }
        });
        let cancel = Arc::new(AtomicBool::new(false));
        let signal = cancel.clone();
        let cancelling = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            signal.store(true, Ordering::Release);
        });
        let root = test_directory();
        let started = std::time::Instant::now();
        let result = tauri::async_runtime::block_on(download_package(
            &Download {
                url: format!("https://127.0.0.1:{port}/package"),
                bytes: 1,
                sha256: "a".repeat(64),
                hosts: vec!["127.0.0.1".into()],
            },
            &root.join("cancelled.partial"),
            &cancel,
        ));
        assert_eq!(result.err().as_deref(), Some("Conversion cancelled"));
        assert!(started.elapsed() < Duration::from_secs(1));
        cancelling.join().unwrap();
        server.join().unwrap();
        remove_private_dir(root.parent().unwrap(), &root).unwrap();
    }
    #[test]
    #[ignore = "requires built local component ZIPs and isolated downloaded Office/Pandoc fixtures"]
    fn real_component_import_and_document_adapters() {
        let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        let verification = project.join("test-results/components-verification");
        let fixtures = verification.join("fixtures");
        let artifacts = project.join("artifacts/releases/Public-PDF-2026-10-25");
        let root = test_directory();
        ComponentState::new(root.clone()).unwrap();
        let cancel = AtomicBool::new(false);
        fs::write(
            fixtures.join("malicious.md"),
            r#"<svg onload="alert(1)"><text>Hello</text></svg>"#,
        )
        .unwrap();
        fs::write(fixtures.join("malicious.html"),r#"<a href="javascript:alert(1)">click</a><svg onload="alert(1)"><text>Hello</text></svg><img src="https://example.test/tracker">"#).unwrap();
        for (id, package) in [
            ("pandoc", verification.join("pandoc.zip")),
            (
                "pdf",
                artifacts.join("supersearch-pdf-component-1.0.0-windows-x64.zip"),
            ),
            (
                "ocr",
                project.join("artifacts/releases/Public-OCR-2026-10-25/supersearch-ocr-component-1.0.1-windows-x64.zip"),
            ),
        ] {
            activate_package(&root, &manifest(id).unwrap(), &package, &cancel)
                .unwrap_or_else(|error| panic!("install {id}: {error}"));
            assert!(executable(&root, id).is_ok());
            println!("verified native import/probe: {id}");
        }
        // 删除已安装入口后，使用同一个已校验组件包进行原子修复。
        fs::remove_file(executable(&root, "pdf").unwrap()).unwrap();
        activate_package(
            &root,
            &manifest("pdf").unwrap(),
            &artifacts.join("supersearch-pdf-component-1.0.0-windows-x64.zip"),
            &cancel,
        )
        .unwrap();
        for target in ["png", "jpg", "txt", "docx", "xlsx", "csv", "pptx"] {
            let output = root.join(format!("pdf-{target}"));
            fs::create_dir(&output).unwrap();
            let options = ConversionOptions {
                target: target.into(),
                ..Default::default()
            };
            let outputs = convert(
                &root,
                &fixtures.join("table.pdf"),
                &output,
                &options,
                &cancel,
            )
            .unwrap_or_else(|error| panic!("PDF -> {target}: {error}"));
            assert!(outputs
                .iter()
                .all(|path| path.strip_prefix(&output).is_ok() && path.is_file()));
            println!(
                "verified native helper: PDF -> {target} ({} outputs)",
                outputs.len()
            );
        }
        let output = root.join("no-table");
        fs::create_dir(&output).unwrap();
        let error = convert(
            &root,
            &fixtures.join("no-table.pdf"),
            &output,
            &ConversionOptions {
                target: "xlsx".into(),
                ..Default::default()
            },
            &cancel,
        )
        .unwrap_err();
        assert!(error.to_ascii_lowercase().contains("table"), "{error}");
        println!("verified native helper no-table error: {error}");
        let output = root.join("merged");
        fs::create_dir(&output).unwrap();
        assert_eq!(
            convert_many(
                &root,
                &[fixtures.join("image-0.png"), fixtures.join("image-1.png")],
                &output,
                &ConversionOptions {
                    target: "pdf".into(),
                    merge: true,
                    ..Default::default()
                },
                &cancel
            )
            .unwrap()
            .len(),
            1
        );
        for target in ["txt", "docx", "pdf"] {
            let output = root.join(format!("ocr-{target}"));
            fs::create_dir(&output).unwrap();
            convert(
                &root,
                &fixtures.join("ocr.png"),
                &output,
                &ConversionOptions {
                    target: target.into(),
                    ocr: true,
                    ..Default::default()
                },
                &cancel,
            )
            .unwrap_or_else(|error| panic!("OCR -> {target}: {error}"));
            println!("verified native OCR: image -> {target}");
        }
        for (input, target) in [
            ("notes.md", "html"),
            ("malicious.md", "html"),
            ("malicious.html", "html"),
            ("notes.html", "md"),
            ("notes.txt", "docx"),
            ("document.docx", "md"),
            ("with-image.docx", "md"),
        ] {
            let output = root.join(format!("pandoc-{input}-{target}"));
            fs::create_dir(&output).unwrap();
            let outputs = convert(
                &root,
                &fixtures.join(input),
                &output,
                &ConversionOptions {
                    target: target.into(),
                    ..Default::default()
                },
                &cancel,
            )
            .unwrap_or_else(|error| panic!("Pandoc {input}->{target}: {error}"));
            if input == "with-image.docx" {
                let text = fs::read_to_string(&outputs[0]).unwrap();
                assert!(text.contains("converted-assets/media/"));
                assert!(!text.contains(&root.to_string_lossy().to_string()));
                assert!(outputs.len() > 1);
            }
            if target == "html" {
                let html = fs::read_to_string(&outputs[0]).unwrap();
                assert!(!html.contains("<svg"));
                assert!(!html.contains("href=\"javascript:"));
                assert!(!html.contains("src=\"https://example.test"));
                assert!(!html.contains("<script"));
            }
            println!("verified native Pandoc: {input} -> {target}");
        }
        let malicious = fixtures.join("malicious-media.docx");
        crafted_media_document(&fixtures.join("with-image.docx"), &malicious);
        let output = root.join("malicious-media-output");
        fs::create_dir(&output).unwrap();
        let error = convert(
            &root,
            &malicious,
            &output,
            &ConversionOptions {
                target: "md".into(),
                ..Default::default()
            },
            &cancel,
        )
        .unwrap_err();
        assert!(error.contains("resource"), "{error}");
        println!("verified native Pandoc refuses executable media sidecar");
        let office =
            verification.join("libreoffice-private/SourceDir/LibreOffice/program/soffice.com");
        assert!(office.is_file());
        for (input, target, sheet) in [
            ("document.docx", "pdf", None),
            ("workbook.xlsx", "pdf", None),
            ("slides.pptx", "pdf", None),
            ("workbook.xlsx", "csv", Some(2)),
            ("with-image.docx", "html", None),
            ("data.csv", "xlsx", None),
        ] {
            let output = root.join(format!("office-{input}-{target}"));
            fs::create_dir(&output).unwrap();
            let outputs = office_convert(
                &root,
                &fixtures.join(input),
                &output,
                &ConversionOptions {
                    target: target.into(),
                    sheet,
                    ..Default::default()
                },
                &cancel,
                Some(&office),
            )
            .unwrap_or_else(|error| panic!("Office {input}->{target}: {error}"));
            if target == "csv" {
                assert!(fs::read_to_string(&outputs[0])
                    .unwrap()
                    .contains("selected"));
            }
            if target == "html" {
                assert!(outputs.len() > 1, "HTML sidecars missing");
            }
            if input == "data.csv" {
                let mut archive = zip::ZipArchive::new(File::open(&outputs[0]).unwrap()).unwrap();
                let mut xml = String::new();
                archive
                    .by_name("xl/worksheets/sheet1.xml")
                    .unwrap()
                    .read_to_string(&mut xml)
                    .unwrap();
                assert!(!xml.contains("<f>"));
                let mut strings = String::new();
                archive
                    .by_name("xl/sharedStrings.xml")
                    .unwrap()
                    .read_to_string(&mut strings)
                    .unwrap();
                assert!(strings.contains("=1+2"));
            }
            println!("verified native isolated Office: {input} -> {target}");
        }
        let output = root.join("office-chinese-csv-xlsx");
        fs::create_dir(&output).unwrap();
        let outputs = office_convert(
            &root,
            &fixtures.join("chinese-semicolon.csv"),
            &output,
            &ConversionOptions {
                target: "xlsx".into(),
                csv_delimiter: ";".into(),
                csv_encoding: "gb18030".into(),
                ..Default::default()
            },
            &cancel,
            Some(&office),
        )
        .unwrap();
        let mut archive = zip::ZipArchive::new(File::open(&outputs[0]).unwrap()).unwrap();
        let mut strings = String::new();
        archive
            .by_name("xl/sharedStrings.xml")
            .unwrap()
            .read_to_string(&mut strings)
            .unwrap();
        assert!(strings.contains("中文"));
        assert!(strings.contains("=1+2"));
        println!("verified native Office GB18030 semicolon CSV and literal formula");
        let report = json!({"status":"passed","root":root,"directions":["PDF->PNG/JPG/TXT/DOCX/XLSX/CSV/PPTX","merged images->PDF","OCR image->TXT/DOCX/searchablePDF","Markdown/HTML/TXT/DOCX via Pandoc","DOCX/XLSX/PPTX->PDF via isolated LibreOffice","XLSX selected sheet->CSV","DOCX->HTML with image sidecars","CSV->XLSX literal formulas"],"systemInstall":false});
        fs::write(
            verification.join("native-adapter-report.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        println!("Native adapter evidence retained at {}", root.display());
    }
    #[test]
    fn rejects_package_path_traversal_and_windows_ambiguous_paths() {
        for value in [
            "../run.exe",
            "C:/run.exe",
            "a/../run.exe",
            "a\\run.exe",
            "a:run.exe",
            "CON",
            "a./run.exe",
        ] {
            assert!(package_path(value).is_err(), "{value}");
        }
        assert_eq!(
            package_path("runtime/helper.exe").unwrap(),
            PathBuf::from("runtime/helper.exe")
        );
    }
    #[test]
    fn html_sanitizer_removes_executable_content_and_unauthorized_images() {
        let attack = r#"<svg onload="alert(1)"><text>Hello</text></svg><a href="javascript:alert(1)">click</a><img src="https://example.test/tracker" onerror="alert(2)"><img src="assets/picture.png"><script>alert(3)</script>"#;
        let clean = clean_html(attack, true);
        for token in [
            "<svg",
            "onload=",
            "onerror=",
            "javascript:",
            "<script",
            "https://example.test",
        ] {
            assert!(!clean.contains(token), "{token}: {clean}");
        }
        assert!(clean.contains("assets/picture.png"));
        assert!(!clean_html(attack, false).contains("src=\"assets/picture.png\""));
    }
    #[test]
    fn resources_require_inert_raster_extension_and_decodable_matching_content() {
        let root = test_directory();
        let executable = root.join("sidecar.exe");
        fs::write(&executable, b"MZsidecar").unwrap();
        assert!(validate_raster_resource(&executable).is_err());
        let renamed = root.join("sidecar.png");
        fs::write(&renamed, b"MZsidecar").unwrap();
        assert!(validate_raster_resource(&renamed).is_err());
        let svg = root.join("sidecar.svg");
        fs::write(&svg, b"<svg onload='alert(1)'/>").unwrap();
        assert!(validate_raster_resource(&svg).is_err());
        let image = root.join("valid.png");
        image::DynamicImage::new_rgb8(2, 2).save(&image).unwrap();
        assert!(validate_raster_resource(&image).is_ok());
        remove_private_dir(root.parent().unwrap(), &root).unwrap();
    }
    #[test]
    fn xml_external_relationships_are_decoded_before_validation() {
        for xml in [
            r#"<Relationship TargetMode = "External" Target="http://example.test"/>"#,
            r#"<Relationship TargetMode='Ext&#101;rnal' Target='https&#58;//example.test'/>"#,
            r#"<style xmlns:xlink='urn:x' xlink:href='file&#58;///secret.png'/>"#,
            r#"<sheet formula='of:=WEBSERVICE(&quot;https://example.test&quot;)'/>"#,
            r#"<f><![CDATA[WEBSERVICE("https://example.test")]]></f>"#,
            r#"<f>WEBSER<!-- split -->VICE("https://example.test")</f>"#,
            r#"<f>WEBSER&#86;ICE("https://example.test")</f>"#,
            r#"<!DOCTYPE a SYSTEM 'https://example.test'><a/>"#,
        ] {
            assert!(validate_document_xml(xml).is_err(), "{xml}");
        }
        assert!(validate_document_xml(
            r#"<Relationship TargetMode='Internal' Target='media/image.png'/>"#
        )
        .is_ok());
    }
    #[test]
    fn shipped_manifest_has_real_bounded_official_downloads() {
        let items = manifests().unwrap();
        assert_eq!(items.len(), 4);
        assert_eq!(
            items
                .iter()
                .find(|i| i.id == "pandoc")
                .unwrap()
                .download
                .as_ref()
                .unwrap()
                .bytes,
            42154914
        );
        assert_eq!(
            items
                .iter()
                .find(|i| i.id == "office")
                .unwrap()
                .download
                .as_ref()
                .unwrap()
                .bytes,
            374870016
        );
    }
    #[test]
    fn office_csv_requires_explicit_sheet_and_disables_formula_import() {
        assert!(office_filter("xlsx", "csv", None).is_err());
        assert!(office_filter("xlsx", "csv", Some(2))
            .unwrap()
            .contains(",2,false,true,false"));
        assert!(office_filter("pptx", "xlsx", None).is_err());
        assert_eq!(
            office_filter("docx", "pdf", None).unwrap(),
            "pdf:writer_pdf_Export"
        );
    }
    #[test]
    fn never_accepts_renamed_pdf_output() {
        let path = std::env::temp_dir().join(format!(
            "supersearch-invalid-output-{}.pdf",
            std::process::id()
        ));
        fs::write(&path, b"not a pdf").unwrap();
        assert!(validate_output(&path).is_err());
        fs::remove_file(path).unwrap();
    }
}
