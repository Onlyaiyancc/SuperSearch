use serde::Serialize;
use base64::{engine::general_purpose::STANDARD, Engine};
use std::{fs::{self, File, OpenOptions}, io::{Read, Seek, SeekFrom}, os::windows::{fs::{MetadataExt, OpenOptionsExt}, io::AsRawHandle}, path::PathBuf, sync::{Mutex, MutexGuard}};
use windows::{core::PCWSTR, Win32::{Foundation::HANDLE, Storage::FileSystem::{GetDriveTypeW, GetFileType, GetFinalPathNameByHandleW, FILE_FLAG_OPEN_REPARSE_POINT, FILE_NAME_NORMALIZED, FILE_TYPE_DISK}}};

#[path = "image_preview.rs"]
mod image_preview;

const TEXT_LIMIT: u64 = 1024 * 1024 * 1024;
const TEXT_WINDOW: u64 = 1024 * 1024;
const IMAGE_LIMIT: u64 = 50 * 1024 * 1024;
const PIXEL_LIMIT: u64 = 64_000_000;

fn begin_content_read() -> Result<MutexGuard<'static, ()>, String> {
    static CONTENT_READ: Mutex<()> = Mutex::new(());
    CONTENT_READ.try_lock().map_err(|_| "Preview busy".into())
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewResult {
    pub kind: String,
    pub text: String,
    pub mime: String,
    pub data: String,
    pub page_count: u32,
    pub page_index: u32,
    pub text_offset: u64,
    pub next_offset: Option<u64>,
    pub total_bytes: u64,
    pub truncated: bool,
    pub sections: Vec<crate::office_preview::OfficeSection>,
}

#[cfg(test)]
fn read_preview(path: &str) -> Result<PreviewResult, String> {
    read_preview_segment(path, 0)
}

pub fn read_preview_segment(path: &str, offset: u64) -> Result<PreviewResult, String> {
    // 关闭预览不能中断原生解码，立即拒绝重叠任务以限制后台资源占用。
    let _reading = begin_content_read()?;
    let local = validate_local_path(path)?;
    let extension = PathBuf::from(&local).extension().and_then(|value| value.to_str()).unwrap_or("").to_ascii_lowercase();
    let mime = match extension.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "tif" | "tiff" => Some("image/tiff"),
        "ico" => Some("image/x-icon"),
        "txt" | "md" | "markdown" | "mdown" | "log" | "csv" | "tsv" | "json" | "jsonc" | "json5" | "ndjson" | "jsonl" | "yaml" | "yml" | "toml" | "ini" | "cfg" | "conf" | "config" | "properties" | "env" | "gitignore" | "gitattributes" | "editorconfig" | "xml" | "html" | "htm" | "svg" | "xhtml" | "rs" | "ts" | "tsx" | "mts" | "cts" | "js" | "jsx" | "mjs" | "cjs" | "css" | "scss" | "sass" | "less" | "py" | "pyw" | "pyi" | "sql" | "c" | "cpp" | "cc" | "cxx" | "h" | "hpp" | "hh" | "hxx" | "cs" | "java" | "kt" | "kts" | "go" | "svelte" | "vue" | "astro" | "sh" | "bash" | "zsh" | "fish" | "bat" | "cmd" | "ps1" | "psm1" | "psd1" | "rb" | "php" | "lua" | "r" | "swift" | "dart" | "ex" | "exs" | "erl" | "hrl" | "clj" | "cljs" | "scala" | "pl" | "pm" | "cmake" | "gradle" | "dockerfile" | "makefile" | "tex" | "bib" | "rst" | "adoc" | "diff" | "patch" => None,
        "docx" | "xlsx" | "pptx" => {
            if offset != 0 { return Err("Preview invalid text offset".into()); }
            let file = open_bounded_file(&local, IMAGE_LIMIT)?;
            let total_bytes = file.metadata().map_err(|_| "Preview unavailable file")?.len();
            let office = crate::office_preview::read_office(file, &extension)?;
            return Ok(PreviewResult { kind: "office".into(), total_bytes, sections: office.sections, truncated: office.truncated, ..Default::default() });
        },
        _ if matches!(PathBuf::from(&local).file_name().and_then(|value| value.to_str()).unwrap_or("").to_ascii_lowercase().as_str(), ".env" | ".gitignore" | ".gitattributes" | ".editorconfig" | "dockerfile" | "makefile" | "cmakelists.txt" | "license" | "readme") => None,
        _ => return Err("Preview unsupported file type".into()),
    };
    let limit = if mime.is_some() { IMAGE_LIMIT } else { TEXT_LIMIT };
    let mut file = open_bounded_file(&local, limit)?;
    let total_bytes = file.metadata().map_err(|_| "Preview unavailable file")?.len();
    if mime.is_none() { return read_text_segment(file, &extension, offset, total_bytes); }
    if offset != 0 { return Err("Preview invalid text offset".into()); }
    // 元数据检查后仍限制实际读取量，避免文件并发增长突破内存上限。
    let mut bytes = Vec::new();
    (&mut file).take(limit + 1).read_to_end(&mut bytes).map_err(|_| "Preview unavailable file")?;
    if bytes.len() as u64 > limit { return Err("Preview size limit exceeded".into()); }
    if let Some(mime) = mime {
        let native_only = match mime {
            "image/tiff" => bytes.starts_with(b"II\x2a\0") || bytes.starts_with(b"MM\0\x2a"),
            "image/x-icon" => bytes.starts_with(&[0, 0, 1, 0]),
            _ => false,
        };
        if matches!(mime, "image/tiff" | "image/x-icon") {
            if !native_only { return Err("Preview invalid image".into()); }
            let png = image_preview::thumbnail(&bytes)?;
            return Ok(PreviewResult { kind: "image".into(), mime: "image/png".into(), data: STANDARD.encode(png), total_bytes, ..Default::default() });
        }
        let (width, height) = image_dimensions(&bytes, mime).ok_or("Preview invalid image")?;
        if width == 0 || height == 0 || u64::from(width) * u64::from(height) > PIXEL_LIMIT {
            return Err("Preview image dimensions exceed limit".into());
        }
        let (mime, bytes) = if width.max(height) > image_preview::LONG_EDGE || bytes.len() > image_preview::OUTPUT_LIMIT {
            ("image/png", image_preview::thumbnail(&bytes)?)
        } else { (mime, bytes) };
        Ok(PreviewResult { kind: "image".into(), mime: mime.into(), data: STANDARD.encode(bytes), total_bytes, ..Default::default() })
    } else {
        unreachable!("text was handled before image reads")
    }
}

pub(crate) fn open_bounded_file(path: &str, limit: u64) -> Result<File, String> {
    let local = validate_local_path(path)?;
    let file = open_local_file(&local)?;
    if file.metadata().map_err(|_| "Preview unavailable file")?.len() > limit {
        return Err("Preview size limit exceeded".into());
    }
    Ok(file)
}

fn validate_local_path(path: &str) -> Result<String, String> {
    let bytes = path.as_bytes();
    if bytes.len() < 4 || bytes.len() > 32_760 || !bytes[0].is_ascii_alphabetic() || bytes[1] != b':' || !matches!(bytes[2], b'\\' | b'/') {
        return Err("Preview local regular file required".into());
    }
    let normalized = path.replace('/', "\\");
    for part in normalized[3..].split('\\') {
        let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$" | "CONIN$" | "CONOUT$")
            || ["COM", "LPT"].iter().any(|prefix| stem.strip_prefix(prefix).is_some_and(|number| matches!(number, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³")));
        if part.is_empty() || part == "." || part == ".." || part.ends_with([' ', '.']) || reserved
            || part.chars().any(|value| value.is_control() || matches!(value, ':' | '*' | '?' | '<' | '>' | '|' | '"')) {
            return Err("Preview local regular file required".into());
        }
    }
    let root: Vec<u16> = normalized[..3].encode_utf16().chain(Some(0)).collect();
    // 仅固定盘、移动盘、光盘和内存盘允许读取；映射网络盘提前拒绝。
    if !matches!(unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) }, 2 | 3 | 5 | 6) {
        return Err("Preview local regular file required".into());
    }
    Ok(normalized)
}

fn open_local_file(path: &str) -> Result<File, String> {
    let mut current = PathBuf::from(&path[..3]);
    // 从卷根逐层检查，拒绝链接和联接点，避免沿父目录跳转到网络位置。
    for part in path[3..].split('\\') {
        current.push(part);
        let metadata = fs::symlink_metadata(&current).map_err(|_| "Preview unavailable file")?;
        if metadata.file_attributes() & 0x400 != 0 { return Err("Preview local regular file required".into()); }
    }
    let file = OpenOptions::new().read(true).share_mode(1).custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0).open(path).map_err(|_| "Preview unavailable file")?;
    let metadata = file.metadata().map_err(|_| "Preview unavailable file")?;
    if !metadata.is_file() || metadata.file_attributes() & 0x400 != 0 {
        return Err("Preview local regular file required".into());
    }
    let handle = HANDLE(file.as_raw_handle());
    if unsafe { GetFileType(handle) } != FILE_TYPE_DISK { return Err("Preview local regular file required".into()); }
    let mut buffer = vec![0u16; 32_768];
    let length = unsafe { GetFinalPathNameByHandleW(handle, &mut buffer, FILE_NAME_NORMALIZED) } as usize;
    if length == 0 || length >= buffer.len() { return Err("Preview unavailable file".into()); }
    let final_path = String::from_utf16(&buffer[..length]).map_err(|_| "Preview local regular file required")?;
    let final_path = final_path.strip_prefix(r"\\?\").ok_or("Preview local regular file required")?;
    validate_local_path(final_path)?;
    // 用实际打开句柄再次校验位置，阻止检查与打开之间被替换的链接。
    if !final_path.eq_ignore_ascii_case(path) { return Err("Preview local regular file required".into()); }
    Ok(file)
}

#[derive(Clone, Copy)]
enum TextEncoding { Utf8, Utf16(bool), Gb18030 }

fn read_text_segment(mut file: File, extension: &str, offset: u64, total_bytes: u64) -> Result<PreviewResult, String> {
    if offset > total_bytes { return Err("Preview invalid text offset".into()); }
    // 每次只取首段作编码判定；后续片段仍使用同一规则，不读取整个大文件。
    let mut head = Vec::new();
    (&mut file).take(TEXT_WINDOW).read_to_end(&mut head).map_err(|_| "Preview unavailable file")?;
    let (encoding, bom, ambiguous) = if head.starts_with(&[0xff, 0xfe]) {
        (TextEncoding::Utf16(true), 2, false)
    } else if head.starts_with(&[0xfe, 0xff]) {
        (TextEncoding::Utf16(false), 2, false)
    } else if head.starts_with(&[0xef, 0xbb, 0xbf]) {
        (TextEncoding::Utf8, 3, false)
    } else if let Ok(complete) = utf8_prefix(&head, head.len() as u64 == total_bytes) {
        (TextEncoding::Utf8, 0, head[..complete].is_ascii())
    } else {
        (TextEncoding::Gb18030, 0, false)
    };
    if offset > 0 && (offset < bom || matches!(encoding, TextEncoding::Utf16(_)) && offset % 2 != 0) {
        return Err("Preview invalid text offset".into());
    }
    let bytes = if offset == 0 { head } else {
        drop(head);
        file.seek(SeekFrom::Start(offset)).map_err(|_| "Preview unavailable file")?;
        let mut bytes = Vec::new();
        (&mut file).take(TEXT_WINDOW.min(total_bytes - offset)).read_to_end(&mut bytes).map_err(|_| "Preview unavailable file")?;
        bytes
    };
    let skip = if offset == 0 { bom as usize } else { 0 };
    if bytes.len() < skip { return Err("Preview unsupported text encoding".into()); }
    let eof = offset + bytes.len() as u64 >= total_bytes;
    let content = &bytes[skip..];
    let decoded = decode_text_prefix(content, encoding, eof);
    // 首段全为 ASCII 时，中文编码可能到后段才出现；仍坚持严格解码。
    let (text, consumed) = match decoded {
        Err(_) if ambiguous => decode_text_prefix(content, TextEncoding::Gb18030, eof)?,
        result => result?,
    };
    drop(bytes);
    let next = offset + skip as u64 + consumed as u64;
    if next == offset && next < total_bytes { return Err("Preview unsupported text encoding".into()); }
    let markdown = matches!(extension, "md" | "markdown" | "mdown");
    Ok(PreviewResult {
        kind: if markdown { "markdown" } else { "text" }.into(),
        mime: if markdown { "text/markdown" } else { "text/plain" }.into(),
        text, text_offset: offset, next_offset: (next < total_bytes).then_some(next),
        total_bytes, truncated: offset > 0 || next < total_bytes, ..Default::default()
    })
}

fn utf8_prefix(bytes: &[u8], eof: bool) -> Result<usize, String> {
    match std::str::from_utf8(bytes) {
        Ok(_) => Ok(bytes.len()),
        Err(error) if !eof && error.error_len().is_none() => Ok(error.valid_up_to()),
        Err(_) => Err("Preview unsupported text encoding".into()),
    }
}

fn decode_text_prefix(bytes: &[u8], encoding: TextEncoding, eof: bool) -> Result<(String, usize), String> {
    let (text, consumed) = match encoding {
        TextEncoding::Utf8 => {
            let consumed = utf8_prefix(bytes, eof)?;
            (std::str::from_utf8(&bytes[..consumed]).map_err(|_| "Preview unsupported text encoding")?.to_owned(), consumed)
        },
        TextEncoding::Utf16(little) => {
            if eof && bytes.len() % 2 != 0 { return Err("Preview unsupported text encoding".into()); }
            let mut units: Vec<u16> = bytes.chunks_exact(2).map(|pair| if little { u16::from_le_bytes([pair[0], pair[1]]) } else { u16::from_be_bytes([pair[0], pair[1]]) }).collect();
            if !eof && units.last().is_some_and(|unit| (0xd800..=0xdbff).contains(unit)) { units.pop(); }
            let consumed = units.len() * 2;
            (String::from_utf16(&units).map_err(|_| "Preview unsupported text encoding")?, consumed)
        },
        TextEncoding::Gb18030 => {
            let mut decoded = None;
            for trim in 0..=if eof { 0 } else { 3.min(bytes.len()) } {
                let length = bytes.len() - trim;
                if let Some(text) = encoding_rs::GB18030.decode_without_bom_handling_and_without_replacement(&bytes[..length]) {
                    decoded = Some((text.into_owned(), length));
                    break;
                }
            }
            decoded.ok_or("Preview unsupported text encoding")?
        },
    };
    if text.chars().any(|value| value.is_control() && !matches!(value, '\n' | '\r' | '\t')) {
        return Err("Preview unsupported text encoding".into());
    }
    Ok((text, consumed))
}

fn image_dimensions(bytes: &[u8], mime: &str) -> Option<(u32, u32)> {
    let le16 = |offset| Some(u16::from_le_bytes(bytes.get(offset..offset + 2)?.try_into().ok()?) as u32);
    let le32 = |offset| Some(u32::from_le_bytes(bytes.get(offset..offset + 4)?.try_into().ok()?));
    let be32 = |offset| Some(u32::from_be_bytes(bytes.get(offset..offset + 4)?.try_into().ok()?));
    match mime {
        "image/png" if bytes.len() >= 33 && bytes.starts_with(b"\x89PNG\r\n\x1a\n") && bytes.get(8..16)? == b"\0\0\0\rIHDR" => Some((be32(16)?, be32(20)?)),
        "image/gif" if bytes.len() >= 13 && (bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) => Some((le16(6)?, le16(8)?)),
        "image/bmp" if bytes.starts_with(b"BM") && bytes.len() >= 26 => {
            if le32(14)? == 12 { Some((le16(18)?, le16(20)?)) }
            else if le32(14)? >= 40 && bytes.len() >= 54 {
                let width = i32::from_le_bytes(bytes.get(18..22)?.try_into().ok()?);
                let height = i32::from_le_bytes(bytes.get(22..26)?.try_into().ok()?);
                (width > 0).then_some((width as u32, height.unsigned_abs()))
            } else { None }
        },
        "image/jpeg" if bytes.starts_with(b"\xff\xd8\xff") => jpeg_dimensions(bytes),
        "image/webp" if bytes.len() >= 30 && bytes.starts_with(b"RIFF") && bytes.get(8..12)? == b"WEBP" && u64::from(le32(4)?) + 8 == bytes.len() as u64 => {
            let payload = bytes.get(20..)?;
            let chunk_length = le32(16)? as usize;
            if chunk_length > payload.len() { return None; }
            match bytes.get(12..16)? {
                b"VP8X" if chunk_length == 10 => {
                    let width = u32::from_le_bytes([payload[4], payload[5], payload[6], 0]) + 1;
                    let height = u32::from_le_bytes([payload[7], payload[8], payload[9], 0]) + 1;
                    Some((width, height))
                },
                b"VP8L" if chunk_length >= 5 && payload[0] == 0x2f => {
                    let packed = u32::from_le_bytes(payload[1..5].try_into().ok()?);
                    Some(((packed & 0x3fff) + 1, ((packed >> 14) & 0x3fff) + 1))
                },
                b"VP8 " if chunk_length >= 10 && payload.get(3..6)? == b"\x9d\x01\x2a" && payload[0] & 1 == 0 => {
                    Some(((u16::from_le_bytes(payload[6..8].try_into().ok()?) & 0x3fff) as u32, (u16::from_le_bytes(payload[8..10].try_into().ok()?) & 0x3fff) as u32))
                },
                _ => None,
            }
        },
        _ => None,
    }
}

fn jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let mut offset = 2usize;
    while offset < bytes.len() {
        if *bytes.get(offset)? != 0xff { return None; }
        while bytes.get(offset) == Some(&0xff) { offset += 1; }
        let marker = *bytes.get(offset)?;
        offset += 1;
        if matches!(marker, 0xd9 | 0xda | 0x00) { return None; }
        if matches!(marker, 0x01 | 0xd0..=0xd8) { continue; }
        let length = u16::from_be_bytes(bytes.get(offset..offset + 2)?.try_into().ok()?) as usize;
        if length < 2 || offset.checked_add(length)? > bytes.len() { return None; }
        if matches!(marker, 0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf) {
            if length < 8 { return None; }
            let height = u16::from_be_bytes(bytes.get(offset + 3..offset + 5)?.try_into().ok()?) as u32;
            let width = u16::from_be_bytes(bytes.get(offset + 5..offset + 7)?.try_into().ok()?) as u32;
            return Some((width, height));
        }
        offset += length;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

    struct Fixture { path: PathBuf, _guard: MutexGuard<'static, ()> }
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            static TEST_SERIAL: Mutex<()> = Mutex::new(());
            let guard = TEST_SERIAL.lock().unwrap_or_else(|error| error.into_inner());
            let path = std::env::temp_dir().join(format!("supersearch-preview-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            fs::create_dir_all(&path).unwrap();
            Self { path, _guard: guard }
        }
        fn file(&self, name: &str, bytes: &[u8]) -> String {
            let path = self.path.join(name);
            fs::write(&path, bytes).unwrap();
            path.to_str().unwrap().to_owned()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) { let _ = fs::remove_dir_all(&self.path); }
    }

    #[test]
    fn reads_utf8_text_and_markdown_without_executing_content() {
        let fixture = Fixture::new();
        let text = read_preview(&fixture.file("note.txt", "你好\nhello".as_bytes())).unwrap();
        assert_eq!(text.kind, "text");
        assert_eq!(text.text, "你好\nhello");
        assert!(text.data.is_empty());
        let markdown = read_preview(&fixture.file("note.MD", b"# title\n<script>unsafe</script>")).unwrap();
        assert_eq!(markdown.kind, "markdown");
        assert!(markdown.text.contains("<script>"));
    }

    #[test]
    fn expanded_source_types_remain_plain_text() {
        let fixture = Fixture::new();
        for name in ["page.html", "image.svg", "run.ps1", "component.vue", "env.sh", "schema.jsonc"] {
            let preview = read_preview(&fixture.file(name, b"<script>alert('source')</script>")).unwrap();
            assert_eq!(preview.kind, "text");
            assert_eq!(preview.mime, "text/plain");
            assert!(preview.data.is_empty());
        }
    }

    #[test]
    fn expanded_text_reads_gb18030_and_large_first_window() {
        let fixture = Fixture::new();
        assert_eq!(read_preview(&fixture.file("chinese.txt", &[0xd6, 0xd0, 0xce, 0xc4])).unwrap().text, "中文");
        let preview = read_preview(&fixture.file("large.txt", &vec![b'a'; 1024 * 1024 + 10])).unwrap();
        assert_eq!(preview.text.len(), 1024 * 1024);
    }

    fn join_segments(path: &str) -> String {
        let mut text = String::new();
        let mut offset = 0;
        loop {
            let preview = read_preview_segment(path, offset).unwrap();
            assert_eq!(preview.text_offset, offset);
            text.push_str(&preview.text);
            match preview.next_offset {
                Some(next) => { assert!(next > offset && next - offset <= TEXT_WINDOW); offset = next; },
                None => break,
            }
        }
        text
    }

    #[test]
    fn text_segment_boundaries_preserve_utf8_utf16_and_gb18030_characters() {
        let fixture = Fixture::new();
        let utf8 = format!("{}😀中文尾", "a".repeat(TEXT_WINDOW as usize - 1));
        assert_eq!(join_segments(&fixture.file("utf8.txt", utf8.as_bytes())), utf8);
        for little in [true, false] {
            let text = format!("{}😀中文尾", "a".repeat(TEXT_WINDOW as usize / 2 - 2));
            let mut bytes = if little { vec![0xff, 0xfe] } else { vec![0xfe, 0xff] };
            for unit in text.encode_utf16() {
                bytes.extend_from_slice(&if little { unit.to_le_bytes() } else { unit.to_be_bytes() });
            }
            let path = fixture.file("utf16.txt", &bytes);
            assert_eq!(join_segments(&path), text);
            assert!(read_preview_segment(&path, 3).is_err());
        }
        // 首个非 ASCII 字符恰好跨片段，GB18030 包含双字节与四字节字符。
        for (character, encoded) in [("中", vec![0xd6, 0xd0]), ("😀", vec![0x94, 0x39, 0xfc, 0x36])] {
            for split in 1..encoded.len() {
                let prefix = "a".repeat(TEXT_WINDOW as usize - split);
                let mut bytes = prefix.as_bytes().to_vec();
                bytes.extend_from_slice(&encoded);
                bytes.extend_from_slice(b" end");
                assert_eq!(join_segments(&fixture.file("gb.txt", &bytes)), format!("{prefix}{character} end"));
            }
        }
    }

    #[test]
    fn text_file_and_raw_byte_offsets_have_independent_limits() {
        let fixture = Fixture::new();
        let path = fixture.file("limit.txt", b"abc");
        assert!(read_preview_segment(&path, 4).is_err());
        assert!(read_preview_segment(&path, u64::MAX).is_err());
        assert_eq!(read_preview_segment(&path, 3).unwrap().next_offset, None);
        let file = OpenOptions::new().write(true).open(&path).unwrap();
        file.set_len(TEXT_LIMIT + 1).unwrap();
        drop(file);
        assert!(read_preview(&path).unwrap_err().contains("size"));
        assert!(open_bounded_file(&path, TEXT_LIMIT).is_err());
    }

    #[test]
    fn concurrent_content_read_returns_busy_without_waiting() {
        let fixture = Fixture::new();
        let path = fixture.file("busy.txt", b"ready");
        let _guard = begin_content_read().unwrap();
        let started = std::time::Instant::now();
        assert_eq!(read_preview_segment(&path, 0).unwrap_err(), "Preview busy");
        assert!(started.elapsed() < std::time::Duration::from_millis(100));
        drop(_guard);
        assert_eq!(read_preview(&path).unwrap().text, "ready");
    }

    #[test]
    fn expanded_images_convert_tiff_ico_and_large_bmp_to_bounded_png() {
        let fixture = Fixture::new();
        let mut tiff = b"II\x2a\0\x08\0\0\0".to_vec();
        tiff.extend_from_slice(&10u16.to_le_bytes());
        for (tag, kind, count, value) in [(256u16, 4u16, 1u32, 1u32), (257, 4, 1, 1), (258, 3, 3, 134), (259, 3, 1, 1), (262, 3, 1, 2), (273, 4, 1, 140), (277, 3, 1, 3), (278, 4, 1, 1), (279, 4, 1, 3), (284, 3, 1, 1)] {
            tiff.extend_from_slice(&tag.to_le_bytes());
            tiff.extend_from_slice(&kind.to_le_bytes());
            tiff.extend_from_slice(&count.to_le_bytes());
            tiff.extend_from_slice(&value.to_le_bytes());
        }
        tiff.extend_from_slice(&[0, 0, 0, 0, 8, 0, 8, 0, 8, 0, 255, 0, 0]);
        for (name, bytes) in [("pixel.tiff", tiff.as_slice()), ("icon.ico", include_bytes!("../icons/icon.ico").as_slice())] {
            let preview = read_preview(&fixture.file(name, bytes)).unwrap();
            assert_eq!(preview.mime, "image/png");
            let png = STANDARD.decode(preview.data).unwrap();
            assert!(image_dimensions(&png, "image/png").is_some());
        }
        tiff[18..22].copy_from_slice(&100_000u32.to_le_bytes());
        tiff[30..34].copy_from_slice(&100_000u32.to_le_bytes());
        assert!(read_preview(&fixture.file("bomb.tiff", &tiff)).is_err());
        let mut bmp = vec![0; 54 + 3000 * 4];
        bmp[..2].copy_from_slice(b"BM");
        let length = bmp.len() as u32;
        bmp[2..6].copy_from_slice(&length.to_le_bytes());
        bmp[10..14].copy_from_slice(&54u32.to_le_bytes());
        bmp[14..18].copy_from_slice(&40u32.to_le_bytes());
        bmp[18..22].copy_from_slice(&3000u32.to_le_bytes());
        bmp[22..26].copy_from_slice(&1u32.to_le_bytes());
        bmp[26..28].copy_from_slice(&1u16.to_le_bytes());
        bmp[28..30].copy_from_slice(&32u16.to_le_bytes());
        let preview = read_preview(&fixture.file("wide.bmp", &bmp)).unwrap();
        assert_eq!(preview.mime, "image/png");
        let png = STANDARD.decode(preview.data).unwrap();
        assert_eq!(image_dimensions(&png, "image/png"), Some((2048, 1)));
        assert!(png.len() <= 16 * 1024 * 1024);
        // 超过旧版 10 MiB 上限的真实位图仍可缩略，不向前端发送原始大图。
        bmp.resize(54 + 3000 * 1000 * 4, 0);
        let length = bmp.len() as u32;
        bmp[2..6].copy_from_slice(&length.to_le_bytes());
        bmp[22..26].copy_from_slice(&1000u32.to_le_bytes());
        let preview = read_preview(&fixture.file("large.bmp", &bmp)).unwrap();
        assert_eq!(preview.total_bytes, bmp.len() as u64);
        assert_eq!(preview.mime, "image/png");
        let png = STANDARD.decode(preview.data).unwrap();
        assert_eq!(image_dimensions(&png, "image/png"), Some((2048, 682)));
        assert!(png.len() <= 16 * 1024 * 1024);
    }

    #[test]
    fn decodes_bom_text_and_rejects_unknown_or_binary_encodings() {
        let fixture = Fixture::new();
        for bytes in [vec![0xef, 0xbb, 0xbf, b'A'], vec![0xff, 0xfe, b'A', 0], vec![0xfe, 0xff, 0, b'A']] {
            assert_eq!(read_preview(&fixture.file("note.txt", &bytes)).unwrap().text, "A");
        }
        for bytes in [vec![0xff], vec![b'A', 0, b'B'], vec![0xff, 0xfe, 0], vec![0xff, 0xfe, 0, 0xd8], vec![b'A', 0x1b, b'B']] {
            assert!(read_preview(&fixture.file("binary.txt", &bytes)).is_err());
        }
    }

    #[test]
    fn refuses_oversized_files_and_unsupported_types() {
        let fixture = Fixture::new();
        assert_eq!(read_preview(&fixture.file("limit.txt", &vec![b'a'; 1024 * 1024])).unwrap().text.len(), 1024 * 1024);
        let path = fixture.file("huge.png", b"image");
        let file = OpenOptions::new().write(true).open(&path).unwrap();
        file.set_len(IMAGE_LIMIT).unwrap();
        drop(file);
        assert!(open_bounded_file(&path, IMAGE_LIMIT).is_ok());
        let file = OpenOptions::new().write(true).open(&path).unwrap();
        file.set_len(IMAGE_LIMIT + 1).unwrap();
        drop(file);
        assert!(read_preview(&path).unwrap_err().contains("size"));
        for name in ["program.exe", "note.pdf", "legacy.doc", "legacy.xls", "legacy.ppt", "macro.docm"] {
            assert!(read_preview(&fixture.file(name, b"hello")).is_err());
        }
        assert!(read_preview(fixture.path.to_str().unwrap()).is_err());
        assert!(read_preview(fixture.path.join("missing.txt").to_str().unwrap()).is_err());
    }

    #[test]
    fn checks_image_signatures_and_limits_decoded_dimensions() {
        let fixture = Fixture::new();
        let png = include_bytes!("../../public/supersearch-icon-source.png");
        let preview = read_preview(&fixture.file("image.png", png)).unwrap();
        assert_eq!(preview.kind, "image");
        assert_eq!(preview.mime, "image/png");
        assert!(preview.data.starts_with("iVBORw0KGgo"));
        assert!(preview.text.is_empty());
        assert!(read_preview(&fixture.file("fake.png", b"not png")).is_err());
        assert!(read_preview(&fixture.file("wrong.jpg", png)).is_err());
        let mut huge = png.to_vec();
        huge[16..20].copy_from_slice(&100_000u32.to_be_bytes());
        huge[20..24].copy_from_slice(&100_000u32.to_be_bytes());
        assert!(read_preview(&fixture.file("bomb.png", &huge)).is_err());
    }

    #[test]
    fn accepts_small_gif_and_bmp_with_bounded_dimensions() {
        let fixture = Fixture::new();
        let gif = b"GIF89a\x01\x00\x01\x00\x80\x00\x00\xff\xff\xff\x00\x00\x00\x21\xf9\x04\x01\x00\x00\x00\x00\x2c\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02\x44\x01\x00\x3b";
        assert_eq!(read_preview(&fixture.file("pixel.gif", gif)).unwrap().mime, "image/gif");
        let mut bmp = vec![0; 58];
        bmp[..2].copy_from_slice(b"BM");
        bmp[2..6].copy_from_slice(&58u32.to_le_bytes());
        bmp[10..14].copy_from_slice(&54u32.to_le_bytes());
        bmp[14..18].copy_from_slice(&40u32.to_le_bytes());
        bmp[18..22].copy_from_slice(&1u32.to_le_bytes());
        bmp[22..26].copy_from_slice(&1u32.to_le_bytes());
        bmp[26..28].copy_from_slice(&1u16.to_le_bytes());
        bmp[28..30].copy_from_slice(&24u16.to_le_bytes());
        assert_eq!(read_preview(&fixture.file("pixel.bmp", &bmp)).unwrap().mime, "image/bmp");
        bmp[22..26].copy_from_slice(&i32::MIN.to_le_bytes());
        assert!(read_preview(&fixture.file("bomb.bmp", &bmp)).is_err());
    }

    #[test]
    fn accepts_jpeg_and_webp_and_rejects_large_encoded_dimensions() {
        let fixture = Fixture::new();
        let mut webp = STANDARD.decode("UklGRhoAAABXRUJQVlA4TA4AAAAvAAAAAAcQEf0PRET/Aw==").unwrap();
        assert_eq!(read_preview(&fixture.file("pixel.webp", &webp)).unwrap().mime, "image/webp");
        webp[21..25].copy_from_slice(&0x0fff_ffffu32.to_le_bytes());
        assert!(read_preview(&fixture.file("bomb.webp", &webp)).is_err());
        let mut jpeg = STANDARD.decode("/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAAgGBgcGBQgHBwcJCQgKDBQNDAsLDBkSEw8UHRofHh0aHBwgJC4nICIsIxwcKDcpLDAxNDQ0Hyc5PTgyPC4zNDL/2wBDAQgJCQwLDBgNDRgyIRwhMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjL/wAARCAABAAEDASIAAhEBAxEB/8QAHwAAAQUBAQEBAQEAAAAAAAAAAAECAwQFBgcICQoL/8QAtRAAAgEDAwIEAwUFBAQAAAF9AQIDAAQRBRIhMUEGE1FhByJxFDKBkaEII0KxwRVS0fAkM2JyggkKFhcYGRolJicoKSo0NTY3ODk6Q0RFRkdISUpTVFVWV1hZWmNkZWZnaGlqc3R1dnd4eXqDhIWGh4iJipKTlJWWl5iZmqKjpKWmp6ipqrKztLW2t7i5usLDxMXGx8jJytLT1NXW19jZ2uHi4+Tl5ufo6erx8vP09fb3+Pn6/8QAHwEAAwEBAQEBAQEBAQAAAAAAAAECAwQFBgcICQoL/8QAtREAAgECBAQDBAcFBAQAAQJ3AAECAxEEBSExBhJBUQdhcRMiMoEIFEKRobHBCSMzUvAVYnLRChYkNOEl8RcYGRomJygpKjU2Nzg5OkNERUZHSElKU1RVVldYWVpjZGVmZ2hpanN0dXZ3eHl6goOEhYaHiImKkpOUlZaXmJmaoqOkpaanqKmqsrO0tba3uLm6wsPExcbHyMnK0tPU1dbX2Nna4uPk5ebn6Onq8vP09fb3+Pn6/9oADAMBAAIRAxEAPwD5/ooooA//2Q==").unwrap();
        assert_eq!(read_preview(&fixture.file("pixel.jpeg", &jpeg)).unwrap().mime, "image/jpeg");
        let frame = jpeg.windows(2).position(|pair| pair == [0xff, 0xc0]).unwrap();
        jpeg[frame + 5..frame + 9].copy_from_slice(&[0xff; 4]);
        assert!(read_preview(&fixture.file("bomb.jpg", &jpeg)).is_err());
        assert!(read_preview(&fixture.file("broken.jpg", b"\xff\xd8\xff\xe0\xff\xff")).is_err());
    }

    #[test]
    fn rejects_network_device_relative_and_alternate_stream_paths() {
        let _fixture = Fixture::new();
        for path in [r"\\server\share\note.txt", r"\\?\C:\note.txt", r"\\.\pipe\note.txt", "https://example.com/a.txt", "note.txt", r"C:note.txt", r"C:\note.txt:stream", r"C:\CON.txt", r"C:\dir\..\note.txt"] {
            assert!(read_preview(path).is_err(), "accepted {path}");
        }
    }

    #[test]
    fn rejects_directory_junctions_before_reading_the_target() {
        use std::{os::windows::process::CommandExt, process::Command};
        let fixture = Fixture::new();
        let target = fixture.path.join("target");
        let junction = fixture.path.join("redirect");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("note.txt"), b"local target").unwrap();
        let created = Command::new("cmd.exe").args(["/D", "/C", "mklink", "/J"]).arg(&junction).arg(&target).creation_flags(0x08000000).output().unwrap();
        assert!(created.status.success(), "junction fixture creation failed");
        let result = read_preview(junction.join("note.txt").to_str().unwrap());
        fs::remove_dir(&junction).unwrap();
        assert_eq!(result.unwrap_err(), "Preview local regular file required");
    }
}
