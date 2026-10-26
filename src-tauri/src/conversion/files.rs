use super::{check_cancel, unique_id, OUTPUT_LIMIT};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::windows::{
        fs::{MetadataExt, OpenOptionsExt},
        io::AsRawHandle,
    },
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};
use windows::{
    core::{w, PCWSTR, PWSTR},
    Win32::{
        Foundation::HANDLE,
        Storage::FileSystem::{
            GetDriveTypeW, GetFileInformationByHandle, GetFinalPathNameByHandleW, MoveFileExW,
            BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
            FILE_NAME_NORMALIZED, FILE_READ_ATTRIBUTES, MOVE_FILE_FLAGS,
        },
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER,
            COINIT_APARTMENTTHREADED,
        },
        UI::Shell::{
            FileOpenDialog, IFileOpenDialog, FOS_ALLOWMULTISELECT, FOS_FILEMUSTEXIST,
            FOS_FORCEFILESYSTEM, FOS_NOCHANGEDIR, FOS_PATHMUSTEXIST, FOS_PICKFOLDERS,
            SIGDN_FILESYSPATH,
        },
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FileIdentity {
    volume: u32,
    high: u32,
    low: u32,
    bytes: u64,
    modified: u64,
}

pub(super) fn identity(file: &File) -> Result<FileIdentity, String> {
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    unsafe { GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut info) }
        .map_err(|_| "Conversion file identity unavailable")?;
    let directory = info.dwFileAttributes & 0x10 != 0;
    Ok(FileIdentity {
        volume: info.dwVolumeSerialNumber,
        high: info.nFileIndexHigh,
        low: info.nFileIndexLow,
        bytes: if directory {
            0
        } else {
            (u64::from(info.nFileSizeHigh) << 32) | u64::from(info.nFileSizeLow)
        },
        modified: if directory {
            0
        } else {
            (u64::from(info.ftLastWriteTime.dwHighDateTime) << 32)
                | u64::from(info.ftLastWriteTime.dwLowDateTime)
        },
    })
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

fn valid_segment(value: &str) -> bool {
    let stem = value.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$" | "CONIN$" | "CONOUT$"
    ) || ["COM", "LPT"].iter().any(|prefix| {
        stem.strip_prefix(prefix).is_some_and(|tail| {
            matches!(
                tail,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    });
    !value.is_empty()
        && !matches!(value, "." | "..")
        && !value.ends_with([' ', '.'])
        && !reserved
        && !value.chars().any(|c| {
            c.is_control() || matches!(c, ':' | '*' | '?' | '<' | '>' | '|' | '"' | '/' | '\\')
        })
}

pub(super) fn normal_path(path: &Path) -> Result<PathBuf, String> {
    let text = path
        .to_str()
        .ok_or("Conversion invalid local path")?
        .replace('/', "\\");
    let text = text.strip_prefix(r"\\?\").unwrap_or(&text);
    let bytes = text.as_bytes();
    if bytes.len() < 3
        || bytes.len() > 32760
        || !bytes[0].is_ascii_alphabetic()
        || bytes[1] != b':'
        || bytes[2] != b'\\'
    {
        return Err("Conversion local regular file required".into());
    }
    if text.len() > 3 && !text[3..].split('\\').all(valid_segment) {
        return Err("Conversion invalid local path".into());
    }
    let root = wide(&text[..3]);
    if !matches!(
        unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) },
        2 | 3 | 5 | 6
    ) {
        return Err("Conversion local regular file required".into());
    }
    Ok(PathBuf::from(text))
}

pub(super) struct DirectoryGrant {
    pub path: PathBuf,
    pub identity: FileIdentity,
    _handles: Vec<File>,
}

impl DirectoryGrant {
    pub fn open(path: &Path) -> Result<Self, String> {
        let path = normal_path(path)?;
        let text = path.to_str().ok_or("Conversion invalid output path")?;
        let mut current = PathBuf::from(&text[..3]);
        let mut handles = Vec::new();
        let mut entries = vec![current.clone()];
        for part in text[3..].split('\\').filter(|value| !value.is_empty()) {
            current.push(part);
            entries.push(current.clone());
        }
        for entry in entries {
            let meta = fs::symlink_metadata(&entry)
                .map_err(|_| "Conversion output directory unavailable")?;
            if !meta.is_dir() || meta.file_attributes() & 0x400 != 0 {
                return Err("Conversion local directory required".into());
            }
            // 禁止删除共享，固定输出目录及全部祖先，避免写入期间目录被换为联接点。
            let handle = OpenOptions::new()
                .access_mode(FILE_READ_ATTRIBUTES.0)
                .share_mode(3)
                .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0 | FILE_FLAG_OPEN_REPARSE_POINT.0)
                .open(&entry)
                .map_err(|_| "Conversion output directory unavailable")?;
            if handle
                .metadata()
                .map_err(|_| "Conversion output directory unavailable")?
                .file_attributes()
                & 0x400
                != 0
            {
                return Err("Conversion local directory required".into());
            }
            let mut actual = vec![0u16; 32768];
            let count = unsafe {
                GetFinalPathNameByHandleW(
                    HANDLE(handle.as_raw_handle()),
                    &mut actual,
                    FILE_NAME_NORMALIZED,
                )
            } as usize;
            if count == 0 || count >= actual.len() {
                return Err("Conversion directory identity unavailable".into());
            }
            let actual = String::from_utf16(&actual[..count])
                .map_err(|_| "Conversion directory identity unavailable")?;
            let actual = normal_path(Path::new(&actual))?;
            if !actual
                .to_string_lossy()
                .eq_ignore_ascii_case(&entry.to_string_lossy())
            {
                return Err("Conversion directory changed".into());
            }
            handles.push(handle);
        }
        let identity = identity(handles.last().ok_or("Conversion directory unavailable")?)?;
        Ok(Self {
            path,
            identity,
            _handles: handles,
        })
    }

    pub fn create_unique_child(&self, stem: &str) -> Result<Self, String> {
        let stem = stem.chars().take(80).collect::<String>();
        if !valid_segment(&stem) {
            return Err("Conversion invalid output name".into());
        }
        for index in 0..10000 {
            let name = if index == 0 {
                format!("{stem} (转换)")
            } else {
                format!("{stem} (转换 {index})")
            };
            let path = self.path.join(name);
            match fs::create_dir(&path) {
                Ok(()) => return Self::open(&path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => return Err("Conversion cannot create output directory".into()),
            }
        }
        Err("Conversion output name capacity reached".into())
    }

    pub fn create_relative_child(&self, relative: &Path) -> Result<Self, String> {
        if relative.is_absolute() {
            return Err("Conversion invalid output path".into());
        }
        let mut current = self.path.clone();
        for part in relative.components() {
            let std::path::Component::Normal(value) = part else {
                return Err("Conversion invalid output path".into());
            };
            if !valid_segment(value.to_str().ok_or("Conversion invalid output path")?) {
                return Err("Conversion invalid output path".into());
            }
            current.push(value);
            match fs::create_dir(&current) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(_) => return Err("Conversion cannot create output directory".into()),
            }
            // 下一级路径使用前确认当前目录仍为常规本地目录。
            Self::open(&current)?;
        }
        Self::open(&current)
    }
}

pub(super) fn validate_staged_output(path: &Path, root: &Path) -> Result<(), String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| "Conversion invalid output location")?;
    if relative.as_os_str().is_empty() {
        return Err("Conversion invalid output location".into());
    }
    let mut current = root.to_path_buf();
    for part in relative.components() {
        let std::path::Component::Normal(name) = part else {
            return Err("Conversion invalid output location".into());
        };
        if !valid_segment(name.to_str().ok_or("Conversion invalid output name")?) {
            return Err("Conversion invalid output name".into());
        }
        current.push(name);
        let meta = fs::symlink_metadata(&current).map_err(|_| "Conversion output unavailable")?;
        if meta.file_attributes() & 0x400 != 0 {
            return Err("Conversion invalid output location".into());
        }
    }
    let meta = fs::metadata(path).map_err(|_| "Conversion output unavailable")?;
    if !meta.is_file() || meta.len() == 0 || meta.len() > OUTPUT_LIMIT {
        return Err("Conversion invalid output file".into());
    }
    Ok(())
}

pub(super) fn commit_output(
    staged: &Path,
    directory: &DirectoryGrant,
    name: &str,
    cancel: &AtomicBool,
) -> Result<PathBuf, String> {
    if !valid_segment(name) {
        return Err("Conversion invalid output name".into());
    }
    check_cancel(cancel)?;
    let temp = directory
        .path
        .join(format!(".supersearch-{}.tmp", unique_id()));
    let result = (|| {
        let mut input = File::open(staged).map_err(|_| "Conversion output unavailable")?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|_| "Conversion output directory is not writable")?;
        let mut bytes = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            check_cancel(cancel)?;
            let count = input
                .read(&mut buffer)
                .map_err(|_| "Conversion output unavailable")?;
            if count == 0 {
                break;
            }
            bytes += count as u64;
            if bytes > OUTPUT_LIMIT {
                return Err("Conversion output size limit exceeded".into());
            }
            output
                .write_all(&buffer[..count])
                .map_err(|_| "Conversion output write failed")?;
        }
        output
            .sync_all()
            .map_err(|_| "Conversion output write failed")?;
        drop(output);
        check_cancel(cancel)?;
        let path = Path::new(name);
        let stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .ok_or("Conversion invalid output name")?;
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        let wide_temp = wide(temp.to_str().ok_or("Conversion invalid output name")?);
        for index in 0..10000 {
            check_cancel(cancel)?;
            let name = if index == 0 {
                name.to_owned()
            } else {
                let suffix = if index == 1 {
                    " (转换)".to_owned()
                } else {
                    format!(" (转换 {index})")
                };
                format!(
                    "{stem}{suffix}{}{ext}",
                    if ext.is_empty() { "" } else { "." }
                )
            };
            let destination = directory.path.join(name);
            let wide_destination = wide(
                destination
                    .to_str()
                    .ok_or("Conversion invalid output name")?,
            );
            // 不使用 REPLACE_EXISTING；同目录临时文件确保最终改名原子且不跨卷。
            match unsafe {
                MoveFileExW(
                    PCWSTR(wide_temp.as_ptr()),
                    PCWSTR(wide_destination.as_ptr()),
                    MOVE_FILE_FLAGS(0),
                )
            } {
                Ok(()) => return Ok(destination),
                Err(error) if matches!(error.code().0 as u32, 0x80070050 | 0x800700b7) => continue,
                Err(_) => return Err("Conversion output commit failed".into()),
            }
        }
        Err("Conversion output name capacity reached".into())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

pub(super) fn clean_owned_directory(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let meta = fs::symlink_metadata(path).map_err(|_| "Conversion cleanup failed")?;
    if !meta.is_dir() || meta.file_attributes() & 0x400 != 0 {
        return Err("Conversion invalid temporary directory".into());
    }
    for entry in fs::read_dir(path).map_err(|_| "Conversion cleanup failed")? {
        let entry = entry.map_err(|_| "Conversion cleanup failed")?;
        let meta = fs::symlink_metadata(entry.path()).map_err(|_| "Conversion cleanup failed")?;
        if meta.file_attributes() & 0x400 != 0 {
            return Err("Conversion invalid temporary directory".into());
        }
        if meta.is_dir() {
            clean_owned_directory(&entry.path())?;
        } else {
            fs::remove_file(entry.path()).map_err(|_| "Conversion cleanup failed")?;
        }
    }
    fs::remove_dir(path).map_err(|_| "Conversion cleanup failed".into())
}

struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
unsafe fn shell_string(value: PWSTR) -> Result<String, String> {
    let result = value
        .to_string()
        .map_err(|_| "Conversion invalid picker path".to_string());
    CoTaskMemFree(Some(value.0.cast()));
    result
}

pub(super) fn choose(folder: bool) -> Result<Vec<PathBuf>, String> {
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() }
        .map_err(|_| "Conversion picker unavailable")?;
    let _apartment = Apartment;
    unsafe {
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
            .map_err(|_| "Conversion picker unavailable")?;
        let flags = FOS_PATHMUSTEXIST
            | FOS_FORCEFILESYSTEM
            | FOS_NOCHANGEDIR
            | if folder {
                FOS_PICKFOLDERS
            } else {
                FOS_FILEMUSTEXIST | FOS_ALLOWMULTISELECT
            };
        dialog
            .SetOptions(flags)
            .map_err(|_| "Conversion picker unavailable")?;
        dialog
            .SetTitle(if folder {
                w!("选择转换输出目录")
            } else {
                w!("选择需要转换的图片或文档")
            })
            .map_err(|_| "Conversion picker unavailable")?;
        if let Err(error) = dialog.Show(None) {
            if error.code().0 as u32 == 0x800704c7 {
                return Ok(vec![]);
            }
            return Err("Conversion picker unavailable".into());
        }
        let results = dialog
            .GetResults()
            .map_err(|_| "Conversion picker unavailable")?;
        let count = results
            .GetCount()
            .map_err(|_| "Conversion picker unavailable")?;
        if count > 100 {
            return Err("Conversion select at most 100 files".into());
        }
        let mut paths = Vec::new();
        for index in 0..count {
            let item = results
                .GetItemAt(index)
                .map_err(|_| "Conversion picker unavailable")?;
            let value = shell_string(
                item.GetDisplayName(SIGDN_FILESYSPATH)
                    .map_err(|_| "Conversion local file required")?,
            )?;
            paths.push(normal_path(Path::new(&value))?);
        }
        Ok(paths)
    }
}
