use crate::model::Entry;
use std::{collections::HashSet, fs, path::{Path, PathBuf}, ptr};
use windows::{core::{w, Interface, PCWSTR, PWSTR}, Win32::{
    Foundation::{GlobalFree, HANDLE},
    System::{Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, STGM_READ},
        DataExchange::{OpenClipboard, CloseClipboard, EmptyClipboard, SetClipboardData},
        Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE}},
    UI::{Shell::{*, Common::COMDLG_FILTERSPEC}, WindowsAndMessaging::{CreateWindowExW, DestroyWindow, HWND_MESSAGE, SW_SHOWNORMAL, WINDOW_EX_STYLE, WINDOW_STYLE}},
}};

struct Apartment;
impl Apartment {
    fn new() -> Result<Self, String> {
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().map_err(|_| "Unable to initialize Windows Shell.")?; }
        Ok(Self)
    }
}
impl Drop for Apartment { fn drop(&mut self) { unsafe { CoUninitialize(); } } }

fn wide(value: &str) -> Vec<u16> { value.encode_utf16().chain(Some(0)).collect() }

unsafe fn owned_shell_string(value: PWSTR) -> Result<String, String> {
    // Shell 分配的字符串在复制之后立即释放；不把 COM 借用指针传到其他线程。
    let result = value.to_string().map_err(|_| "Windows Shell returned an invalid name.".to_string());
    CoTaskMemFree(Some(value.0.cast()));
    result
}

fn known_folder(id: &windows::core::GUID) -> Result<PathBuf, String> {
    unsafe {
        let value = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None).map_err(|_| "Unable to locate an application source.")?;
        Ok(PathBuf::from(owned_shell_string(value)?))
    }
}

struct LinkTarget { target: String, arguments: String, working_directory: String }

fn read_shortcut(path: &str) -> Result<LinkTarget, String> {
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(|_| "Unable to read application shortcut.")?;
        let persist: IPersistFile = link.cast().map_err(|_| "Unable to read application shortcut.")?;
        let file = wide(path);
        persist.Load(PCWSTR(file.as_ptr()), STGM_READ).map_err(|_| "Unable to read application shortcut.")?;
        // Windows ShellLink API 没有所需长度接口；使用完整 Windows 路径上限并拒绝截断。
        let mut target = vec![0u16; 32768];
        let mut arguments = vec![0u16; 32768];
        let mut working = vec![0u16; 32768];
        link.GetPath(&mut target, ptr::null_mut(), SLGP_RAWPATH.0 as u32).map_err(|_| "Unable to resolve application shortcut.")?;
        link.GetArguments(&mut arguments).map_err(|_| "Unable to read shortcut arguments.")?;
        link.GetWorkingDirectory(&mut working).map_err(|_| "Unable to read shortcut working directory.")?;
        Ok(LinkTarget { target: read_buffer(&target)?, arguments: read_buffer(&arguments)?, working_directory: read_buffer(&working)? })
    }
}

fn read_buffer(buffer: &[u16]) -> Result<String, String> {
    let size = buffer.iter().position(|value| *value == 0).ok_or("Windows Shell returned a truncated path.")?;
    if size == buffer.len() - 1 { return Err("Windows Shell returned a truncated path.".into()); }
    String::from_utf16(&buffer[..size]).map_err(|_| "Windows Shell returned an invalid path.".into())
}

fn executable(path: &str) -> bool { Path::new(path).extension().is_some_and(|value| value.eq_ignore_ascii_case("exe")) }

pub fn manual_app(path: String, title: String, aliases: Vec<String>) -> Result<Entry, String> {
    validate_locator(&path)?;
    if !Path::new(&path).is_file() { return Err("Application is no longer available.".into()); }
    let _apartment = Apartment::new()?;
    let shortcut = Path::new(&path).extension().is_some_and(|value| value.eq_ignore_ascii_case("lnk"));
    let key = if shortcut {
        let link = read_shortcut(&path)?;
        if !executable(&link.target) { return Err("Select an executable application shortcut.".into()); }
        launch_key(&link.target, &link.arguments, &link.working_directory)
    } else if executable(&path) { launch_key(&path, "", "") }
    else { return Err("Select an .exe or .lnk application.".into()); };
    Ok(Entry { entity_id: entity_id("app", &key), kind: "application".into(), title,
        subtitle: path.clone(), locator: path, launch_kind: if shortcut { "shortcut" } else { "path" }.into(), aliases, launch_key: key })
}

pub fn discover_apps(include_desktop: bool) -> Result<Vec<Entry>, String> {
    let _apartment = Apartment::new()?;
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    let mut roots = vec![(FOLDERID_Programs, false), (FOLDERID_CommonPrograms, false)];
    if include_desktop { roots.extend([(FOLDERID_Desktop, true), (FOLDERID_PublicDesktop, true)]); }
    let mut examined = 0usize;
    for (id, desktop) in roots {
        let root = known_folder(&id)?;
        if !root.exists() { continue; }
        scan_shortcuts(&root, desktop, 0, &mut examined, &mut entries, &mut seen)?;
    }
    discover_packaged(&mut entries, &mut seen)?;
    entries.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()).then(a.entity_id.cmp(&b.entity_id)));
    Ok(entries)
}

fn scan_shortcuts(root: &Path, desktop: bool, depth: usize, examined: &mut usize, entries: &mut Vec<Entry>, seen: &mut HashSet<String>) -> Result<(), String> {
    // 仅遍历已知来源，限制深度和目录项；不跟随符号链接/目录联接，避免扩展到整块磁盘。
    if depth > 12 { return Err("Application source exceeds the directory depth limit.".into()); }
    let files = fs::read_dir(root).map_err(|_| "Unable to enumerate an application source. Existing applications were retained.")?;
    for file in files {
        *examined += 1;
        if *examined > 10000 { return Err("Application discovery limit reached. Existing applications were retained.".into()); }
        let file = file.map_err(|_| "Unable to enumerate an application source.")?;
        let path = file.path();
        let metadata = fs::symlink_metadata(&path).map_err(|_| "Unable to read an application source.")?;
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 { continue; }
        if metadata.is_dir() { scan_shortcuts(&path, desktop, depth + 1, examined, entries, seen)?; continue; }
        if !path.extension().is_some_and(|value| value.eq_ignore_ascii_case("lnk")) { continue; }
        let Some(locator) = path.to_str() else { continue; };
        let Ok(link) = read_shortcut(locator) else { continue; };
        if link.target.is_empty() || (desktop && !executable(&link.target)) { continue; }
        // 普通文档快捷方式不进入应用目录；Shell 应用由 AppsFolder 来源保留原生标识。
        if !executable(&link.target) { continue; }
        let key = launch_key(&link.target, &link.arguments, &link.working_directory);
        if !seen.insert(key.clone()) { continue; }
        entries.push(Entry { entity_id: entity_id("app", &key), kind: "application".into(),
            title: path.file_stem().map(|value| value.to_string_lossy().into_owned()).unwrap_or_default(),
            subtitle: link.target, locator: locator.into(), launch_kind: "shortcut".into(), aliases: vec![], launch_key: key });
    }
    Ok(())
}

fn discover_packaged(entries: &mut Vec<Entry>, seen: &mut HashSet<String>) -> Result<(), String> {
    unsafe {
        let root: IShellItem = SHGetKnownFolderItem(&FOLDERID_AppsFolder, KF_FLAG_DEFAULT, None).map_err(|_| "Unable to locate the Windows application folder.")?;
        let items: IEnumShellItems = root.BindToHandler(None, &BHID_EnumItems).map_err(|error| format!("Unable to enumerate Windows applications ({:08X}).", error.code().0 as u32))?;
        for _ in 0..10000 {
            let mut children = [None];
            let mut fetched = 0;
            let result = items.Next(&mut children, Some(&mut fetched));
            if result.is_err() { return Err("Unable to enumerate packaged applications. Existing applications were retained.".into()); }
            if fetched == 0 { return Ok(()); }
            let Some(item) = children[0].take() else { continue; };
            let Ok(title) = item.GetDisplayName(SIGDN_NORMALDISPLAY).and_then(|value| {
                let text = value.to_string(); CoTaskMemFree(Some(value.0.cast())); Ok(text?)
            }) else { continue; };
            let Ok(identifier) = item.GetDisplayName(SIGDN_PARENTRELATIVEPARSING).and_then(|value| {
                let text = value.to_string(); CoTaskMemFree(Some(value.0.cast())); Ok(text?)
            }) else { continue; };
            if !identifier.contains('!') || identifier.contains('\0') || identifier.contains('\\') { continue; }
            let locator = format!("shell:AppsFolder\\{identifier}");
            if !seen.insert(locator.clone()) { continue; }
            entries.push(Entry { entity_id: entity_id("app", &locator), kind: "application".into(), title,
                subtitle: "Windows application".into(), locator: locator.clone(), launch_kind: "shell".into(), aliases: vec![], launch_key: locator });
        }
    }
    Err("Packaged application discovery limit reached.".into())
}

fn existing_path(entry: &Entry) -> Result<&str, String> {
    if entry.launch_kind == "shell" { return Err("This Windows application has no file path.".into()); }
    validate_locator(&entry.locator)?;
    if !Path::new(&entry.locator).exists() { return Err("File is no longer available.".into()); }
    Ok(&entry.locator)
}

pub fn open_entry(entry: &Entry) -> Result<(), String> {
    let _apartment = Apartment::new()?;
    if entry.launch_kind == "shell" {
        if !entry.locator.starts_with("shell:AppsFolder\\") || entry.locator.contains('\0') {
            return Err("Invalid Windows application identifier.".into());
        }
        let name = wide(&entry.locator);
        let _: IShellItem = unsafe { SHCreateItemFromParsingName(PCWSTR(name.as_ptr()), None) }.map_err(|_| "Application is no longer available.")?;
    } else { existing_path(entry)?; }
    let target = wide(&entry.locator);
    // 目标始终作为单独的 Shell 参数传入，不构造命令行，不转交任意参数字符串。
    let result = unsafe { ShellExecuteW(None, w!("open"), PCWSTR(target.as_ptr()), None, None, SW_SHOWNORMAL) };
    if result.0 as isize <= 32 { Err("Unable to open this item.".into()) } else { Ok(()) }
}

pub fn web_search_url(raw: &str) -> Result<tauri::Url, String> {
    if raw.trim().is_empty() || raw.chars().count() > 512 || raw.chars().any(char::is_control) {
        return Err("Invalid web search query.".into());
    }
    // 用户输入仅作为 q 参数编码，不能改变协议、域名或添加额外参数。
    let mut url = tauri::Url::parse("https://www.bing.com/search").map_err(|_| "Web search is unavailable.")?;
    url.query_pairs_mut().append_pair("q",raw.trim());
    Ok(url)
}

pub fn open_web_search(raw: &str) -> Result<(), String> {
    let url = web_search_url(raw)?;
    let _apartment = Apartment::new()?;
    let target = wide(url.as_str());
    // 固定 HTTPS 地址作为独立 Shell 目标交给默认浏览器，不拼接命令行。
    let result = unsafe { ShellExecuteW(None, w!("open"), PCWSTR(target.as_ptr()), None, None, SW_SHOWNORMAL) };
    if result.0 as isize <= 32 { Err("Unable to open the browser.".into()) } else { Ok(()) }
}

pub fn reveal_entry(entry: &Entry) -> Result<(), String> {
    let path = existing_path(entry)?;
    let _apartment = Apartment::new()?;
    let target = wide(path);
    unsafe {
        let mut pidl = ptr::null_mut();
        SHParseDisplayName(PCWSTR(target.as_ptr()), None, &mut pidl, 0, None).map_err(|_| "Unable to locate this item.")?;
        let result = SHOpenFolderAndSelectItems(pidl, None, 0);
        CoTaskMemFree(Some(pidl.cast()));
        result.map_err(|_| "Unable to reveal this item.".into())
    }
}

pub fn copy_path(entry: &Entry) -> Result<(), String> {
    let path = existing_path(entry)?;
    let text = wide(path);
    unsafe {
        // EmptyClipboard 后必须保留有效窗口所有者，否则 SetClipboardData 会失败。
        let owner = CreateWindowExW(WINDOW_EX_STYLE(0), w!("STATIC"), w!("SuperSearch Clipboard"), WINDOW_STYLE(0),
            0, 0, 0, 0, Some(HWND_MESSAGE), None, None, None).map_err(|_| "Clipboard is unavailable.")?;
        if OpenClipboard(Some(owner)).is_err() { let _ = DestroyWindow(owner); return Err("Clipboard is unavailable.".into()); }
        let result = (|| {
            let memory = GlobalAlloc(GMEM_MOVEABLE, text.len() * 2).map_err(|_| "Unable to allocate clipboard text.")?;
            let target = GlobalLock(memory);
            if target.is_null() { let _ = GlobalFree(Some(memory)); return Err("Unable to allocate clipboard text.".into()); }
            ptr::copy_nonoverlapping(text.as_ptr(), target.cast::<u16>(), text.len());
            let _ = GlobalUnlock(memory);
            if EmptyClipboard().is_err() { let _ = GlobalFree(Some(memory)); return Err("Clipboard is unavailable.".into()); }
            if SetClipboardData(13, Some(HANDLE(memory.0))).is_err() { let _ = GlobalFree(Some(memory)); return Err("Unable to copy this path.".into()); }
            // 成功后由系统接管 HGLOBAL，不能再次释放。
            Ok(())
        })();
        let _ = CloseClipboard();
        let _ = DestroyWindow(owner);
        result
    }
}

pub fn choose_file(kind: &str) -> Result<Option<String>, String> {
    if !matches!(kind, "app" | "everything") { return Err("Invalid file picker kind.".into()); }
    let _apartment = Apartment::new()?;
    unsafe {
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).map_err(|_| "Unable to open the file picker.")?;
        dialog.SetTitle(if kind == "app" { w!("选择应用程序") } else { w!("选择 Everything 客户端") }).map_err(|_| "Unable to configure the file picker.")?;
        dialog.SetOptions(FOS_FILEMUSTEXIST | FOS_PATHMUSTEXIST | FOS_FORCEFILESYSTEM | FOS_NOCHANGEDIR).map_err(|_| "Unable to configure the file picker.")?;
        let filters = [COMDLG_FILTERSPEC { pszName: if kind == "app" { w!("应用程序 (*.exe;*.lnk)") } else { w!("Everything 客户端 (Everything.exe)") },
            pszSpec: if kind == "app" { w!("*.exe;*.lnk") } else { w!("Everything.exe") } }];
        dialog.SetFileTypes(&filters).map_err(|_| "Unable to configure the file picker.")?;
        if let Err(error) = dialog.Show(None) {
            if error.code().0 as u32 == 0x800704c7 { return Ok(None); }
            return Err("Unable to open the file picker.".into());
        }
        let result = dialog.GetResult().map_err(|_| "Unable to read the selected file.")?;
        let path = owned_shell_string(result.GetDisplayName(SIGDN_FILESYSPATH).map_err(|_| "Select a local application file.")?)?;
        validate_locator(&path)?;
        if kind == "everything" && !Path::new(&path).file_name().is_some_and(|name| name.eq_ignore_ascii_case("Everything.exe")) {
            return Err("Select Everything.exe from the full Everything release.".into());
        }
        Ok(Some(path))
    }
}

fn launch_key(target: &str, arguments: &str, working_directory: &str) -> String {
    // 保留路径大小写，避免合并大小写敏感目录中的不同目标；长度前缀防止字段碰撞。
    format!("{}:{}{}:{}{}:{}", target.len(), target, arguments.len(), arguments, working_directory.len(), working_directory)
}

fn validate_locator(locator: &str) -> Result<(), String> {
    let bytes = locator.as_bytes();
    let absolute_drive = bytes.len() >= 3 && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':' && (bytes[2] == b'\\' || bytes[2] == b'/');
    let absolute_extended = locator.starts_with("\\\\?\\");
    let absolute_unc = locator.starts_with("\\\\") && !locator.starts_with("\\\\.\\");
    if locator.contains('\0') || locator.encode_utf16().count() > 32767
        || !(absolute_drive || absolute_extended || absolute_unc) {
        return Err("Invalid item path.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_search_encodes_one_query_on_fixed_bing_domain() {
        for raw in ["  中文 搜索 & + \"引用\" : 示例  ", "https://other.example/a?q=x&z=1#fragment", "javascript:alert('x')", "a + b & c"] {
            let url = web_search_url(raw).unwrap();
            assert_eq!(url.scheme(),"https");
            assert_eq!(url.host_str(),Some("www.bing.com"));
            assert_eq!(url.path(),"/search");
            assert_eq!(url.fragment(),None);
            assert_eq!(url.username(),"");
            assert_eq!(url.query_pairs().collect::<Vec<_>>(),vec![("q".into(),raw.trim().into())]);
        }
        let url = web_search_url("& + \" :").unwrap();
        assert!(url.as_str().contains("%26") && url.as_str().contains("%2B") && url.as_str().contains("%22") && url.as_str().contains("%3A"));
        assert!(web_search_url(&"中".repeat(512)).is_ok());
    }

    #[test]
    fn web_search_rejects_blank_controls_and_oversized_queries() {
        for raw in ["", " 　", "bad\0query", "line\nquery", "tab\tquery", "bad\u{7f}query"] {
            assert!(web_search_url(raw).is_err(),"{raw:?}");
        }
        assert!(web_search_url(&"中".repeat(513)).is_err());
    }

    #[test]
    fn launch_identity_preserves_arguments_and_directory() {
        let a = launch_key("C:\\App\\工具.exe", "--one", "C:\\Work");
        assert_ne!(a, launch_key("C:\\App\\工具.exe", "--two", "C:\\Work"));
        assert_ne!(a, launch_key("C:\\App\\工具.exe", "--one", "C:\\Other"));
        assert_ne!(a, launch_key("C:\\App\\工具.exe", "--one", "c:\\Work"));
    }

    #[test]
    fn locator_rejects_shell_input_and_embedded_nul() {
        for value in ["", "relative.exe", "https://example.com", "C:\\bad\0.exe", "shell:AppsFolder\\bad"] {
            assert!(validate_locator(value).is_err(), "{value:?}");
        }
        assert!(validate_locator("C:\\项目, 空格\\工具.exe").is_ok());
        assert!(validate_locator(&format!("C:\\{}\\工具.exe", "目录".repeat(300))).is_ok());
    }

    #[test]
    fn compact_identity_preserves_long_unicode_path_without_disclosure() {
        let path = format!("C:\\{}\\private.txt", "目录".repeat(300));
        let id = entity_id("file", &path);
        assert!(id.len() < 64);
        assert!(!id.contains("private"));
        assert_eq!(id, entity_id("file", &path));
        assert_ne!(id, entity_id("file", &(path + "x")));
    }
}

pub(crate) fn entity_id(kind: &str, locator: &str) -> String {
    // 固定 FNV-1a 128 位身份键，不把真实路径发送给前端；完整路径仍保留在后端描述符。
    let mut hash = 0x6c62272e07bb014262b821756295c58du128;
    for byte in locator.as_bytes() {
        hash ^= *byte as u128;
        hash = hash.wrapping_mul(0x0000000001000000000000000000013bu128);
    }
    format!("{kind}:{hash:032x}")
}
