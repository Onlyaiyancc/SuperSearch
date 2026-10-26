use base64::{engine::general_purpose::STANDARD, Engine};
use std::{
    collections::VecDeque,
    fs::File,
    io::Read,
    os::windows::io::AsRawHandle,
    sync::{atomic::{AtomicU64, Ordering}, Mutex},
};
use windows::{
    Data::Pdf::{PdfDocument, PdfPageRenderOptions},
    Graphics::Imaging::BitmapEncoder,
    Storage::Streams::{DataReader, DataWriter, InMemoryRandomAccessStream},
    Win32::{
        Foundation::HANDLE,
        Storage::FileSystem::{GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION},
        System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED},
    },
};

const INPUT_LIMIT: u64 = 100 * 1024 * 1024;
const PNG_LIMIT: u64 = 16 * 1024 * 1024;
const CANCELLED: &str = "Preview PDF cancelled";
const UNAVAILABLE: &str = "Preview PDF unavailable";

#[derive(Debug)]
pub struct PdfPageResult {
    pub data: String,
    pub page_count: u32,
    pub page_index: u32,
}

#[derive(Default)]
pub struct PdfState {
    session: Mutex<Session>,
    epoch: AtomicU64,
    cache: Mutex<Option<CachedDocument>>,
    #[cfg(test)]
    after_load: Mutex<Option<RenderGate>>,
}

#[cfg(test)]
struct RenderGate {
    loaded: std::sync::mpsc::SyncSender<()>,
    resume: std::sync::mpsc::Receiver<()>,
}

#[derive(Default)]
struct Session {
    active: Option<(String, u64)>,
    cancelled: VecDeque<String>,
}

impl Session {
    fn remember_cancelled(&mut self, preview_id: &str) {
        if !self.cancelled.iter().any(|id| id == preview_id) {
            if self.cancelled.len() == 64 { self.cancelled.pop_front(); }
            self.cancelled.push_back(preview_id.into());
        }
    }
}

struct CachedDocument {
    epoch: u64,
    identity: [u32; 3],
    document: PdfDocument,
    // 保留有界输入流，确保 PDF 引擎后续翻页仍能访问原始字节。
    _source: InMemoryRandomAccessStream,
    // 缓存期间保留原句柄，避免删除后文件索引被新文件复用。
    _file: File,
}

impl PdfState {
    pub fn begin(&self, preview_id: &str) {
        let mut session = self.session.lock().unwrap_or_else(|error| error.into_inner());
        // 关闭消息可能早于阻塞任务启动，保留有界墓碑避免复活已关闭预览。
        if session.cancelled.iter().any(|id| id == preview_id) { return; }
        if let Some((old_id, _)) = session.active.take() {
            if old_id != preview_id { session.remember_cancelled(&old_id); }
        }
        let epoch = self.epoch.fetch_add(1, Ordering::SeqCst) + 1;
        session.active = Some((preview_id.into(), epoch));
        self.discard_idle_cache();
    }

    pub fn cancel(&self, preview_id: &str) {
        let mut session = self.session.lock().unwrap_or_else(|error| error.into_inner());
        session.remember_cancelled(preview_id);
        if session.active.as_ref().is_some_and(|(id, _)| id == preview_id) {
            self.epoch.fetch_add(1, Ordering::SeqCst);
            session.active = None;
            self.discard_idle_cache();
        }
    }

    pub fn clear(&self) {
        let mut session = self.session.lock().unwrap_or_else(|error| error.into_inner());
        self.epoch.fetch_add(1, Ordering::SeqCst);
        if let Some((old_id, _)) = session.active.take() { session.remember_cancelled(&old_id); }
        self.discard_idle_cache();
    }

    fn discard_idle_cache(&self) {
        // UI 关闭不等待原生渲染；忙碌任务在退出前根据代次清理缓存。
        if let Ok(mut cache) = self.cache.try_lock() { *cache = None; }
    }

    fn ensure_current(&self, epoch: u64) -> Result<(), String> {
        if self.epoch.load(Ordering::SeqCst) == epoch { Ok(()) } else { Err(CANCELLED.into()) }
    }

    pub fn render(&self, preview_id: &str, file: File, page: u32) -> Result<PdfPageResult, String> {
        let epoch = self.session.lock().map_err(|_| UNAVAILABLE)?.active.as_ref()
            .filter(|(id, _)| id == preview_id).map(|(_, epoch)| *epoch).ok_or(CANCELLED)?;
        // 单一渲染锁同时限制输入流、解码器及 PNG 缓冲的并发内存占用。
        let mut cache = self.cache.try_lock().map_err(|_| "Preview PDF busy")?;
        let result = (|| {
            self.ensure_current(epoch)?;
            let _apartment = Apartment::new()?;
            let identity = file_identity(&file)?;
            if cache.as_ref().is_some_and(|entry| entry.epoch == epoch && entry.identity != identity) {
                return Err("Preview PDF session mismatch".into());
            }
            if cache.as_ref().is_none_or(|entry| entry.epoch != epoch) {
                *cache = None;
                let loaded = load_document(file, epoch, || self.ensure_current(epoch))?;
                self.ensure_current(epoch)?;
                *cache = Some(loaded);
            }
            #[cfg(test)]
            {
                // 测试通过独立通道确认持锁完成原生载入，不争抢真实渲染锁。
                let gate = self.after_load.lock().unwrap().take();
                if let Some(gate) = gate {
                    gate.loaded.send(()).expect("PDF test load observer disconnected");
                    gate.resume.recv_timeout(std::time::Duration::from_secs(5))
                        .expect("PDF test render gate timed out");
                }
            }
            let document = &cache.as_ref().ok_or(UNAVAILABLE)?.document;
            let page_count = native(document.PageCount())?;
            if page >= page_count { return Err("Preview PDF page out of range".into()); }
            self.ensure_current(epoch)?;
            render_page(document, page, page_count)
        })();
        // 完成检查与释放渲染锁使用同一短会话锁，防止取消恰好落在二者之间。
        let session = self.session.lock().unwrap_or_else(|error| error.into_inner());
        let result = if let Err(error) = self.ensure_current(epoch) {
            *cache = None;
            Err(error)
        } else { result };
        drop(cache);
        drop(session);
        result
    }
}

struct Apartment;

impl Apartment {
    fn new() -> Result<Self, String> {
        // 仅在调用方提供的阻塞线程上同步等待 WinRT，不占用 UI 线程。
        native(unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok())?;
        Ok(Self)
    }
}

impl Drop for Apartment {
    fn drop(&mut self) { unsafe { CoUninitialize() }; }
}

fn native<T>(result: windows::core::Result<T>) -> Result<T, String> {
    result.map_err(|_| UNAVAILABLE.into())
}

fn file_identity(file: &File) -> Result<[u32; 3], String> {
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    // 使用已校验句柄确认缓存来源，不重新解析或打开前端路径。
    native(unsafe { GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut information) })?;
    Ok([information.dwVolumeSerialNumber, information.nFileIndexHigh, information.nFileIndexLow])
}

fn load_document(mut file: File, epoch: u64, check: impl Fn() -> Result<(), String>) -> Result<CachedDocument, String> {
    if file.metadata().map_err(|_| "Preview unavailable file")?.len() > INPUT_LIMIT {
        return Err("Preview size limit exceeded".into());
    }
    let identity = file_identity(&file)?;
    let stream = native(InMemoryRandomAccessStream::new())?;
    let writer = native(DataWriter::CreateDataWriter(&native(stream.GetOutputStreamAt(0))?))?;
    let mut chunk = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        check()?;
        let count = file.read(&mut chunk).map_err(|_| "Preview unavailable file")?;
        if count == 0 { break; }
        total += count as u64;
        if total > INPUT_LIMIT { return Err("Preview size limit exceeded".into()); }
        native(writer.WriteBytes(&chunk[..count]))?;
        let stored = native(native(writer.StoreAsync())?.get())?;
        if stored as usize != count { return Err(UNAVAILABLE.into()); }
    }
    native(native(writer.FlushAsync())?.get())?;
    native(writer.DetachStream())?;
    drop(writer);
    native(stream.Seek(0))?;
    check()?;
    let document = PdfDocument::LoadFromStreamAsync(&stream).and_then(|operation| operation.get())
        .map_err(|_| "Preview invalid or password protected PDF")?;
    check()?;
    Ok(CachedDocument { epoch, identity, document, _source: stream, _file: file })
}

fn render_page(document: &PdfDocument, index: u32, page_count: u32) -> Result<PdfPageResult, String> {
    let page = native(document.GetPage(index))?;
    let rendered = (|| {
        let size = native(page.Size())?;
        if !size.Width.is_finite() || !size.Height.is_finite() || size.Width <= 0.0 || size.Height <= 0.0 {
            return Err("Preview invalid PDF page dimensions".into());
        }
        let scale = (1600.0 / size.Width).min(2000.0 / size.Height).min(1.0);
        let options = native(PdfPageRenderOptions::new())?;
        native(options.SetDestinationWidth((size.Width * scale).floor().max(1.0) as u32))?;
        native(options.SetDestinationHeight((size.Height * scale).floor().max(1.0) as u32))?;
        native(options.SetBitmapEncoderId(native(BitmapEncoder::PngEncoderId())?))?;
        let output = native(InMemoryRandomAccessStream::new())?;
        native(native(page.RenderWithOptionsToStreamAsync(&output, &options))?.get())?;
        let count = native(output.Size())?;
        if count == 0 || count > PNG_LIMIT { return Err("Preview PDF output limit exceeded".into()); }
        let reader = native(DataReader::CreateDataReader(&native(output.GetInputStreamAt(0))?))?;
        if native(native(reader.LoadAsync(count as u32))?.get())? != count as u32 { return Err(UNAVAILABLE.into()); }
        let mut png = vec![0; count as usize];
        native(reader.ReadBytes(&mut png))?;
        Ok(PdfPageResult { data: STANDARD.encode(png), page_count, page_index: index })
    })();
    let _ = page.Close();
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine};
    use std::{fs, io::{Seek, SeekFrom}, path::PathBuf, sync::atomic::{AtomicU64, Ordering}, time::{Duration, Instant}};

    struct Fixture(PathBuf);

    impl Fixture {
        fn new(bytes: &[u8]) -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!("supersearch-pdf-{}-{}.pdf", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            fs::write(&path, bytes).unwrap();
            Self(path)
        }

        fn open(&self) -> File { File::open(&self.0).unwrap() }
    }

    impl Drop for Fixture {
        fn drop(&mut self) { let _ = fs::remove_file(&self.0); }
    }

    // 使用完整交叉引用表生成真实 PDF，不依赖用户文件或外部生成工具。
    fn pdf(width: u32, height: u32) -> Vec<u8> {
        let red = "1 0 0 rg 10 10 80 80 re f\n";
        let blue = "0 0 1 rg 10 10 80 80 re f\n";
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".into(),
            "<< /Type /Pages /Count 2 /Kids [3 0 R 5 0 R] >>".into(),
            format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] /Resources << >> /Contents 4 0 R >>"),
            format!("<< /Length {} >>\nstream\n{red}endstream", red.len()),
            format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] /Resources << >> /Contents 6 0 R >>"),
            format!("<< /Length {} >>\nstream\n{blue}endstream", blue.len()),
        ];
        let mut body = "%PDF-1.4\n".to_string();
        let mut offsets = Vec::new();
        for (index, object) in objects.iter().enumerate() {
            offsets.push(body.len());
            body.push_str(&format!("{} 0 obj\n{object}\nendobj\n", index + 1));
        }
        let xref = body.len();
        body.push_str("xref\n0 7\n0000000000 65535 f \n");
        for offset in offsets { body.push_str(&format!("{offset:010} 00000 n \n")); }
        body.push_str(&format!("trailer\n<< /Size 7 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n"));
        body.into_bytes()
    }

    #[test]
    fn renders_requested_pdf_pages_and_reuses_document() {
        let source = Fixture::new(&pdf(300, 400));
        let state = PdfState::default();
        state.begin("pages");
        let first = state.render("pages", source.open(), 0).unwrap();
        assert_eq!((first.page_index, first.page_count), (0, 2));
        let png = STANDARD.decode(&first.data).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        // 同一文件句柄位于 EOF，只有复用文档而不重新读取才能成功。
        let mut at_end = source.open();
        at_end.seek(SeekFrom::End(0)).unwrap();
        let second = state.render("pages", at_end, 1).unwrap();
        assert_eq!((second.page_index, second.page_count), (1, 2));
        assert_ne!(first.data, second.data);
    }

    #[test]
    fn rejects_corrupt_pdf_and_out_of_range_page() {
        let corrupt = Fixture::new(b"%PDF-1.7\ncorrupt");
        let source = Fixture::new(&pdf(100, 200));
        let state = PdfState::default();
        state.begin("invalid");
        assert_eq!(state.render("invalid", corrupt.open(), 0).unwrap_err(), "Preview invalid or password protected PDF");
        assert_eq!(state.render("invalid", source.open(), 2).unwrap_err(), "Preview PDF page out of range");
        assert_eq!(state.render("invalid", source.open(), u32::MAX).unwrap_err(), "Preview PDF page out of range");
    }

    #[test]
    fn reused_preview_id_cannot_read_a_different_cached_file() {
        let first = Fixture::new(&pdf(100, 200));
        let second = Fixture::new(&pdf(300, 400));
        let state = PdfState::default();
        state.begin("identity");
        state.render("identity", first.open(), 0).unwrap();
        assert_eq!(state.render("identity", second.open(), 1).unwrap_err(), "Preview PDF session mismatch");
        assert!(state.render("identity", first.open(), 1).is_ok());
    }

    #[test]
    fn bounds_rendered_png_dimensions() {
        let source = Fixture::new(&pdf(10000, 20000));
        let state = PdfState::default();
        state.begin("large");
        let png = STANDARD.decode(state.render("large", source.open(), 0).unwrap().data).unwrap();
        let width = u32::from_be_bytes(png[16..20].try_into().unwrap());
        let height = u32::from_be_bytes(png[20..24].try_into().unwrap());
        assert!(width > 0 && width <= 1600);
        assert!(height > 0 && height <= 2000);
        assert!(u64::from(width) * u64::from(height) <= 3_200_000);
        assert!(png.len() <= 16 * 1024 * 1024);
    }

    #[test]
    fn cancellation_does_not_wait_for_render_lock() {
        let source = Fixture::new(&pdf(100, 200));
        let state = PdfState::default();
        state.begin("old");
        let render_lock = state.cache.lock().unwrap();
        let start = Instant::now();
        state.cancel("old");
        state.clear();
        state.begin("new");
        assert!(start.elapsed() < Duration::from_millis(100));
        drop(render_lock);
        assert_eq!(state.render("old", source.open(), 0).unwrap_err(), "Preview PDF cancelled");
        assert!(state.render("new", source.open(), 0).is_ok());
    }

    #[test]
    fn replacing_or_closing_preview_releases_cached_document() {
        let source = Fixture::new(&pdf(100, 200));
        let corrupt = Fixture::new(b"not a PDF");
        let state = PdfState::default();
        state.begin("one");
        state.render("one", source.open(), 0).unwrap();
        assert!(state.cache.lock().unwrap().is_some());
        state.cancel("unrelated");
        assert!(state.render("one", source.open(), 1).is_ok());
        state.clear();
        assert!(state.cache.lock().unwrap().is_none());
        state.begin("two");
        assert_eq!(state.render("two", corrupt.open(), 0).unwrap_err(), "Preview invalid or password protected PDF");
        assert!(state.cache.lock().unwrap().is_none());
    }

    #[test]
    fn close_before_begin_cannot_resurrect_preview() {
        let source = Fixture::new(&pdf(100, 200));
        let state = PdfState::default();
        state.cancel("late");
        state.begin("late");
        assert_eq!(state.render("late", source.open(), 0).unwrap_err(), "Preview PDF cancelled");
        assert!(state.cache.lock().unwrap().is_none());
    }

    #[test]
    fn cancelled_delayed_begin_preserves_current_preview() {
        let source = Fixture::new(&pdf(100, 200));
        let state = PdfState::default();
        state.cancel("closed");
        state.begin("current");
        state.render("current", source.open(), 0).unwrap();
        state.begin("closed");
        assert!(state.render("current", source.open(), 1).is_ok());
        state.clear();
        state.begin("current");
        assert_eq!(state.render("current", source.open(), 0).unwrap_err(), "Preview PDF cancelled");
    }

    #[test]
    fn keeps_cancelled_sessions_bounded() {
        let state = PdfState::default();
        for index in 0..1000 { state.cancel(&format!("closed-{index}")); }
        let session = state.session.lock().unwrap();
        assert_eq!(session.cancelled.len(), 64);
        assert_eq!(session.cancelled.front().unwrap(), "closed-936");
        assert_eq!(session.cancelled.back().unwrap(), "closed-999");
    }

    #[test]
    fn busy_renderer_does_not_start_another_decode() {
        let source = Fixture::new(&pdf(100, 200));
        let state = PdfState::default();
        state.begin("busy");
        let render_lock = state.cache.lock().unwrap();
        assert_eq!(state.render("busy", source.open(), 0).unwrap_err(), "Preview PDF busy");
        drop(render_lock);
        assert!(state.render("busy", source.open(), 0).is_ok());
    }

    #[test]
    fn closing_during_native_work_cleans_cache_on_completion() {
        let source = Fixture::new(&pdf(10000, 20000));
        let state = PdfState::default();
        state.begin("in-flight");
        let (loaded_tx, loaded_rx) = std::sync::mpsc::sync_channel(1);
        let (resume_tx, resume_rx) = std::sync::mpsc::sync_channel(1);
        *state.after_load.lock().unwrap() = Some(RenderGate { loaded: loaded_tx, resume: resume_rx });
        std::thread::scope(|scope| {
            let render = scope.spawn(|| state.render("in-flight", source.open(), 0));
            loaded_rx.recv_timeout(Duration::from_secs(5)).expect("Native PDF load did not complete");
            state.cancel("in-flight");
            resume_tx.send(()).unwrap();
            assert_eq!(render.join().unwrap().unwrap_err(), "Preview PDF cancelled");
        });
        assert!(state.cache.lock().unwrap().is_none());
    }

    #[test]
    fn decode_cancellation_discards_document_before_caching() {
        let source = Fixture::new(&pdf(100, 200));
        let _apartment = Apartment::new().unwrap();
        let state = PdfState::default();
        state.begin("decode");
        let epoch = state.epoch.load(Ordering::SeqCst);
        let checks = AtomicU64::new(0);
        let loaded = load_document(source.open(), epoch, || {
            // 小文件依次经过读取、EOF、载入前、载入后四个取消检查点。
            if checks.fetch_add(1, Ordering::SeqCst) == 3 { state.cancel("decode"); }
            state.ensure_current(epoch)
        });
        assert_eq!(loaded.err().unwrap(), "Preview PDF cancelled");
        assert!(state.cache.lock().unwrap().is_none());
    }

    #[test]
    fn rejects_pdf_over_input_limit_without_reading_it() {
        let source = Fixture::new(b"%PDF-1.4\n");
        fs::OpenOptions::new().write(true).open(&source.0).unwrap().set_len(INPUT_LIMIT + 1).unwrap();
        let state = PdfState::default();
        state.begin("oversized");
        assert_eq!(state.render("oversized", source.open(), 0).unwrap_err(), "Preview size limit exceeded");
        assert!(state.cache.lock().unwrap().is_none());
    }
}
