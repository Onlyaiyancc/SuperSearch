use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    ffi::{c_char, c_int, c_void, CString, OsString},
    fs,
    io::{Read, Write},
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    time::Duration,
};
use windows::{
    core::{PCSTR, PCWSTR},
    Win32::{
        Foundation::{
            CloseHandle, FreeLibrary, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, HMODULE,
            INVALID_HANDLE_VALUE,
        },
        System::{
            Com::CoCreateGuid,
            LibraryLoader::{
                GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR,
                LOAD_LIBRARY_SEARCH_SYSTEM32,
            },
            Memory::{
                CreateFileMappingW, MapViewOfFile, OpenFileMappingW, UnmapViewOfFile,
                FILE_MAP_READ, FILE_MAP_WRITE, MEMORY_MAPPED_VIEW_ADDRESS, PAGE_READWRITE,
            },
        },
    },
};

const MAPPING_PREFIX: &str = "Local\\SuperSearch-Screen-OCR-";
const RESPONSE_PREFIX: &str = "Local\\SuperSearch-Screen-OCR-Response-";
const MAX_RESPONSE: usize = 512 * 1024;
const MAX_PIXELS: usize = 40_000_000;
const MAX_TEXT: usize = 64 * 1024;
const MAX_TSV: usize = 4 * 1024 * 1024;
static TASK_SERIAL: AtomicU64 = AtomicU64::new(0);

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    mapping: String,
    response_mapping: String,
    width: u32,
    height: u32,
    runtime_dir: PathBuf,
}

pub fn recognize(
    image: &image::RgbaImage,
    runtime_dir: &Path,
    task_root: &Path,
    cancel: &AtomicBool,
) -> Result<crate::qr_links::ExtractedLinks, String> {
    check_cancel(cancel)?;
    pixel_count(image.width(), image.height())?;
    let runtime_dir = runtime_dir
        .canonicalize()
        .map_err(|_| "Conversion screen OCR component unavailable")?;
    validate_runtime(&runtime_dir)?;
    let (mapping, _pixels) = SharedPixels::from_image(image, cancel)?;
    let (response_mapping, response) = SharedResponse::new()?;
    let metadata = serde_json::to_vec(&Request {
        mapping,
        response_mapping,
        width: image.width(),
        height: image.height(),
        runtime_dir,
    })
    .map_err(|_| "Conversion screen OCR metadata unavailable")?;
    if metadata.len() > MAX_TEXT {
        return Err("Conversion screen OCR metadata limit".into());
    }
    check_cancel(cancel)?;
    let task = TaskDirectory::new(task_root)?;
    let executable =
        std::env::current_exe().map_err(|_| "Conversion screen OCR executable unavailable")?;
    // 现有执行器约束句柄、内存、取消和超时；磁盘只接触映射元数据和固定状态标识。
    let output = crate::conversion_process::run(
        &executable,
        &[OsString::from("--screen-ocr-worker")],
        &task.0,
        Some(&metadata),
        cancel,
        Duration::from_secs(25),
    )?;
    check_cancel(cancel)?;
    if output.stdout != b"{}" {
        return Err("Conversion screen OCR invalid status".into());
    }
    response.read_links()
}

pub fn run_worker() -> Result<(), String> {
    let request = read_request(std::io::stdin().lock())?;
    let pixels = SharedPixels::open(
        &request.mapping,
        pixel_count(request.width, request.height)?,
    )?;
    let response = SharedResponse::open(&request.response_mapping)?;
    let input = unsafe {
        std::slice::from_raw_parts(
            pixels.view.Value.cast::<u8>(),
            pixel_count(request.width, request.height)?,
        )
    };
    let prepared = prepare_ocr_pixels(input, request.width, request.height)?;
    let engine = Tesseract::new(&request.runtime_dir)?;
    unsafe {
        // 稀疏文本布局适合屏幕中分散的网址；灰度截图只通过只读内存映射传给隔离进程。
        (engine.set_page_mode)(engine.handle, 11);
        (engine.set_image)(
            engine.handle,
            prepared.bytes.as_ptr(),
            prepared.width as c_int,
            prepared.height as c_int,
            1,
            prepared.width as c_int,
        );
    }
    let text = TessText {
        pointer: unsafe { (engine.get_tsv)(engine.handle, 0) },
        delete: engine.delete_text,
    };
    let tsv = unsafe { bounded_utf8(text.pointer, MAX_TSV)? };
    let content = tsv_screen_text(&tsv)?;
    let links = crate::qr_links::extract_screen_text(&content)?;
    let output =
        serde_json::to_vec(&links).map_err(|_| "Conversion screen OCR result unavailable")?;
    decode_links(&output)?;
    response.write_links(&output)?;
    // 截图、OCR 全文和含查询参数的网址均不写入磁盘或标准输出。
    std::io::stdout()
        .lock()
        .write_all(b"{}")
        .map_err(|_| "Conversion screen OCR result unavailable".into())
}

fn check_cancel(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Acquire) {
        Err("Conversion cancelled".into())
    } else {
        Ok(())
    }
}

fn pixel_count(width: u32, height: u32) -> Result<usize, String> {
    if width == 0 || height == 0 || width > c_int::MAX as u32 || height > c_int::MAX as u32 {
        return Err("Conversion screen OCR image size limit".into());
    }
    (width as usize)
        .checked_mul(height as usize)
        .filter(|count| *count <= MAX_PIXELS)
        .ok_or_else(|| "Conversion screen OCR image size limit".into())
}

fn ocr_dimensions(width: u32, height: u32) -> Result<(u32, u32), String> {
    pixel_count(width, height)?;
    // 优先放大小字号的字符细节；大屏降到两倍或原尺寸，处理后仍遵守原像素上限。
    for scale in [3, 2] {
        if let (Some(w), Some(h)) = (width.checked_mul(scale), height.checked_mul(scale)) {
            if pixel_count(w, h).is_ok() {
                return Ok((w, h));
            }
        }
    }
    Ok((width, height))
}

struct OcrPixels<'a> {
    width: u32,
    height: u32,
    bytes: Cow<'a, [u8]>,
}

fn prepare_ocr_pixels(bytes: &[u8], width: u32, height: u32) -> Result<OcrPixels<'_>, String> {
    if bytes.len() != pixel_count(width, height)? {
        return Err("Conversion screen OCR image size limit".into());
    }
    let (w, h) = ocr_dimensions(width, height)?;
    if (w, h) == (width, height) {
        return Ok(OcrPixels {
            width,
            height,
            bytes: Cow::Borrowed(bytes),
        });
    }
    let source = image::ImageBuffer::<image::Luma<u8>, _>::from_raw(width, height, bytes)
        .ok_or("Conversion screen OCR image size limit")?;
    let enlarged = image::imageops::resize(&source, w, h, image::imageops::FilterType::Lanczos3);
    Ok(OcrPixels {
        width: w,
        height: h,
        bytes: Cow::Owned(enlarged.into_raw()),
    })
}

fn valid_mapping_name(name: &str) -> bool {
    name.strip_prefix(MAPPING_PREFIX).is_some_and(|nonce| {
        nonce.len() == 32 && nonce.bytes().all(|value| value.is_ascii_hexdigit())
    })
}

fn valid_response_mapping_name(name: &str) -> bool {
    name.strip_prefix(RESPONSE_PREFIX).is_some_and(|nonce| {
        nonce.len() == 32 && nonce.bytes().all(|value| value.is_ascii_hexdigit())
    })
}

fn validate_runtime(runtime: &Path) -> Result<(), String> {
    if !runtime.is_absolute()
        || runtime.as_os_str().len() > 32760
        || runtime.as_os_str().encode_wide().any(|c| c == 0)
        || !runtime.join("libtesseract-5.dll").is_file()
        || !runtime.join("tessdata/eng.traineddata").is_file()
        || !runtime.join("tessdata/chi_sim.traineddata").is_file()
    {
        return Err("Conversion screen OCR component unavailable".into());
    }
    Ok(())
}

fn tessdata_path(runtime: &Path) -> Result<CString, String> {
    if !runtime.is_absolute() {
        return Err("Conversion screen OCR data unavailable".into());
    }
    let path = runtime.join("tessdata");
    let path = path
        .to_str()
        .ok_or("Conversion screen OCR data unavailable")?;
    // Rust 规范化路径带扩展前缀；此组件的 C 文件接口无法读取它，保留等价的普通绝对路径。
    let normalized = if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else {
        path.strip_prefix(r"\\?\").unwrap_or(path).to_string()
    };
    CString::new(normalized).map_err(|_| "Conversion screen OCR data unavailable".into())
}

fn read_request(input: impl Read) -> Result<Request, String> {
    let mut bytes = Vec::new();
    input
        .take((MAX_TEXT + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| "Conversion screen OCR metadata unavailable")?;
    if bytes.len() > MAX_TEXT {
        return Err("Conversion screen OCR metadata limit".into());
    }
    let request: Request =
        serde_json::from_slice(&bytes).map_err(|_| "Conversion screen OCR invalid metadata")?;
    if !valid_mapping_name(&request.mapping)
        || !valid_response_mapping_name(&request.response_mapping)
    {
        return Err("Conversion screen OCR invalid mapping".into());
    }
    pixel_count(request.width, request.height)?;
    validate_runtime(&request.runtime_dir)?;
    Ok(request)
}

fn decode_links(bytes: &[u8]) -> Result<crate::qr_links::ExtractedLinks, String> {
    if bytes.len() > MAX_RESPONSE {
        return Err("Conversion screen OCR result limit".into());
    }
    let links: crate::qr_links::ExtractedLinks =
        serde_json::from_slice(bytes).map_err(|_| "Conversion screen OCR invalid result")?;
    if links.items.len() > 16
        || links.items.iter().enumerate().any(|(index, item)| {
            item.id != index.to_string()
                || crate::qr_links::web_url(&item.url).is_none_or(|url| {
                    url.as_str() != item.url || url.host_str() != Some(item.domain.as_str())
                })
        })
    {
        return Err("Conversion screen OCR invalid result".into());
    }
    Ok(links)
}

struct SharedPixels {
    view: MEMORY_MAPPED_VIEW_ADDRESS,
    handle: HANDLE,
}
impl SharedPixels {
    fn from_image(image: &image::RgbaImage, cancel: &AtomicBool) -> Result<(String, Self), String> {
        let count = pixel_count(image.width(), image.height())?;
        check_cancel(cancel)?;
        let nonce =
            unsafe { CoCreateGuid() }.map_err(|_| "Conversion screen OCR mapping unavailable")?;
        let name = format!("{MAPPING_PREFIX}{:032x}", nonce.to_u128());
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let handle = unsafe {
            CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                None,
                PAGE_READWRITE,
                0,
                count as u32,
                PCWSTR(wide.as_ptr()),
            )
        }
        .map_err(|_| "Conversion screen OCR mapping unavailable")?;
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            unsafe {
                let _ = CloseHandle(handle);
            }
            return Err("Conversion screen OCR mapping unavailable".into());
        }
        let result = Self::map(handle, count, FILE_MAP_WRITE)?;
        // 直接写入页文件支持的映射，不创建截图文件或额外的灰度副本。
        let output =
            unsafe { std::slice::from_raw_parts_mut(result.view.Value.cast::<u8>(), count) };
        for (index, (target, pixel)) in output.iter_mut().zip(image.pixels()).enumerate() {
            if index % 65536 == 0 {
                check_cancel(cancel)?;
            }
            *target =
                ((299 * pixel[0] as u32 + 587 * pixel[1] as u32 + 114 * pixel[2] as u32 + 500)
                    / 1000) as u8;
        }
        Ok((name, result))
    }

    fn open(name: &str, count: usize) -> Result<Self, String> {
        if !valid_mapping_name(name) || count == 0 || count > MAX_PIXELS {
            return Err("Conversion screen OCR invalid mapping".into());
        }
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let handle = unsafe { OpenFileMappingW(FILE_MAP_READ.0, false, PCWSTR(wide.as_ptr())) }
            .map_err(|_| "Conversion screen OCR mapping unavailable")?;
        Self::map(handle, count, FILE_MAP_READ)
    }

    fn map(
        handle: HANDLE,
        count: usize,
        access: windows::Win32::System::Memory::FILE_MAP,
    ) -> Result<Self, String> {
        let view = unsafe { MapViewOfFile(handle, access, 0, 0, count) };
        if view.Value.is_null() {
            unsafe {
                let _ = CloseHandle(handle);
            }
            return Err("Conversion screen OCR mapping unavailable".into());
        }
        Ok(Self { view, handle })
    }
}
impl Drop for SharedPixels {
    fn drop(&mut self) {
        unsafe {
            let _ = UnmapViewOfFile(self.view);
            let _ = CloseHandle(self.handle);
        }
    }
}

struct SharedResponse {
    mapping: SharedPixels,
}
impl SharedResponse {
    fn new() -> Result<(String, Self), String> {
        let nonce =
            unsafe { CoCreateGuid() }.map_err(|_| "Conversion screen OCR mapping unavailable")?;
        let name = format!("{RESPONSE_PREFIX}{:032x}", nonce.to_u128());
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let handle = unsafe {
            CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                None,
                PAGE_READWRITE,
                0,
                (MAX_RESPONSE + 4) as u32,
                PCWSTR(wide.as_ptr()),
            )
        }
        .map_err(|_| "Conversion screen OCR mapping unavailable")?;
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            unsafe {
                let _ = CloseHandle(handle);
            }
            return Err("Conversion screen OCR mapping unavailable".into());
        }
        let mapping = SharedPixels::map(handle, MAX_RESPONSE + 4, FILE_MAP_READ | FILE_MAP_WRITE)?;
        // 新建页文件映射已清零；第一个对齐的 u32 是结果长度，其余字节仅保存网址 JSON。
        Ok((name, Self { mapping }))
    }

    fn open(name: &str) -> Result<Self, String> {
        if !valid_response_mapping_name(name) {
            return Err("Conversion screen OCR invalid mapping".into());
        }
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let handle = unsafe { OpenFileMappingW(FILE_MAP_WRITE.0, false, PCWSTR(wide.as_ptr())) }
            .map_err(|_| "Conversion screen OCR mapping unavailable")?;
        Ok(Self {
            mapping: SharedPixels::map(handle, MAX_RESPONSE + 4, FILE_MAP_WRITE)?,
        })
    }

    fn write_links(&self, bytes: &[u8]) -> Result<(), String> {
        if bytes.is_empty() || bytes.len() > MAX_RESPONSE {
            return Err("Conversion screen OCR result limit".into());
        }
        decode_links(bytes)?;
        unsafe {
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                self.mapping.view.Value.cast::<u8>().add(4),
                bytes.len(),
            );
            // 完整写入正文后才发布长度，父进程仅在工作进程退出后读取。
            (&*self.mapping.view.Value.cast::<AtomicU32>())
                .store(bytes.len() as u32, Ordering::Release);
        }
        Ok(())
    }

    fn read_links(&self) -> Result<crate::qr_links::ExtractedLinks, String> {
        let length = unsafe { &*self.mapping.view.Value.cast::<AtomicU32>() }
            .load(Ordering::Acquire) as usize;
        if length == 0 || length > MAX_RESPONSE {
            return Err("Conversion screen OCR invalid result".into());
        }
        let bytes = unsafe {
            std::slice::from_raw_parts(self.mapping.view.Value.cast::<u8>().add(4), length)
        };
        decode_links(bytes)
    }
}

struct TaskDirectory(PathBuf);
impl TaskDirectory {
    fn new(root: &Path) -> Result<Self, String> {
        if !root.is_absolute() || !root.is_dir() {
            return Err("Conversion screen OCR workspace unavailable".into());
        }
        for _ in 0..8 {
            let serial = TASK_SERIAL.fetch_add(1, Ordering::Relaxed);
            let path = root.join(format!("screen-ocr-{}-{serial}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => break,
            }
        }
        Err("Conversion screen OCR workspace unavailable".into())
    }
}
impl Drop for TaskDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

type Create = unsafe extern "C" fn() -> *mut c_void;
type Delete = unsafe extern "C" fn(*mut c_void);
type Init = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> c_int;
type SetVariable = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> c_int;
type SetPageMode = unsafe extern "C" fn(*mut c_void, c_int);
type SetImage = unsafe extern "C" fn(*mut c_void, *const u8, c_int, c_int, c_int, c_int);
type GetTsv = unsafe extern "C" fn(*mut c_void, c_int) -> *mut c_char;
type DeleteText = unsafe extern "C" fn(*const c_char);

struct Library(HMODULE);
impl Drop for Library {
    fn drop(&mut self) {
        unsafe {
            let _ = FreeLibrary(self.0);
        }
    }
}
struct Tesseract {
    _library: Library,
    handle: *mut c_void,
    delete: Delete,
    delete_text: DeleteText,
    set_page_mode: SetPageMode,
    set_image: SetImage,
    get_tsv: GetTsv,
}
impl Tesseract {
    fn new(runtime: &Path) -> Result<Self, String> {
        validate_runtime(runtime)?;
        let dll = runtime.join("libtesseract-5.dll");
        let wide: Vec<u16> = dll.as_os_str().encode_wide().chain(Some(0)).collect();
        // 固定绝对 DLL 路径，仅允许同一组件目录和系统目录解析依赖。
        let library = Library(
            unsafe {
                LoadLibraryExW(
                    PCWSTR(wide.as_ptr()),
                    None,
                    LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32,
                )
            }
            .map_err(|_| "Conversion screen OCR library unavailable")?,
        );
        macro_rules! symbol {
            ($name:literal, $kind:ty) => {{
                let address = unsafe { GetProcAddress(library.0, PCSTR($name.as_ptr())) }
                    .ok_or("Conversion screen OCR API unavailable")?;
                unsafe {
                    std::mem::transmute::<unsafe extern "system" fn() -> isize, $kind>(address)
                }
            }};
        }
        let create = symbol!(b"TessBaseAPICreate\0", Create);
        let init = symbol!(b"TessBaseAPIInit3\0", Init);
        let set_variable = symbol!(b"TessBaseAPISetVariable\0", SetVariable);
        let delete = symbol!(b"TessBaseAPIDelete\0", Delete);
        let delete_text = symbol!(b"TessDeleteText\0", DeleteText);
        let set_page_mode = symbol!(b"TessBaseAPISetPageSegMode\0", SetPageMode);
        let set_image = symbol!(b"TessBaseAPISetImage\0", SetImage);
        let get_tsv = symbol!(b"TessBaseAPIGetTsvText\0", GetTsv);
        let handle = unsafe { create() };
        if handle.is_null() {
            return Err("Conversion screen OCR engine unavailable".into());
        }
        let engine = Self {
            _library: library,
            handle,
            delete,
            delete_text,
            set_page_mode,
            set_image,
            get_tsv,
        };
        let data = tessdata_path(runtime)?;
        unsafe {
            // 丢弃引擎诊断，避免第三方调试输出包含屏幕片段或文件路径。
            if set_variable(handle, c"debug_file".as_ptr(), c"NUL".as_ptr()) == 0 {
                return Err("Conversion screen OCR diagnostic setup failed".into());
            }
            if init(handle, data.as_ptr(), c"eng+chi_sim".as_ptr()) != 0 {
                return Err("Conversion screen OCR language initialization failed".into());
            }
            if set_variable(handle, c"debug_file".as_ptr(), c"NUL".as_ptr()) == 0 {
                return Err("Conversion screen OCR diagnostic restore failed".into());
            }
        }
        Ok(engine)
    }
}
impl Drop for Tesseract {
    fn drop(&mut self) {
        unsafe {
            (self.delete)(self.handle);
        }
    }
}
struct TessText {
    pointer: *mut c_char,
    delete: DeleteText,
}
impl Drop for TessText {
    fn drop(&mut self) {
        if !self.pointer.is_null() {
            unsafe {
                (self.delete)(self.pointer);
            }
        }
    }
}

#[cfg(test)]
unsafe fn bounded_text(pointer: *const c_char) -> Result<String, String> {
    unsafe { bounded_utf8(pointer, MAX_TEXT) }
}

unsafe fn bounded_utf8(pointer: *const c_char, limit: usize) -> Result<String, String> {
    if pointer.is_null() {
        return Err("Conversion screen OCR text unavailable".into());
    }
    // Tesseract 保证返回以 NUL 结尾的分配；只探测允许的前缀，超限时不发布部分网址。
    for length in 0..=limit {
        if unsafe { *pointer.add(length) } == 0 {
            let bytes = unsafe { std::slice::from_raw_parts(pointer.cast::<u8>(), length) };
            return std::str::from_utf8(bytes)
                .map(str::to_owned)
                .map_err(|_| "Conversion screen OCR invalid text".into());
        }
    }
    Err("Conversion screen OCR text limit".into())
}

struct OcrWord<'a> {
    text: &'a str,
    left: u32,
    top: u32,
    width: u32,
    height: u32,
    confidence: f32,
}

fn starts_web_address(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    ["http://", "https://", "www."]
        .iter()
        .any(|prefix| lower.starts_with(prefix))
}

fn partial_url_prefix(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let host = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
        .unwrap_or(&lower);
    let host = host
        .strip_prefix("www.")
        .unwrap_or(host)
        .split(['/', ':', '?', '#'])
        .next()
        .unwrap_or("");
    // 已经是点分主机的完整地址不再接后面的词，避免把说明或另一个裸域名拼入 URL。
    !host.is_empty()
        && host.len() <= 63
        && host.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
}

fn url_piece(text: &str) -> bool {
    !text.is_empty()
        && text.chars().all(|c| {
            c.is_alphanumeric()
                || matches!(
                    c,
                    '-' | '.'
                        | '_'
                        | '~'
                        | ':'
                        | '/'
                        | '?'
                        | '#'
                        | '['
                        | ']'
                        | '@'
                        | '!'
                        | '$'
                        | '&'
                        | '\''
                        | '('
                        | ')'
                        | '*'
                        | '+'
                        | ','
                        | ';'
                        | '='
                        | '%'
                )
        })
}

fn complete_joined_url(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    if lower.starts_with("https://") || lower.starts_with("http://") {
        return crate::qr_links::web_url(text).is_some_and(|url| {
            url.host_str()
                .is_some_and(|host| host.contains('.') && !host.ends_with('.'))
        });
    }
    if text.contains("://") {
        return false;
    }
    let Some(url) = crate::qr_links::web_url(&format!("https://{text}")) else {
        return false;
    };
    crate::qr_links::extract_screen_text(text)
        .is_ok_and(|links| links.items.len() == 1 && links.items[0].url == url.as_str())
}

fn append_tsv_line(
    output: &mut String,
    words: &[OcrWord<'_>],
    line_height: u32,
) -> Result<(), String> {
    if words.is_empty() {
        return Ok(());
    }
    if !output.is_empty() {
        output.push('\n');
    }
    let mut index = 0;
    while index < words.len() {
        if index > 0 {
            output.push(' ');
        }
        let first = &words[index];
        let mut candidate = first.text.to_owned();
        let mut joined = None;
        if first.confidence >= 70.0 && partial_url_prefix(first.text) && url_piece(first.text) {
            for next in index + 1..words.len() {
                let previous = &words[next - 1];
                let word = &words[next];
                let right = previous
                    .left
                    .checked_add(previous.width)
                    .ok_or("Conversion screen OCR invalid geometry")?;
                let overlap = previous
                    .top
                    .saturating_add(previous.height)
                    .min(word.top.saturating_add(word.height))
                    .saturating_sub(previous.top.max(word.top));
                // 小于四分之一行高的间隙视作字符间距；真正的词间距和新地址起始必须保留。
                if word.confidence < 70.0
                    || starts_web_address(word.text)
                    || !url_piece(word.text)
                    || word.left < right
                    || word.left - right > line_height / 4
                    || overlap * 2 < previous.height.min(word.height)
                {
                    break;
                }
                if candidate.len() + word.text.len() > 16 * 1024 {
                    break;
                }
                candidate.push_str(word.text);
                if complete_joined_url(&candidate) {
                    joined = Some(next);
                    break;
                }
            }
        }
        if let Some(last) = joined {
            output.push_str(&candidate);
            index = last + 1;
        } else {
            output.push_str(first.text);
            index += 1;
        }
        if output.len() > MAX_TEXT {
            return Err("Conversion screen OCR text limit".into());
        }
    }
    Ok(())
}

fn tsv_screen_text(tsv: &str) -> Result<String, String> {
    if tsv.len() > MAX_TSV {
        return Err("Conversion screen OCR text limit".into());
    }
    let mut lines = tsv.lines().peekable();
    // C API 只返回数据行，命令行渲染器才额外添加标题；两种格式共用相同校验。
    if lines.peek() == Some(&"level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext") { lines.next(); }
    let mut output = String::new();
    let mut words = Vec::new();
    let mut current_line = None;
    let mut line_height = 0;
    let mut text_bytes = 0usize;
    for row in lines {
        let columns: Vec<_> = row.splitn(12, '\t').collect();
        if columns.len() != 12 {
            return Err("Conversion screen OCR invalid geometry".into());
        }
        if !matches!(columns[0], "1" | "2" | "3" | "4" | "5") {
            return Err("Conversion screen OCR invalid geometry".into());
        }
        if !matches!(columns[0], "4" | "5") {
            continue;
        }
        let number = |column: usize| {
            columns[column]
                .parse::<u32>()
                .map_err(|_| "Conversion screen OCR invalid geometry".to_string())
        };
        let key = [number(1)?, number(2)?, number(3)?, number(4)?];
        if columns[0] == "4" {
            append_tsv_line(&mut output, &words, line_height)?;
            words.clear();
            current_line = Some(key);
            line_height = number(9)?;
            if line_height == 0 || line_height > c_int::MAX as u32 {
                return Err("Conversion screen OCR invalid geometry".into());
            }
            continue;
        }
        if columns[11].is_empty() {
            continue;
        }
        if current_line != Some(key) || columns[11].chars().any(char::is_control) {
            return Err("Conversion screen OCR invalid geometry".into());
        }
        text_bytes = text_bytes
            .checked_add(columns[11].len() + 1)
            .filter(|length| *length <= MAX_TEXT)
            .ok_or("Conversion screen OCR text limit")?;
        let word = OcrWord {
            text: columns[11],
            left: number(6)?,
            top: number(7)?,
            width: number(8)?,
            height: number(9)?,
            confidence: columns[10]
                .parse::<f32>()
                .map_err(|_| "Conversion screen OCR invalid geometry")?,
        };
        if word.width == 0
            || word.height == 0
            || !word.confidence.is_finite()
            || word
                .left
                .checked_add(word.width)
                .is_none_or(|right| right > c_int::MAX as u32)
            || word
                .top
                .checked_add(word.height)
                .is_none_or(|bottom| bottom > c_int::MAX as u32)
        {
            return Err("Conversion screen OCR invalid geometry".into());
        }
        words.push(word);
    }
    append_tsv_line(&mut output, &words, line_height)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires the verified local OCR component; no screen capture or network"]
    fn real_small_text_keeps_j_and_i_as_distinct_hostname_characters() {
        let runtime = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("test-results/screen-links/native-image-runtime/components/ocr/1.0.1");
        let engine = Tesseract::new(&runtime).unwrap();
        for (fixture, expected) in [
            (
                include_bytes!("test-fixtures/ocr-tjch-12.png").as_slice(),
                "https://tjch.hotjob.cn/",
            ),
            (
                include_bytes!("test-fixtures/ocr-tich-14.png").as_slice(),
                "https://tich.hotjob.cn/",
            ),
        ] {
            // 固定合成图像复现字形混淆；回归不得在文本层把 i 猜成 j。
            let image = image::load_from_memory(fixture).unwrap().to_luma8();
            let prepared =
                prepare_ocr_pixels(image.as_raw(), image.width(), image.height()).unwrap();
            unsafe {
                (engine.set_page_mode)(engine.handle, 11);
                (engine.set_image)(
                    engine.handle,
                    prepared.bytes.as_ptr(),
                    prepared.width as c_int,
                    prepared.height as c_int,
                    1,
                    prepared.width as c_int,
                );
            }
            let text = TessText {
                pointer: unsafe { (engine.get_tsv)(engine.handle, 0) },
                delete: engine.delete_text,
            };
            let tsv = unsafe { bounded_utf8(text.pointer, MAX_TSV).unwrap() };
            let links =
                crate::qr_links::extract_screen_text(&tsv_screen_text(&tsv).unwrap()).unwrap();
            assert_eq!(
                links
                    .items
                    .iter()
                    .map(|item| item.url.as_str())
                    .collect::<Vec<_>>(),
                [expected]
            );
        }
    }

    #[test]
    fn ocr_resampling_enlarges_small_text_without_exceeding_pixel_budget() {
        assert_eq!(ocr_dimensions(1920, 1080).unwrap(), (5760, 3240));
        assert_eq!(ocr_dimensions(3840, 2160).unwrap(), (7680, 4320));
        assert_eq!(ocr_dimensions(7680, 4320).unwrap(), (7680, 4320));
        assert!(ocr_dimensions(0, 100).is_err());
        assert!(ocr_dimensions(u32::MAX, 2).is_err());
    }

    #[test]
    fn ocr_resampling_preserves_luma_and_rejects_truncated_buffers() {
        let pixels = [0, 255, 255, 0];
        let image = prepare_ocr_pixels(&pixels, 2, 2).unwrap();
        assert_eq!((image.width, image.height), (6, 6));
        assert_eq!(image.bytes.len(), 36);
        assert!(image.bytes[0] < 32);
        assert!(image.bytes[5] > 223);
        assert!(prepare_ocr_pixels(&pixels[..3], 2, 2).is_err());
    }

    fn word_tsv(rows: &[(u32, u32, u32, u32, f32, &str)]) -> String {
        let mut tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n".to_string();
        let mut current = 0;
        for &(line, left, width, top, confidence, text) in rows {
            if line != current {
                current = line;
                tsv.push_str(&format!(
                    "4\t1\t{line}\t1\t1\t0\t0\t{top}\t1000\t20\t-1\t\n"
                ));
            }
            tsv.push_str(&format!(
                "5\t1\t{line}\t1\t1\t1\t{left}\t{top}\t{width}\t20\t{confidence}\t{text}\n"
            ));
        }
        tsv
    }

    #[test]
    fn tsv_repairs_only_small_geometry_gaps_inside_url_fragments() {
        let tsv = word_tsv(&[
            (1, 32, 106, 26, 86.0, "https://exam"),
            (1, 143, 171, 26, 87.0, "ple.com/screen-one"),
            (2, 30, 89, 68, 87.0, "www.exam"),
            (2, 124, 163, 68, 87.0, "ple.org/screen-two"),
            (3, 31, 42, 110, 91.0, "exam"),
            (3, 78, 102, 110, 91.0, "ple.net/help"),
        ]);
        assert_eq!(
            tsv_screen_text(&tsv).unwrap(),
            "https://example.com/screen-one\nwww.example.org/screen-two\nexample.net/help"
        );
        assert_eq!(
            tsv_screen_text(tsv.split_once('\n').unwrap().1).unwrap(),
            tsv_screen_text(&tsv).unwrap()
        );
    }

    #[test]
    fn tsv_preserves_real_spaces_surrounding_words_and_adjacent_addresses() {
        let tsv = word_tsv(&[
            (1, 0, 30, 0, 95.0, "read"),
            (1, 32, 150, 0, 95.0, "https://one.example"),
            (1, 184, 150, 0, 95.0, "https://two.example"),
            (1, 336, 30, 0, 95.0, "now"),
            (2, 0, 42, 40, 95.0, "exam"),
            (2, 54, 100, 40, 95.0, "ple.net/help"),
            (3, 0, 80, 80, 95.0, "one.com"),
            (3, 82, 80, 80, 95.0, "two.org"),
        ]);
        assert_eq!(
            tsv_screen_text(&tsv).unwrap(),
            "read https://one.example https://two.example now\nexam ple.net/help\none.com two.org"
        );
    }

    #[test]
    fn tsv_never_joins_low_confidence_words_or_different_lines() {
        let tsv = word_tsv(&[
            (1, 0, 42, 0, 20.0, "exam"),
            (1, 47, 102, 0, 90.0, "ple.net/help"),
            (2, 0, 42, 40, 90.0, "exam"),
            (3, 47, 102, 40, 90.0, "ple.net/help"),
        ]);
        assert_eq!(
            tsv_screen_text(&tsv).unwrap(),
            "exam ple.net/help\nexam\nple.net/help"
        );
        assert!(tsv_screen_text(&"x".repeat(4 * 1024 * 1024 + 1)).is_err());
    }

    #[test]
    fn tsv_does_not_finish_a_hostname_at_an_ocr_split_dot() {
        let tsv = word_tsv(&[
            (1, 0, 90, 0, 95.0, "https://exa"),
            (1, 95, 45, 0, 95.0, "mple."),
            (1, 145, 80, 0, 95.0, "com/path"),
        ]);
        assert_eq!(tsv_screen_text(&tsv).unwrap(), "https://example.com/path");
    }

    #[test]
    fn tesseract_data_paths_remove_windows_extended_prefix_without_changing_unicode_or_unc() {
        for (runtime, expected) in [
            (r"\\?\C:\ocr", r"C:\ocr\tessdata"),
            (r"C:\识别组件", r"C:\识别组件\tessdata"),
            (r"\\?\UNC\host\share\ocr", r"\\host\share\ocr\tessdata"),
        ] {
            assert_eq!(
                tessdata_path(Path::new(runtime)).unwrap().to_str().unwrap(),
                expected
            );
        }
        assert!(tessdata_path(Path::new("relative-runtime")).is_err());
    }

    #[test]
    fn response_mapping_roundtrip_bounds_and_namespace() {
        let (name, parent) = SharedResponse::new().unwrap();
        assert!(valid_response_mapping_name(&name));
        assert!(!valid_mapping_name(&name));
        assert!(SharedResponse::open(
            "Local\\SuperSearch-Screen-OCR-0123456789abcdef0123456789abcdef"
        )
        .is_err());
        assert!(parent.read_links().is_err());
        let child = SharedResponse::open(&name).unwrap();
        let expected =
            crate::qr_links::extract_screen_text("https://example.com/?private=memory-only")
                .unwrap();
        child
            .write_links(&serde_json::to_vec(&expected).unwrap())
            .unwrap();
        assert_eq!(
            parent.read_links().unwrap().items[0].url,
            expected.items[0].url
        );
        assert!(child.write_links(&vec![b'x'; 512 * 1024 + 1]).is_err());
        unsafe {
            &*parent
                .mapping
                .view
                .Value
                .cast::<std::sync::atomic::AtomicU32>()
        }
        .store(u32::MAX, Ordering::Release);
        assert!(parent.read_links().is_err());
        drop(child);
        drop(parent);
        assert!(SharedResponse::open(&name).is_err());
    }

    #[test]
    fn rejects_empty_oversize_and_overflowing_images() {
        for (width, height) in [(0, 1), (1, 0), (40_000_001, 1), (u32::MAX, u32::MAX)] {
            assert!(pixel_count(width, height).is_err());
        }
        assert_eq!(pixel_count(8000, 5000).unwrap(), 40_000_000);
    }

    #[test]
    fn rejects_names_outside_the_private_mapping_namespace() {
        for name in [
            "",
            "Local\\Other-123",
            "Global\\SuperSearch-Screen-OCR-0123456789abcdef0123456789abcdef",
            "Local\\SuperSearch-Screen-OCR-../x",
            "Local\\SuperSearch-Screen-OCR-0123456789abcdef0123456789abcdef-extra",
        ] {
            assert!(!valid_mapping_name(name));
        }
        assert!(valid_mapping_name(
            "Local\\SuperSearch-Screen-OCR-0123456789abcdef0123456789abcdef"
        ));
    }

    #[test]
    fn metadata_is_bounded_and_rejects_extra_fields() {
        assert!(read_request(&vec![b' '; 65537][..]).is_err());
        assert!(read_request(br#"{"mapping":"Local\\SuperSearch-Screen-OCR-0123456789abcdef0123456789abcdef","width":1,"height":1,"runtime_dir":"C:\\ocr","extra":true}"#.as_slice()).is_err());
    }

    #[test]
    fn shared_pixels_are_readonly_for_the_worker_and_released_on_drop() {
        let image = image::RgbaImage::from_pixel(3, 2, image::Rgba([255, 255, 255, 255]));
        let (name, parent) = SharedPixels::from_image(&image, &AtomicBool::new(false)).unwrap();
        let child = SharedPixels::open(&name, 6).unwrap();
        assert_eq!(
            unsafe { std::slice::from_raw_parts(child.view.Value.cast::<u8>(), 6) },
            &[255; 6]
        );
        assert!(SharedPixels::open(&name, 65536).is_err());
        drop(child);
        drop(parent);
        assert!(SharedPixels::open(&name, 6).is_err());
    }

    #[test]
    fn cancelled_recognition_creates_no_workspace() {
        let directory =
            std::env::temp_dir().join(format!("supersearch-ocr-cancel-{}", std::process::id()));
        let result = recognize(
            &image::RgbaImage::new(1, 1),
            Path::new("C:\\missing-runtime"),
            &directory,
            &AtomicBool::new(true),
        );
        assert!(result.is_err());
        assert!(!directory.exists());
    }

    #[test]
    fn bounded_text_does_not_publish_partial_or_oversize_content() {
        assert_eq!(
            unsafe { bounded_text(b"https://example.com\0".as_ptr().cast()) }.unwrap(),
            "https://example.com"
        );
        let mut bytes = vec![b'x'; 65537];
        bytes.push(0);
        assert!(unsafe { bounded_text(bytes.as_ptr().cast()) }.is_err());
        assert!(unsafe { bounded_text(b"\xff\0".as_ptr().cast()) }.is_err());
        assert!(unsafe { bounded_text(std::ptr::null()) }.is_err());
    }
}
