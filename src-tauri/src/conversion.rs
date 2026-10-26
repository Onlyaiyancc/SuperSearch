mod csv;
mod files;
mod images;

use files::{commit_output, DirectoryGrant};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};

pub const INPUT_LIMIT: u64 = 200 * 1024 * 1024;
pub const OUTPUT_LIMIT: u64 = 2 * 1024 * 1024 * 1024;
pub const PIXEL_LIMIT: u64 = 50_000_000;

pub use csv::CsvPreview;
pub fn normalize_csv(
    source: &Path,
    output: &Path,
    options: &ConversionOptions,
) -> Result<(), String> {
    csv::normalize(source, output, options)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct ConversionOptions {
    pub target: String,
    pub quality: u8,
    pub dpi: u16,
    pub ico_size: u16,
    pub pages: String,
    pub sheet: Option<u32>,
    pub merge: bool,
    pub ocr: bool,
    pub csv_delimiter: String,
    pub csv_encoding: String,
}

impl Default for ConversionOptions {
    fn default() -> Self {
        Self {
            target: "png".into(),
            quality: 90,
            dpi: 150,
            ico_size: 256,
            pages: String::new(),
            sheet: None,
            merge: false,
            ocr: false,
            csv_delimiter: ",".into(),
            csv_encoding: "utf-8".into(),
        }
    }
}

impl ConversionOptions {
    pub fn validate(&self) -> Result<(), String> {
        if !matches!(
            self.target.as_str(),
            "png"
                | "jpg"
                | "webp"
                | "bmp"
                | "tiff"
                | "gif"
                | "ico"
                | "pdf"
                | "doc"
                | "docx"
                | "odt"
                | "rtf"
                | "txt"
                | "html"
                | "md"
                | "xls"
                | "xlsx"
                | "ods"
                | "csv"
                | "ppt"
                | "pptx"
                | "odp"
        ) {
            return Err("Conversion unsupported target".into());
        }
        if !(1..=100).contains(&self.quality)
            || !matches!(self.dpi, 150 | 300)
            || !matches!(self.ico_size, 16 | 32 | 48 | 64 | 128 | 256)
            || self.sheet.is_some_and(|value| value == 0 || value > 1024)
            || self.pages.len() > 2048
        {
            return Err("Conversion invalid options".into());
        }
        parse_pages(&self.pages)?;
        csv::validate_selection(&self.csv_delimiter, &self.csv_encoding)?;
        if self.merge && self.target != "pdf" {
            return Err("Conversion merge requires PDF target".into());
        }
        if self.ocr && !matches!(self.target.as_str(), "pdf" | "docx" | "txt") {
            return Err("Conversion invalid OCR target".into());
        }
        Ok(())
    }
}

pub fn parse_pages(value: &str) -> Result<Vec<u32>, String> {
    if value.trim().is_empty() || value.trim() == "all" {
        return Ok(Vec::new());
    }
    let mut pages = Vec::new();
    let mut seen = HashSet::new();
    for span in value.split(',') {
        let pair = span.trim().split('-').collect::<Vec<_>>();
        if pair.len() > 2 || pair.is_empty() {
            return Err("Conversion invalid pages".into());
        }
        let start = pair[0]
            .trim()
            .parse::<u32>()
            .map_err(|_| "Conversion invalid pages")?;
        let end = if pair.len() == 2 {
            pair[1]
                .trim()
                .parse::<u32>()
                .map_err(|_| "Conversion invalid pages")?
        } else {
            start
        };
        if start == 0 || end < start || end > 500 {
            return Err("Conversion invalid pages".into());
        }
        for page in start..=end {
            if seen.insert(page) {
                pages.push(page);
            }
        }
    }
    Ok(pages)
}

pub fn image_format(extension: &str) -> bool {
    matches!(
        extension,
        "png" | "jpg" | "jpeg" | "webp" | "bmp" | "tif" | "tiff" | "gif" | "ico"
    )
}

pub fn supported_target(source: &str, target: &str) -> bool {
    if image_format(source) {
        return matches!(
            target,
            "png" | "jpg" | "webp" | "bmp" | "tiff" | "gif" | "ico" | "pdf" | "txt" | "docx"
        );
    }
    match source {
        "pdf" => matches!(
            target,
            "png" | "jpg" | "txt" | "docx" | "xlsx" | "csv" | "pptx" | "pdf"
        ),
        "doc" | "docx" | "odt" | "rtf" => {
            matches!(
                target,
                "doc" | "docx" | "odt" | "rtf" | "pdf" | "txt" | "html"
            ) || source == "docx" && target == "md"
        }
        "xls" | "xlsx" | "ods" => matches!(target, "xls" | "xlsx" | "ods" | "csv" | "pdf"),
        "csv" => matches!(target, "xlsx" | "ods"),
        "ppt" | "pptx" | "odp" => matches!(target, "ppt" | "pptx" | "odp" | "pdf"),
        "md" | "markdown" | "txt" | "html" | "htm" => {
            matches!(target, "md" | "txt" | "html" | "docx" | "pdf")
        }
        _ => false,
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionSource {
    pub id: String,
    pub name: String,
    pub extension: String,
    pub bytes: u64,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionDirectory {
    pub id: String,
    pub label: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionOutput {
    pub id: String,
    pub name: String,
    pub path: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionItem {
    pub source_id: String,
    pub name: String,
    pub status: String,
    pub outputs: Vec<ConversionOutput>,
    pub error: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionJob {
    pub id: String,
    pub status: String,
    pub target: String,
    pub options: ConversionOptions,
    pub items: Vec<ConversionItem>,
    pub completed: usize,
    pub total: usize,
}

struct SourceGrant {
    summary: ConversionSource,
    path: PathBuf,
    identity: files::FileIdentity,
    parent_identity: files::FileIdentity,
    created: Instant,
}
#[derive(Clone)]
struct OutputGrant {
    path: PathBuf,
    identity: files::FileIdentity,
}
struct Job {
    summary: Mutex<ConversionJob>,
    cancel: AtomicBool,
    sources: Vec<Arc<SourceGrant>>,
    output: Option<OutputGrant>,
}
struct Inner {
    root: PathBuf,
    sources: Mutex<HashMap<String, Arc<SourceGrant>>>,
    directories: Mutex<HashMap<String, OutputGrant>>,
    jobs: Mutex<VecDeque<Arc<Job>>>,
    outputs: Mutex<HashMap<String, PathBuf>>,
    running: AtomicBool,
}
#[derive(Clone)]
pub struct ConversionState(Arc<Inner>);

fn locked<T>(value: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>, String> {
    value
        .lock()
        .map_err(|_| "Conversion state unavailable".into())
}
fn unique_id() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("c-{tick:x}-{:x}", NEXT.fetch_add(1, Ordering::Relaxed))
}
pub fn check_cancel(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Acquire) {
        Err("Conversion cancelled".into())
    } else {
        Ok(())
    }
}

impl ConversionState {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&root).map_err(|_| "Conversion storage unavailable")?;
        let jobs = root.join("conversion-jobs");
        // 此目录只存本应用私有临时文件；先检查重解析点，避免启动清理越过数据目录。
        if jobs.exists() {
            files::clean_owned_directory(&jobs)?;
        }
        fs::create_dir_all(&jobs).map_err(|_| "Conversion storage unavailable")?;
        Ok(Self(Arc::new(Inner {
            root,
            sources: Mutex::new(HashMap::new()),
            directories: Mutex::new(HashMap::new()),
            jobs: Mutex::new(VecDeque::new()),
            outputs: Mutex::new(HashMap::new()),
            running: AtomicBool::new(false),
        })))
    }

    #[cfg(test)]
    pub fn grant_path(&self, path: PathBuf) -> Result<ConversionSource, String> {
        self.grant_paths(vec![path])?
            .pop()
            .ok_or("Conversion no files selected".into())
    }

    pub fn grant_paths(&self, paths: Vec<PathBuf>) -> Result<Vec<ConversionSource>, String> {
        if paths.is_empty() || paths.len() > 100 {
            return Err("Conversion select at most 100 files".into());
        }
        let mut added = Vec::new();
        for path in paths {
            let path = files::normal_path(&path)?;
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if !supported_target(&extension, "pdf") && extension != "pdf" && extension != "csv" {
                return Err("Conversion unsupported source format".into());
            }
            let file = crate::file_preview::open_bounded_file(
                path.to_str().ok_or("Conversion invalid path")?,
                INPUT_LIMIT,
            )
            .map_err(|error| error.replace("Preview", "Conversion"))?;
            let identity = files::identity(&file)?;
            let parent =
                DirectoryGrant::open(path.parent().ok_or("Conversion invalid source directory")?)?;
            let parent_identity = parent.identity;
            let bytes = file
                .metadata()
                .map_err(|_| "Conversion source unavailable")?
                .len();
            if extension == "csv" && bytes > csv::CSV_INPUT_LIMIT {
                return Err("Conversion CSV input size limit exceeded".into());
            }
            if bytes == 0 {
                return Err("Conversion empty source file".into());
            }
            let summary = ConversionSource {
                id: unique_id(),
                name: path
                    .file_name()
                    .ok_or("Conversion invalid source name")?
                    .to_string_lossy()
                    .into_owned(),
                extension,
                bytes,
            };
            // 授权只保留文件身份；真正执行时重开并核对，不让未提交的选择锁住用户文件。
            drop(file);
            added.push(Arc::new(SourceGrant {
                summary,
                path,
                identity,
                parent_identity,
                created: Instant::now(),
            }));
        }
        let mut sources = locked(&self.0.sources)?;
        sources.retain(|_, value| {
            Arc::strong_count(value) > 1 || value.created.elapsed().as_secs() < 3600
        });
        let mut result = Vec::new();
        let mut selected = HashSet::new();
        for source in added {
            let summary = if let Some(existing) = sources.values().find(|existing| {
                existing.path == source.path && existing.identity == source.identity
            }) {
                existing.summary.clone()
            } else {
                if sources.len() >= 2200 {
                    return Err(
                        "Conversion source capacity reached; restart or remove older selections"
                            .into(),
                    );
                }
                let summary = source.summary.clone();
                sources.insert(summary.id.clone(), source);
                summary
            };
            if selected.insert(summary.id.clone()) {
                result.push(summary);
            }
        }
        Ok(result)
    }

    pub fn has_active(&self) -> bool {
        self.0
            .jobs
            .lock()
            .map(|jobs| {
                jobs.iter().any(|job| {
                    job.summary
                        .lock()
                        .map(|value| matches!(value.status.as_str(), "queued" | "running"))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    }
    pub fn cancel_all(&self) {
        if let Ok(jobs) = self.0.jobs.lock() {
            for job in jobs.iter() {
                job.cancel.store(true, Ordering::Release);
            }
        }
    }
    pub fn list(&self) -> Result<Vec<ConversionJob>, String> {
        locked(&self.0.jobs)?
            .iter()
            .rev()
            .map(|job| Ok(locked(&job.summary)?.clone()))
            .collect()
    }

    fn retry(&self, job_id: String, source_id: Option<String>) -> Result<Arc<Job>, String> {
        let original = locked(&self.0.jobs)?
            .iter()
            .find(|job| {
                job.summary
                    .lock()
                    .map(|summary| summary.id == job_id)
                    .unwrap_or(false)
            })
            .cloned()
            .ok_or("Conversion job unavailable")?;
        // 只复制历史任务已持有的授权；释放历史锁后再走公共校验和入队流程。
        let (sources, options) = {
            let summary = locked(&original.summary)?;
            if summary.status != "failed"
                || summary.items.iter().any(|item| {
                    !matches!(item.status.as_str(), "completed" | "failed" | "cancelled")
                })
            {
                return Err("Conversion retry requires failed terminal job".into());
            }
            let sources = if summary.options.merge {
                if source_id.is_some() {
                    return Err("Conversion merged retry requires whole batch".into());
                }
                original.sources.clone()
            } else {
                let source_id = source_id.ok_or("Conversion retry source required")?;
                if !summary
                    .items
                    .iter()
                    .any(|item| item.source_id == source_id && item.status == "failed")
                {
                    return Err("Conversion retry source is not a failed item".into());
                }
                vec![original
                    .sources
                    .iter()
                    .find(|source| source.summary.id == source_id)
                    .cloned()
                    .ok_or("Conversion retry source is not a failed item")?]
            };
            (sources, summary.options.clone())
        };
        self.enqueue(sources, original.output.clone(), options)
    }

    fn submit(
        &self,
        source_ids: Vec<String>,
        output_id: Option<String>,
        options: ConversionOptions,
    ) -> Result<Arc<Job>, String> {
        if source_ids.is_empty() || source_ids.len() > 100 {
            return Err("Conversion select at most 100 files".into());
        }
        let mut seen = HashSet::new();
        let sources = {
            let granted = locked(&self.0.sources)?;
            source_ids
                .iter()
                .map(|id| {
                    if !seen.insert(id) {
                        return Err("Conversion duplicate source".into());
                    }
                    granted
                        .get(id)
                        .cloned()
                        .ok_or("Conversion source grant expired; select files again".into())
                })
                .collect::<Result<Vec<_>, String>>()?
        };
        let output = match output_id {
            Some(id) => Some(
                locked(&self.0.directories)?
                    .get(&id)
                    .cloned()
                    .ok_or("Conversion output grant expired")?,
            ),
            None => None,
        };
        self.enqueue(sources, output, options)
    }

    fn enqueue(
        &self,
        sources: Vec<Arc<SourceGrant>>,
        output: Option<OutputGrant>,
        options: ConversionOptions,
    ) -> Result<Arc<Job>, String> {
        options.validate()?;
        if sources.is_empty() || sources.len() > 100 {
            return Err("Conversion select at most 100 files".into());
        }
        if sources
            .iter()
            .map(|source| source.summary.bytes)
            .sum::<u64>()
            > OUTPUT_LIMIT
        {
            return Err("Conversion batch input size limit exceeded".into());
        }
        for source in &sources {
            let ext = &source.summary.extension;
            if !supported_target(ext, &options.target)
                || image_format(ext)
                    && matches!(options.target.as_str(), "txt" | "docx")
                    && !options.ocr
                || ext == "pdf" && options.target == "pdf" && !options.ocr
            {
                return Err("Conversion unsupported direction or OCR is required".into());
            }
            if options.merge && !image_format(ext) {
                return Err("Conversion merge requires image sources".into());
            }
            if matches!(ext.as_str(), "xls" | "xlsx" | "ods")
                && options.target == "csv"
                && options.sheet.is_none()
            {
                return Err("Conversion choose worksheet for CSV".into());
            }
        }
        let summary = ConversionJob {
            id: unique_id(),
            status: "queued".into(),
            target: options.target.clone(),
            options,
            items: sources
                .iter()
                .map(|source| ConversionItem {
                    source_id: source.summary.id.clone(),
                    name: source.summary.name.clone(),
                    status: "pending".into(),
                    outputs: vec![],
                    error: String::new(),
                })
                .collect(),
            completed: 0,
            total: sources.len(),
        };
        let job = Arc::new(Job {
            summary: Mutex::new(summary),
            cancel: AtomicBool::new(false),
            sources,
            output,
        });
        let mut jobs = locked(&self.0.jobs)?;
        if jobs
            .iter()
            .filter(|job| {
                job.summary
                    .lock()
                    .map(|value| value.status == "queued")
                    .unwrap_or(true)
            })
            .count()
            >= 5
        {
            return Err("Conversion queue capacity reached".into());
        }
        // 完成历史只保留二十批；仍在运行或排队的任务不会被移除。
        while jobs.len() >= 20 {
            if let Some(index) = jobs.iter().position(|job| {
                job.summary
                    .lock()
                    .map(|value| !matches!(value.status.as_str(), "queued" | "running"))
                    .unwrap_or(false)
            }) {
                let removed = jobs.remove(index).unwrap();
                if let Ok(summary) = removed.summary.lock() {
                    if let Ok(mut outputs) = self.0.outputs.lock() {
                        for output in summary.items.iter().flat_map(|item| &item.outputs) {
                            outputs.remove(&output.id);
                        }
                    }
                };
            } else {
                break;
            }
        }
        jobs.push_back(job.clone());
        Ok(job)
    }

    fn wake(&self, app: AppHandle) {
        if self
            .0
            .running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        let state = self.clone();
        std::thread::spawn(move || loop {
            let next = match state.claim_next() {
                Ok(value) => value,
                Err(_) => {
                    state.0.running.store(false, Ordering::Release);
                    return;
                }
            };
            let Some(job) = next else {
                break;
            };
            state.run(&job, &|| {
                let _ = app.emit("conversions_changed", ());
            });
        });
    }

    fn claim_next(&self) -> Result<Option<Arc<Job>>, String> {
        let jobs = locked(&self.0.jobs)?;
        let next = jobs.iter().find_map(|job| {
            let mut summary = job.summary.lock().ok()?;
            if summary.status != "queued" {
                return None;
            }
            summary.status = "running".into();
            Some(job.clone())
        });
        // 清空标志与检查队列在同一锁内，避免入队刚好遇到工作线程退出而丢失唤醒。
        if next.is_none() {
            self.0.running.store(false, Ordering::Release);
        }
        Ok(next)
    }

    fn run(&self, job: &Job, notify: &dyn Fn()) {
        let (id, options) = match job.summary.lock() {
            Ok(value) => (value.id.clone(), value.options.clone()),
            Err(_) => return,
        };
        notify();
        let workspace = self.0.root.join("conversion-jobs").join(&id);
        let result = (|| -> Result<(), String> {
            fs::create_dir(&workspace).map_err(|_| "Conversion storage unavailable")?;
            if options.merge {
                self.run_merged(job, &workspace, &options)?;
            } else {
                let mut output_bytes = 0;
                for (index, source) in job.sources.iter().enumerate() {
                    if check_cancel(&job.cancel).is_err() {
                        break;
                    }
                    update_item(job, index, "running", vec![], String::new());
                    notify();
                    let result = self.run_item(
                        source,
                        job.output.as_ref(),
                        &workspace.join(index.to_string()),
                        &options,
                        &job.cancel,
                        &mut output_bytes,
                    );
                    match result {
                        Ok(outputs) => update_item(job, index, "completed", outputs, String::new()),
                        Err(error) => update_item(
                            job,
                            index,
                            if job.cancel.load(Ordering::Acquire) {
                                "cancelled"
                            } else {
                                "failed"
                            },
                            vec![],
                            error,
                        ),
                    }
                    notify();
                }
            }
            Ok(())
        })();
        if let Ok(mut summary) = job.summary.lock() {
            let cancelled = job.cancel.load(Ordering::Acquire);
            for item in &mut summary.items {
                if matches!(item.status.as_str(), "pending" | "running") {
                    item.status = if cancelled { "cancelled" } else { "failed" }.into();
                    item.error = result
                        .as_ref()
                        .err()
                        .cloned()
                        .unwrap_or_else(|| "Conversion cancelled".into());
                }
            }
            summary.completed = summary
                .items
                .iter()
                .filter(|item| matches!(item.status.as_str(), "completed" | "failed" | "cancelled"))
                .count();
            summary.status = if cancelled {
                "cancelled"
            } else if summary.items.iter().any(|item| item.status == "failed") {
                "failed"
            } else {
                "completed"
            }
            .into();
        }
        // 临时源副本仅存在本批处理期间，成功输出已经独立提交。
        let _ = files::clean_owned_directory(&workspace);
        notify();
    }

    fn run_item(
        &self,
        source: &SourceGrant,
        output: Option<&OutputGrant>,
        workspace: &Path,
        options: &ConversionOptions,
        cancel: &AtomicBool,
        total: &mut u64,
    ) -> Result<Vec<ConversionOutput>, String> {
        check_cancel(cancel)?;
        fs::create_dir(workspace).map_err(|_| "Conversion storage unavailable")?;
        let staged = stage_source(source, workspace, cancel)?;
        let output_dir = workspace.join("output");
        fs::create_dir(&output_dir).map_err(|_| "Conversion storage unavailable")?;
        let converted = if image_format(&source.summary.extension)
            && image_format(&options.target)
            && !options.ocr
        {
            images::convert(&staged, &output_dir, options, cancel)?
        } else {
            crate::conversion_components::convert(
                &self.0.root,
                &staged,
                &output_dir,
                options,
                cancel,
            )?
        };
        let directory = output_directory(source, output)?;
        self.commit_outputs(
            converted,
            &output_dir,
            &directory,
            &source.summary.name,
            cancel,
            total,
        )
    }

    fn run_merged(
        &self,
        job: &Job,
        workspace: &Path,
        options: &ConversionOptions,
    ) -> Result<(), String> {
        let mut staged = Vec::new();
        for (index, source) in job.sources.iter().enumerate() {
            update_item(job, index, "running", vec![], String::new());
            let subdir = workspace.join(index.to_string());
            fs::create_dir(&subdir).map_err(|_| "Conversion storage unavailable")?;
            staged.push(stage_source(source, &subdir, &job.cancel)?);
        }
        let output_dir = workspace.join("output");
        fs::create_dir(&output_dir).map_err(|_| "Conversion storage unavailable")?;
        let converted = crate::conversion_components::convert_many(
            &self.0.root,
            &staged,
            &output_dir,
            options,
            &job.cancel,
        )?;
        let source = &job.sources[0];
        let directory = output_directory(source, job.output.as_ref())?;
        let outputs = self.commit_outputs(
            converted,
            &output_dir,
            &directory,
            &source.summary.name,
            &job.cancel,
            &mut 0,
        )?;
        for index in 0..job.sources.len() {
            update_item(
                job,
                index,
                "completed",
                if index == 0 { outputs.clone() } else { vec![] },
                String::new(),
            );
        }
        Ok(())
    }

    fn commit_outputs(
        &self,
        paths: Vec<PathBuf>,
        staging: &Path,
        directory: &DirectoryGrant,
        source_name: &str,
        cancel: &AtomicBool,
        total: &mut u64,
    ) -> Result<Vec<ConversionOutput>, String> {
        if paths.is_empty() || paths.len() > 2000 {
            return Err("Conversion invalid output count".into());
        }
        let mut bytes = 0u64;
        for path in &paths {
            files::validate_staged_output(path, staging)?;
            bytes = bytes
                .checked_add(
                    fs::metadata(path)
                        .map_err(|_| "Conversion output unavailable")?
                        .len(),
                )
                .ok_or("Conversion output size limit exceeded")?;
        }
        if bytes == 0 || total.saturating_add(bytes) > OUTPUT_LIMIT {
            return Err("Conversion output size limit exceeded".into());
        }
        check_cancel(cancel)?;
        let folder = if paths.len() > 1 {
            Some(
                directory.create_unique_child(
                    Path::new(source_name)
                        .file_stem()
                        .and_then(|value| value.to_str())
                        .unwrap_or("转换结果"),
                )?,
            )
        } else {
            None
        };
        let destination = folder.as_ref().unwrap_or(directory);
        let mut completed = Vec::new();
        let result = (|| {
            for path in paths {
                check_cancel(cancel)?;
                let relative = path
                    .strip_prefix(staging)
                    .map_err(|_| "Conversion invalid output location")?;
                let target = if let Some(parent) = relative
                    .parent()
                    .filter(|value| !value.as_os_str().is_empty())
                {
                    Some(destination.create_relative_child(parent)?)
                } else {
                    None
                };
                let actual = commit_output(
                    &path,
                    target.as_ref().unwrap_or(destination),
                    relative
                        .file_name()
                        .and_then(|value| value.to_str())
                        .ok_or("Conversion invalid output name")?,
                    cancel,
                )?;
                let id = unique_id();
                completed.push(ConversionOutput {
                    id,
                    name: actual
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    path: actual.to_string_lossy().into_owned(),
                });
            }
            check_cancel(cancel)?;
            Ok::<(), String>(())
        })();
        if let Err(error) = result {
            for output in &completed {
                let _ = fs::remove_file(&output.path);
            }
            return Err(error);
        }
        let mut grants = locked(&self.0.outputs)?;
        for output in &completed {
            grants.insert(output.id.clone(), PathBuf::from(&output.path));
        }
        *total += bytes;
        Ok(completed)
    }
}

fn update_item(
    job: &Job,
    index: usize,
    status: &str,
    outputs: Vec<ConversionOutput>,
    error: String,
) {
    if let Ok(mut summary) = job.summary.lock() {
        if let Some(item) = summary.items.get_mut(index) {
            item.status = status.into();
            item.outputs = outputs;
            item.error = error;
        }
        summary.completed = summary
            .items
            .iter()
            .filter(|item| matches!(item.status.as_str(), "completed" | "failed" | "cancelled"))
            .count();
    }
}

fn output_directory(
    source: &SourceGrant,
    selected: Option<&OutputGrant>,
) -> Result<DirectoryGrant, String> {
    let (path, expected) = match selected {
        Some(selected) => (selected.path.as_path(), selected.identity),
        None => (
            source
                .path
                .parent()
                .ok_or("Conversion output unavailable")?,
            source.parent_identity,
        ),
    };
    // 只在提交结果期间固定目录及祖先；草稿和已完成历史不长期阻止用户移动文件夹。
    let directory = DirectoryGrant::open(path)?;
    if directory.identity != expected {
        return Err("Conversion output directory changed; choose it again".into());
    }
    Ok(directory)
}

fn stage_source(
    source: &SourceGrant,
    directory: &Path,
    cancel: &AtomicBool,
) -> Result<PathBuf, String> {
    let mut file = crate::file_preview::open_bounded_file(
        source.path.to_str().ok_or("Conversion invalid path")?,
        INPUT_LIMIT,
    )
    .map_err(|_| "Conversion source changed; select files again")?;
    if files::identity(&file)? != source.identity {
        return Err("Conversion source changed; select files again".into());
    }
    let output = directory.join(&source.summary.name);
    let mut writer = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)
        .map_err(|_| "Conversion storage unavailable")?;
    let mut total = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        check_cancel(cancel)?;
        let count = file
            .read(&mut buffer)
            .map_err(|_| "Conversion source unavailable")?;
        if count == 0 {
            break;
        }
        total += count as u64;
        if total > INPUT_LIMIT || total > source.summary.bytes {
            return Err("Conversion source changed or size limit exceeded".into());
        }
        writer
            .write_all(&buffer[..count])
            .map_err(|_| "Conversion storage unavailable")?;
    }
    if total != source.summary.bytes || files::identity(&file)? != source.identity {
        return Err("Conversion source changed; select files again".into());
    }
    writer
        .flush()
        .map_err(|_| "Conversion storage unavailable")?;
    Ok(output)
}

fn require_enabled(app: &AppHandle) -> Result<(), String> {
    if app
        .state::<crate::service::AppState>()
        .store
        .lock()
        .map_err(|_| "Conversion state unavailable")?
        .settings()
        .conversion_enabled
    {
        Ok(())
    } else {
        Err("Conversion plugin disabled".into())
    }
}

#[tauri::command]
pub async fn choose_conversion_files(app: AppHandle) -> Result<Vec<ConversionSource>, String> {
    require_enabled(&app)?;
    let paths = tauri::async_runtime::spawn_blocking(|| files::choose(false))
        .await
        .map_err(|_| "Conversion picker unavailable")??;
    if paths.is_empty() {
        return Ok(vec![]);
    }
    require_enabled(&app)?;
    let state = app.state::<ConversionState>().inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.grant_paths(paths))
        .await
        .map_err(|_| "Conversion source unavailable")?
}

#[tauri::command]
pub async fn choose_conversion_directory(
    app: AppHandle,
) -> Result<Option<ConversionDirectory>, String> {
    require_enabled(&app)?;
    let paths = tauri::async_runtime::spawn_blocking(|| files::choose(true))
        .await
        .map_err(|_| "Conversion picker unavailable")??;
    let Some(path) = paths.into_iter().next() else {
        return Ok(None);
    };
    require_enabled(&app)?;
    let directory = DirectoryGrant::open(&path)?;
    let value = ConversionDirectory {
        id: unique_id(),
        label: directory.path.to_string_lossy().into_owned(),
    };
    let state = app.state::<ConversionState>();
    let mut directories = locked(&state.0.directories)?;
    if directories.len() >= 100 {
        directories.clear();
    }
    directories.insert(
        value.id.clone(),
        OutputGrant {
            path: directory.path.clone(),
            identity: directory.identity,
        },
    );
    Ok(Some(value))
}

#[tauri::command]
pub fn release_conversion_sources(app: AppHandle, ids: Vec<String>) -> Result<(), String> {
    if ids.len() > 100 {
        return Err("Conversion invalid source count".into());
    }
    let state = app.state::<ConversionState>();
    let mut sources = locked(&state.0.sources)?;
    for id in ids {
        if sources
            .get(&id)
            .is_some_and(|source| Arc::strong_count(source) == 1)
        {
            sources.remove(&id);
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn preview_conversion_csv(
    app: AppHandle,
    source_id: String,
    delimiter: String,
    encoding: String,
) -> Result<CsvPreview, String> {
    require_enabled(&app)?;
    let options = ConversionOptions {
        csv_delimiter: delimiter,
        csv_encoding: encoding,
        ..Default::default()
    };
    options.validate()?;
    let source = {
        let state = app.state::<ConversionState>();
        let granted = locked(&state.0.sources)?;
        granted
            .get(&source_id)
            .cloned()
            .ok_or("Conversion source grant expired; select files again")?
    };
    if source.summary.extension != "csv" {
        return Err("Conversion CSV source format required".into());
    }
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<CsvPreview, String> {
        let mut file = crate::file_preview::open_bounded_file(
            source
                .path
                .to_str()
                .ok_or("Conversion invalid source path")?,
            csv::CSV_INPUT_LIMIT,
        )
        .map_err(|_| "Conversion CSV source unavailable or input size limit exceeded")?;
        if files::identity(&file)? != source.identity {
            return Err("Conversion source changed; select files again".into());
        }
        let preview = csv::preview_file(&mut file, &options)?;
        if files::identity(&file)? != source.identity {
            return Err("Conversion source changed; select files again".into());
        }
        Ok(preview)
    })
    .await
    .map_err(|_| "Conversion CSV preview unavailable")??;
    require_enabled(&app)?;
    Ok(result)
}

#[tauri::command]
pub fn start_conversion(
    app: AppHandle,
    source_ids: Vec<String>,
    output_id: Option<String>,
    options: ConversionOptions,
) -> Result<ConversionJob, String> {
    require_enabled(&app)?;
    let state = app.state::<ConversionState>();
    let job = state.submit(source_ids, output_id, options)?;
    let summary = locked(&job.summary)?.clone();
    state.wake(app.clone());
    let _ = app.emit("conversions_changed", ());
    Ok(summary)
}

#[tauri::command]
pub fn retry_conversion(
    app: AppHandle,
    job_id: String,
    source_id: Option<String>,
) -> Result<ConversionJob, String> {
    require_enabled(&app)?;
    let state = app.state::<ConversionState>();
    let job = state.retry(job_id, source_id)?;
    let summary = locked(&job.summary)?.clone();
    state.wake(app.clone());
    let _ = app.emit("conversions_changed", ());
    Ok(summary)
}

#[tauri::command]
pub fn list_conversions(app: AppHandle) -> Result<Vec<ConversionJob>, String> {
    app.state::<ConversionState>().list()
}

#[tauri::command]
pub fn cancel_conversion(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<ConversionState>();
    let jobs = locked(&state.0.jobs)?;
    let job = jobs
        .iter()
        .find(|job| {
            job.summary
                .lock()
                .map(|value| value.id == id)
                .unwrap_or(false)
        })
        .ok_or("Conversion job unavailable")?;
    let mut summary = locked(&job.summary)?;
    if !matches!(summary.status.as_str(), "queued" | "running") {
        return Ok(());
    }
    if summary.status == "queued" {
        summary.status = "cancelled".into();
        for item in &mut summary.items {
            item.status = "cancelled".into();
        }
        summary.completed = summary.total;
    }
    job.cancel.store(true, Ordering::Release);
    drop(summary);
    let _ = app.emit("conversions_changed", ());
    Ok(())
}

#[tauri::command]
pub fn open_conversion_output(app: AppHandle, id: String, reveal: bool) -> Result<(), String> {
    let state = app.state::<ConversionState>();
    let path = locked(&state.0.outputs)?
        .get(&id)
        .cloned()
        .ok_or("Conversion output grant expired")?;
    let file = crate::file_preview::open_bounded_file(
        path.to_str().ok_or("Conversion invalid path")?,
        OUTPUT_LIMIT,
    )
    .map_err(|_| "Conversion output unavailable")?;
    let entry = crate::model::Entry {
        entity_id: id,
        kind: "file".into(),
        title: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        subtitle: String::new(),
        locator: path.to_string_lossy().into_owned(),
        launch_kind: "file".into(),
        aliases: vec![],
        launch_key: String::new(),
    };
    let result = if reveal {
        crate::platform::reveal_entry(&entry)
    } else {
        crate::platform::open_entry(&entry)
    };
    drop(file);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf, sync::atomic::AtomicBool};

    fn fixture() -> PathBuf {
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::current_dir()
            .unwrap()
            .join("../test-results")
            .join(format!("conversion-native-{id}"));
        std::fs::create_dir_all(&path).unwrap();
        path.canonicalize()
            .unwrap()
            .to_string_lossy()
            .strip_prefix(r"\\?\")
            .map(PathBuf::from)
            .unwrap_or(path)
    }

    #[test]
    fn target_matrix_does_not_allow_arbitrary_cross_family_conversion() {
        assert!(supported_target("png", "webp"));
        assert!(supported_target("png", "pdf"));
        assert!(supported_target("pdf", "docx"));
        assert!(!supported_target("xlsx", "pptx"));
        assert!(!supported_target("exe", "pdf"));
    }

    #[test]
    fn options_reject_unbounded_and_ambiguous_page_requests() {
        let mut options = ConversionOptions::default();
        options.target = "png".into();
        options.dpi = 1200;
        assert!(options.validate().is_err());
        options.dpi = 150;
        options.pages = "0,1".into();
        assert!(options.validate().is_err());
        options.pages = "1-3,5".into();
        assert!(options.validate().is_ok());
    }

    #[test]
    fn real_image_roundtrip_preserves_size_and_composites_jpeg_alpha() {
        let root = fixture();
        let input = root.join("透明图片.png");
        image::RgbaImage::from_pixel(23, 11, image::Rgba([255, 0, 0, 0]))
            .save(&input)
            .unwrap();
        let output = root.join("out");
        fs::create_dir(&output).unwrap();
        let mut options = ConversionOptions::default();
        options.target = "jpg".into();
        let paths = images::convert(&input, &output, &options, &AtomicBool::new(false)).unwrap();
        let result = image::open(&paths[0]).unwrap();
        assert_eq!((result.width(), result.height()), (23, 11));
        let pixel = result.to_rgb8().get_pixel(0, 0).0;
        assert!(pixel.iter().all(|value| *value >= 250));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn output_commit_never_overwrites_existing_files() {
        let root = fixture();
        let existing = root.join("报告.png");
        fs::write(&existing, b"original").unwrap();
        let staged = root.join("staged.png");
        fs::write(&staged, b"new output").unwrap();
        let directory = DirectoryGrant::open(&root).unwrap();
        let created =
            commit_output(&staged, &directory, "报告.png", &AtomicBool::new(false)).unwrap();
        assert_ne!(created, existing);
        assert_eq!(fs::read(existing).unwrap(), b"original");
        assert_eq!(fs::read(created).unwrap(), b"new output");
        drop(directory);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cancellation_prevents_creating_an_output() {
        let root = fixture();
        let staged = root.join("staged.txt");
        fs::write(&staged, b"document").unwrap();
        let directory = DirectoryGrant::open(&root).unwrap();
        assert!(commit_output(&staged, &directory, "result.txt", &AtomicBool::new(true)).is_err());
        assert!(!root.join("result.txt").exists());
        drop(directory);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn source_grants_deduplicate_identity_without_holding_user_files_locked() {
        let root = fixture();
        let state = ConversionState::new(root.join("state")).unwrap();
        let first = root.join("image.png");
        image::RgbaImage::from_pixel(2, 2, image::Rgba([1, 2, 3, 255]))
            .save(&first)
            .unwrap();
        let one = state.grant_path(first.clone()).unwrap();
        let again = state.grant_path(first.clone()).unwrap();
        assert_eq!(one.id, again.id);
        let second_dir = root.join("other");
        fs::create_dir(&second_dir).unwrap();
        let second = second_dir.join("image.png");
        fs::copy(&first, &second).unwrap();
        assert_ne!(one.id, state.grant_path(second).unwrap().id);
        // 授权之后允许正常编辑；旧身份随后必须拒绝读取更新后的内容。
        fs::write(&first, b"changed content").unwrap();
        let granted = locked(&state.0.sources)
            .unwrap()
            .get(&one.id)
            .unwrap()
            .clone();
        let staging = root.join("stage");
        fs::create_dir(&staging).unwrap();
        assert!(stage_source(&granted, &staging, &AtomicBool::new(false))
            .unwrap_err()
            .contains("changed"));
        drop(granted);
        drop(state);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn selected_directory_identity_rejects_replacement_without_permanent_locks() {
        let root = fixture();
        let state = ConversionState::new(root.join("state")).unwrap();
        let file = root.join("image.png");
        image::RgbaImage::from_pixel(2, 2, image::Rgba([1, 2, 3, 255]))
            .save(&file)
            .unwrap();
        let source = state.grant_path(file).unwrap();
        let source = locked(&state.0.sources)
            .unwrap()
            .get(&source.id)
            .unwrap()
            .clone();
        let selected = root.join("selected");
        fs::create_dir(&selected).unwrap();
        let record = {
            let opened = DirectoryGrant::open(&selected).unwrap();
            OutputGrant {
                path: opened.path.clone(),
                identity: opened.identity,
            }
        };
        fs::rename(&selected, root.join("moved")).unwrap();
        fs::create_dir(&selected).unwrap();
        assert!(output_directory(&source, Some(&record))
            .err()
            .unwrap()
            .contains("changed"));
        drop(source);
        drop(state);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn pending_queue_rejects_sixth_batch_and_forged_grants() {
        let root = fixture();
        let state = ConversionState::new(root.join("state")).unwrap();
        let source = root.join("image.png");
        image::RgbaImage::from_pixel(2, 2, image::Rgba([1, 2, 3, 255]))
            .save(&source)
            .unwrap();
        let source = state.grant_path(source).unwrap();
        assert!(state
            .submit(vec!["forged".into()], None, ConversionOptions::default())
            .is_err());
        for _ in 0..5 {
            state
                .submit(vec![source.id.clone()], None, ConversionOptions::default())
                .unwrap();
        }
        assert!(state
            .submit(vec![source.id], None, ConversionOptions::default())
            .is_err());
        assert_eq!(state.list().unwrap().len(), 5);
        drop(state);
        fs::remove_dir_all(root).unwrap();
    }

    fn retry_png(state: &ConversionState, root: &Path, name: &str) -> ConversionSource {
        let path = root.join(name);
        image::RgbaImage::from_pixel(9, 7, image::Rgba([25, 50, 100, 255]))
            .save(&path)
            .unwrap();
        state.grant_path(path).unwrap()
    }

    fn retry_output(state: &ConversionState, path: &Path) -> String {
        fs::create_dir(path).unwrap();
        let directory = DirectoryGrant::open(path).unwrap();
        let id = unique_id();
        locked(&state.0.directories).unwrap().insert(
            id.clone(),
            OutputGrant {
                path: directory.path.clone(),
                identity: directory.identity,
            },
        );
        id
    }

    fn fail_retry_job(job: &Job) -> String {
        let mut summary = locked(&job.summary).unwrap();
        summary.status = "failed".into();
        summary.completed = summary.total;
        for item in &mut summary.items {
            item.status = "failed".into();
            item.error = "Conversion temporary failure".into();
        }
        summary.id.clone()
    }

    #[test]
    fn retry_failed_item_uses_original_options_directory_and_collision_policy() {
        let root = fixture();
        let state = ConversionState::new(root.join("state")).unwrap();
        let one = retry_png(&state, &root, "completed.png");
        let two = retry_png(&state, &root, "retry.png");
        let destination = root.join("original-output");
        let output_id = retry_output(&state, &destination);
        let options = ConversionOptions {
            target: "jpg".into(),
            quality: 72,
            dpi: 300,
            pages: "1".into(),
            ..Default::default()
        };
        let original = state
            .submit(
                vec![one.id.clone(), two.id.clone()],
                Some(output_id),
                options.clone(),
            )
            .unwrap();
        let claimed = state.claim_next().unwrap().unwrap();
        state.run(&claimed, &|| {});
        let completed = locked(&original.summary).unwrap().clone();
        assert_eq!(completed.status, "completed");
        let existing_path = completed.items[1].outputs[0].path.clone();
        let existing_bytes = fs::read(&existing_path).unwrap();
        // 模拟单项瞬时失败；已经完成的同批输出必须继续保留。
        {
            let mut summary = locked(&original.summary).unwrap();
            summary.status = "failed".into();
            summary.items[1].status = "failed".into();
            summary.items[1].outputs.clear();
        }
        original.cancel.store(true, Ordering::Release);
        // 草稿授权被清除并改选其他目录，历史重试不应读取这些授权表。
        locked(&state.0.sources).unwrap().clear();
        locked(&state.0.directories).unwrap().clear();
        let draft_directory = root.join("draft-output");
        retry_output(&state, &draft_directory);
        let before_retry =
            serde_json::to_value(locked(&original.summary).unwrap().clone()).unwrap();
        let retry = state
            .retry(completed.id.clone(), Some(two.id.clone()))
            .unwrap();
        assert!(!retry.cancel.load(Ordering::Acquire));
        assert_eq!(retry.sources.len(), 1);
        assert_eq!(retry.sources[0].summary.id, two.id);
        assert_eq!(
            serde_json::to_value(&locked(&retry.summary).unwrap().options).unwrap(),
            serde_json::to_value(options).unwrap()
        );
        let claimed_retry = state.claim_next().unwrap().unwrap();
        state.run(&claimed_retry, &|| {});
        let retried = locked(&retry.summary).unwrap().clone();
        assert_eq!(retried.status, "completed");
        assert_ne!(retried.id, completed.id);
        let output = PathBuf::from(&retried.items[0].outputs[0].path);
        assert_eq!(output.parent().unwrap(), destination);
        assert_ne!(output, PathBuf::from(&existing_path));
        assert_eq!(fs::read(existing_path).unwrap(), existing_bytes);
        let decoded = image::open(output).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (9, 7));
        assert_eq!(fs::read_dir(&draft_directory).unwrap().count(), 0);
        assert_eq!(
            serde_json::to_value(locked(&original.summary).unwrap().clone()).unwrap(),
            before_retry
        );
        drop(state);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn retry_merged_failure_retains_original_order_options_and_all_grants() {
        let root = fixture();
        let state = ConversionState::new(root.join("state")).unwrap();
        let one = retry_png(&state, &root, "one.png");
        let two = retry_png(&state, &root, "two.png");
        let options = ConversionOptions {
            target: "pdf".into(),
            merge: true,
            dpi: 300,
            quality: 65,
            ..Default::default()
        };
        let original = state
            .submit(vec![two.id.clone(), one.id.clone()], None, options.clone())
            .unwrap();
        let id = fail_retry_job(&original);
        assert!(state.retry(id.clone(), Some(two.id.clone())).is_err());
        let retry = state.retry(id, None).unwrap();
        assert_eq!(
            retry
                .sources
                .iter()
                .map(|source| &source.summary.id)
                .collect::<Vec<_>>(),
            vec![&two.id, &one.id]
        );
        for (original, retried) in original.sources.iter().zip(&retry.sources) {
            assert!(Arc::ptr_eq(original, retried));
        }
        assert_eq!(
            serde_json::to_value(&locked(&retry.summary).unwrap().options).unwrap(),
            serde_json::to_value(options).unwrap()
        );
        assert!(retry.output.is_none());
        assert_eq!(locked(&original.summary).unwrap().status, "failed");
        drop(state);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn retry_rejects_nonterminal_success_cancelled_foreign_and_unknown_work() {
        let root = fixture();
        let state = ConversionState::new(root.join("state")).unwrap();
        let source = retry_png(&state, &root, "one.png");
        let foreign = retry_png(&state, &root, "foreign.png");
        let original = state
            .submit(vec![source.id.clone()], None, ConversionOptions::default())
            .unwrap();
        let id = locked(&original.summary).unwrap().id.clone();
        for status in ["queued", "running", "completed", "cancelled"] {
            locked(&original.summary).unwrap().status = status.into();
            assert!(
                state.retry(id.clone(), Some(source.id.clone())).is_err(),
                "{status}"
            );
        }
        fail_retry_job(&original);
        assert!(state
            .retry("unknown".into(), Some(source.id.clone()))
            .is_err());
        assert!(state.retry(id.clone(), Some(foreign.id)).is_err());
        assert!(state.retry(id.clone(), None).is_err());
        locked(&original.summary).unwrap().items[0].status = "completed".into();
        assert!(state.retry(id.clone(), Some(source.id.clone())).is_err());
        locked(&original.summary).unwrap().items[0].status = "running".into();
        assert!(state.retry(id, Some(source.id)).is_err());
        assert_eq!(state.list().unwrap().len(), 1);
        drop(state);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn retry_reuses_validation_and_enforces_pending_queue_capacity() {
        let root = fixture();
        let state = ConversionState::new(root.join("state")).unwrap();
        let source = retry_png(&state, &root, "one.png");
        let original = state
            .submit(vec![source.id.clone()], None, ConversionOptions::default())
            .unwrap();
        let id = fail_retry_job(&original);
        locked(&original.summary).unwrap().options.quality = 0;
        assert!(state
            .retry(id.clone(), Some(source.id.clone()))
            .err()
            .unwrap()
            .contains("invalid options"));
        locked(&original.summary).unwrap().options.quality = 90;
        for _ in 0..5 {
            state
                .submit(vec![source.id.clone()], None, ConversionOptions::default())
                .unwrap();
        }
        assert!(state
            .retry(id, Some(source.id))
            .err()
            .unwrap()
            .contains("queue capacity"));
        assert_eq!(state.list().unwrap().len(), 6);
        drop(state);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn retry_rejects_replaced_source_and_replaced_output_at_execution() {
        for replace_source in [true, false] {
            let root = fixture();
            let state = ConversionState::new(root.join("state")).unwrap();
            let source = retry_png(&state, &root, "one.png");
            let destination = root.join("output");
            let output = retry_output(&state, &destination);
            let original = state
                .submit(
                    vec![source.id.clone()],
                    Some(output),
                    ConversionOptions::default(),
                )
                .unwrap();
            let id = fail_retry_job(&original);
            let retry = state.retry(id, Some(source.id)).unwrap();
            if replace_source {
                fs::rename(root.join("one.png"), root.join("moved.png")).unwrap();
                retry_png(&state, &root, "one.png");
            } else {
                fs::rename(&destination, root.join("moved-output")).unwrap();
                fs::create_dir(&destination).unwrap();
            }
            let claimed = state.claim_next().unwrap().unwrap();
            state.run(&claimed, &|| {});
            let summary = locked(&retry.summary).unwrap().clone();
            assert_eq!(summary.status, "failed");
            assert!(
                summary.items[0].error.contains(if replace_source {
                    "source changed"
                } else {
                    "output directory changed"
                }),
                "{}",
                summary.items[0].error
            );
            assert!(summary.items[0].outputs.is_empty());
            assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
            assert_eq!(locked(&original.summary).unwrap().status, "failed");
            drop(state);
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn retry_csv_keeps_reviewed_encoding_delimiter_and_original_source_identity() {
        let root = fixture();
        let state = ConversionState::new(root.join("state")).unwrap();
        let path = root.join("reviewed.csv");
        fs::write(&path, b"name;value\r\nalpha;42\r\n").unwrap();
        let source = state.grant_path(path.clone()).unwrap();
        let options = ConversionOptions {
            target: "xlsx".into(),
            csv_encoding: "gb18030".into(),
            csv_delimiter: ";".into(),
            ..Default::default()
        };
        let original = state
            .submit(vec![source.id.clone()], None, options.clone())
            .unwrap();
        let id = fail_retry_job(&original);
        locked(&state.0.sources).unwrap().clear();
        let retry = state.retry(id, Some(source.id)).unwrap();
        assert!(Arc::ptr_eq(&original.sources[0], &retry.sources[0]));
        assert_eq!(
            serde_json::to_value(&locked(&retry.summary).unwrap().options).unwrap(),
            serde_json::to_value(options).unwrap()
        );
        let staged = root.join("staged");
        fs::create_dir(&staged).unwrap();
        assert!(stage_source(&retry.sources[0], &staged, &retry.cancel).is_ok());
        fs::rename(&path, root.join("old.csv")).unwrap();
        fs::write(&path, b"unreviewed,new\n1,2\n").unwrap();
        let changed_stage = root.join("changed-stage");
        fs::create_dir(&changed_stage).unwrap();
        assert!(
            stage_source(&retry.sources[0], &changed_stage, &retry.cancel)
                .unwrap_err()
                .contains("source changed")
        );
        drop(state);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn native_queue_executes_real_conversion_then_cleans_staging_and_honors_cancel() {
        let root = fixture();
        let state = ConversionState::new(root.join("state")).unwrap();
        let source_path = root.join("图片.png");
        image::RgbaImage::from_pixel(9, 7, image::Rgba([25, 50, 100, 255]))
            .save(&source_path)
            .unwrap();
        let source_bytes = fs::read(&source_path).unwrap();
        let source = state.grant_path(source_path.clone()).unwrap();
        state
            .submit(
                vec![source.id.clone()],
                None,
                ConversionOptions {
                    target: "webp".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        let job = state.claim_next().unwrap().unwrap();
        let events = std::sync::atomic::AtomicUsize::new(0);
        state.run(&job, &|| {
            events.fetch_add(1, Ordering::Relaxed);
        });
        let summary = locked(&job.summary).unwrap().clone();
        assert_eq!(summary.status, "completed");
        assert_eq!(summary.completed, 1);
        assert_eq!(summary.items[0].status, "completed");
        assert_eq!(summary.items[0].outputs.len(), 1);
        let output = &summary.items[0].outputs[0];
        let decoded = image::open(&output.path).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (9, 7));
        assert_eq!(fs::read(&source_path).unwrap(), source_bytes);
        assert!(!state
            .0
            .root
            .join("conversion-jobs")
            .join(&summary.id)
            .exists());
        assert!(events.load(Ordering::Relaxed) >= 3);
        // 完成后源和输出目录都不残留删除共享锁。
        let selected = root.join("select");
        fs::create_dir(&selected).unwrap();
        let record = {
            let held = DirectoryGrant::open(&selected).unwrap();
            OutputGrant {
                path: selected.clone(),
                identity: held.identity,
            }
        };
        locked(&state.0.directories)
            .unwrap()
            .insert("selected".into(), record);
        let cancelled = state
            .submit(
                vec![source.id],
                Some("selected".into()),
                ConversionOptions {
                    target: "jpg".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        cancelled.cancel.store(true, Ordering::Release);
        let next = state.claim_next().unwrap().unwrap();
        state.run(&next, &|| {});
        assert_eq!(locked(&cancelled.summary).unwrap().status, "cancelled");
        assert!(!selected.join("图片.jpg").exists());
        fs::rename(&selected, root.join("moved-select")).unwrap();
        fs::write(source_path, b"source can be edited").unwrap();
        drop(job);
        drop(next);
        drop(cancelled);
        drop(state);
        fs::remove_dir_all(root).unwrap();
    }
}
