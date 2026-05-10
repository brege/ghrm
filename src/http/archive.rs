use crate::explorer::view::{self, ViewQuery};
use crate::explorer::walk;
use crate::http::delivery;
use crate::http::server::{AppState, Mode};
use crate::{dirs, paths};

use anyhow::{Context, Result};
use axum::{
    Json,
    body::Body,
    extract::{Path as AxPath, Query, RawQuery, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use zip::write::SimpleFileOptions;

const JOB_TTL: Duration = Duration::from_secs(30 * 60);

#[derive(Deserialize)]
pub(crate) struct ArchivePath {
    format: String,
    path: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArchiveFormat {
    Zip,
    TarZst,
}

#[derive(Clone, Debug)]
struct ArchiveEntry {
    source: PathBuf,
    archive_path: PathBuf,
}

#[derive(Clone)]
pub(crate) struct ArchiveJobs {
    dir: Arc<PathBuf>,
    jobs: Arc<Mutex<BTreeMap<String, ArchiveJob>>>,
    next: Arc<AtomicU64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JobState {
    Running,
    Ready,
    Failed,
}

#[derive(Clone, Debug)]
struct ArchiveJob {
    dir: PathBuf,
    path: PathBuf,
    filename: String,
    content_type: &'static str,
    format: ArchiveFormat,
    state: JobState,
    done_files: usize,
    total_files: usize,
    done_bytes: u64,
    total_bytes: u64,
    updated: Instant,
    error: Option<String>,
}

#[derive(Serialize)]
struct StartResponse {
    id: String,
    status_url: String,
    download_url: String,
}

#[derive(Serialize)]
pub(crate) struct JobStatus {
    id: String,
    state: &'static str,
    format: &'static str,
    filename: String,
    done_files: usize,
    total_files: usize,
    done_bytes: u64,
    total_bytes: u64,
    percent: u8,
    download_url: Option<String>,
    error: Option<String>,
}

struct JobFile {
    path: PathBuf,
    filename: String,
    content_type: &'static str,
}

struct ProgressReader {
    file: fs::File,
    jobs: ArchiveJobs,
    id: String,
}

pub(crate) async fn start(
    State(s): State<AppState>,
    AxPath(path): AxPath<ArchivePath>,
    RawQuery(raw_query): RawQuery,
    Query(q): Query<ViewQuery>,
) -> Response {
    let Some(format) = ArchiveFormat::parse(&path.format) else {
        return not_found();
    };
    let Some(rel) = archive_rel(path.path.as_deref()) else {
        return not_found();
    };
    if s.mode != Mode::Dir {
        return not_found();
    }

    let source_dir = s.target.join(&rel);
    if !source_dir.is_dir() {
        return not_found();
    }

    let view = view::from_query(&q, raw_query.as_deref(), &s.view_cfg, &s.filters);
    let matcher = view::matcher(&view, &s.filters);
    let tree = s.nav_tree(&view, matcher.as_ref());
    let Some(files) = visible_files(&tree, &rel) else {
        return not_found();
    };

    let root_name = archive_root_name(&s.target, &rel);
    let entries = archive_entries(&s.target, &rel, &root_name, &files);
    let filename = archive_filename(&root_name, format);

    match s
        .archive_jobs
        .start(format, source_dir, root_name, filename, entries)
    {
        Ok(job) => Json(job).into_response(),
        Err(_) => server_error(),
    }
}

pub(crate) async fn status(State(s): State<AppState>, AxPath(id): AxPath<String>) -> Response {
    match s.archive_jobs.status(&id) {
        Some(status) => Json(status).into_response(),
        None => not_found(),
    }
}

pub(crate) async fn download(State(s): State<AppState>, AxPath(id): AxPath<String>) -> Response {
    let Some(file) = s.archive_jobs.file(&id) else {
        return not_found();
    };
    let mut res = delivery::stream_file(&file.path).await;
    if res.status().is_success() {
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(file.content_type),
        );
        res.headers_mut().insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&content_disposition(&file.filename)).unwrap(),
        );
    }
    res
}

impl ArchiveFormat {
    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "zip" => Some(Self::Zip),
            "tar.zst" => Some(Self::TarZst),
            _ => None,
        }
    }

    fn content_type(self) -> &'static str {
        match self {
            Self::Zip => "application/zip",
            Self::TarZst => "application/zstd",
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::Zip => "zip",
            Self::TarZst => "tar.zst",
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Zip => "zip",
            Self::TarZst => "tar.zst",
        }
    }
}

impl JobState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Ready => "ready",
            Self::Failed => "failed",
        }
    }
}

impl ArchiveJobs {
    pub(crate) fn new() -> Result<Self> {
        Self::new_in(dirs::cache()?.join("archives"))
    }

    fn new_in(base: PathBuf) -> Result<Self> {
        let server_dir = base.join(server_id());
        fs::create_dir_all(&server_dir)?;
        Ok(Self {
            dir: Arc::new(server_dir),
            jobs: Arc::new(Mutex::new(BTreeMap::new())),
            next: Arc::new(AtomicU64::new(1)),
        })
    }

    fn start(
        &self,
        format: ArchiveFormat,
        source_dir: PathBuf,
        root_name: String,
        filename: String,
        entries: Vec<ArchiveEntry>,
    ) -> Result<StartResponse> {
        self.cleanup_expired();
        let id = self.next_id();
        let job_dir = self.dir.join(&id);
        fs::create_dir_all(&job_dir)?;
        let output = job_dir.join(&filename);
        let partial = job_dir.join("archive.part");
        let total_bytes = archive_total_bytes(&entries);
        let job = ArchiveJob {
            dir: job_dir,
            path: output.clone(),
            filename: filename.clone(),
            content_type: format.content_type(),
            format,
            state: JobState::Running,
            done_files: 0,
            total_files: entries.len(),
            done_bytes: 0,
            total_bytes,
            updated: Instant::now(),
            error: None,
        };
        self.jobs.lock().unwrap().insert(id.clone(), job);

        let jobs = self.clone();
        let job_id = id.clone();
        tokio::task::spawn_blocking(move || {
            let result = write_archive_file(
                format,
                &source_dir,
                &root_name,
                &entries,
                &partial,
                &jobs,
                &job_id,
            )
            .and_then(|()| {
                fs::rename(&partial, &output)
                    .with_context(|| format!("move archive into place {}", output.display()))
            });
            match result {
                Ok(()) => jobs.mark_ready(&job_id),
                Err(err) => {
                    let _ = fs::remove_file(&partial);
                    jobs.mark_failed(&job_id, err.to_string());
                }
            }
        });

        Ok(StartResponse {
            id: id.clone(),
            status_url: status_url(&id),
            download_url: download_url(&id),
        })
    }

    fn status(&self, id: &str) -> Option<JobStatus> {
        self.cleanup_expired();
        self.jobs
            .lock()
            .unwrap()
            .get(id)
            .map(|job| job_status(id, job))
    }

    fn file(&self, id: &str) -> Option<JobFile> {
        self.cleanup_expired();
        self.jobs.lock().unwrap().get(id).and_then(|job| {
            (job.state == JobState::Ready).then(|| JobFile {
                path: job.path.clone(),
                filename: job.filename.clone(),
                content_type: job.content_type,
            })
        })
    }

    fn add_bytes(&self, id: &str, bytes: u64) {
        let mut guard = self.jobs.lock().unwrap();
        let Some(job) = guard.get_mut(id) else {
            return;
        };
        if job.state != JobState::Running {
            return;
        }
        job.done_bytes = job.done_bytes.saturating_add(bytes).min(job.total_bytes);
        job.updated = Instant::now();
    }

    fn finish_file(&self, id: &str) {
        let mut guard = self.jobs.lock().unwrap();
        let Some(job) = guard.get_mut(id) else {
            return;
        };
        if job.state != JobState::Running {
            return;
        }
        job.done_files = job.done_files.saturating_add(1).min(job.total_files);
        job.updated = Instant::now();
    }

    fn mark_ready(&self, id: &str) {
        let mut guard = self.jobs.lock().unwrap();
        let Some(job) = guard.get_mut(id) else {
            return;
        };
        job.state = JobState::Ready;
        job.done_files = job.total_files;
        job.done_bytes = job.total_bytes;
        job.updated = Instant::now();
    }

    fn mark_failed(&self, id: &str, error: String) {
        let mut guard = self.jobs.lock().unwrap();
        let Some(job) = guard.get_mut(id) else {
            return;
        };
        job.state = JobState::Failed;
        job.error = Some(error);
        job.updated = Instant::now();
    }

    fn next_id(&self) -> String {
        let seq = self.next.fetch_add(1, Ordering::Relaxed);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        format!("{now:x}-{seq:x}")
    }

    fn cleanup_expired(&self) {
        let mut expired = Vec::new();
        {
            let mut guard = self.jobs.lock().unwrap();
            guard.retain(|_, job| {
                let stale = matches!(job.state, JobState::Ready | JobState::Failed)
                    && job.updated.elapsed() > JOB_TTL;
                if stale {
                    expired.push(job.dir.clone());
                }
                !stale
            });
        }
        for dir in expired {
            let _ = fs::remove_dir_all(dir);
        }
    }
}

impl Read for ProgressReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.file.read(buf)?;
        if read > 0 {
            self.jobs.add_bytes(&self.id, read as u64);
        }
        Ok(read)
    }
}

fn archive_rel(raw: Option<&str>) -> Option<PathBuf> {
    match raw {
        Some(raw) if !raw.trim_matches('/').is_empty() => paths::safe_rel(raw),
        Some(_) | None => Some(PathBuf::new()),
    }
}

fn visible_files(tree: &walk::NavTree, rel: &Path) -> Option<Vec<PathBuf>> {
    let start_key = path_key(rel);
    tree.dirs.get(&start_key)?;

    let mut out = Vec::new();
    let mut pending = vec![rel.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let key = path_key(&dir);
        let nav_dir = tree.dirs.get(&key)?;
        for entry in &nav_dir.entries {
            let child = dir.join(&entry.name);
            if entry.is_dir {
                pending.push(child);
            } else {
                out.push(child);
            }
        }
    }
    out.sort_by_key(|path| path_key(path));
    Some(out)
}

fn archive_entries(
    root: &Path,
    rel: &Path,
    root_name: &str,
    files: &[PathBuf],
) -> Vec<ArchiveEntry> {
    files
        .iter()
        .filter_map(|file| {
            let inner = file.strip_prefix(rel).ok()?;
            Some(ArchiveEntry {
                source: root.join(file),
                archive_path: Path::new(root_name).join(inner),
            })
        })
        .collect()
}

fn write_archive_file(
    format: ArchiveFormat,
    source_dir: &Path,
    root_name: &str,
    entries: &[ArchiveEntry],
    output: &Path,
    jobs: &ArchiveJobs,
    id: &str,
) -> Result<()> {
    match format {
        ArchiveFormat::Zip => write_zip(output, root_name, entries, jobs, id),
        ArchiveFormat::TarZst => write_tar_zst(output, source_dir, root_name, entries, jobs, id),
    }
}

fn write_zip(
    output: &Path,
    root_name: &str,
    entries: &[ArchiveEntry],
    jobs: &ArchiveJobs,
    id: &str,
) -> Result<()> {
    let file = fs::File::create(output)
        .with_context(|| format!("create archive output {}", output.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);
    let dir_options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .unix_permissions(0o755);

    zip.add_directory(format!("{root_name}/"), dir_options)?;
    for entry in entries {
        let archive_path = zip_path(&entry.archive_path)?;
        zip.start_file(archive_path, options)
            .with_context(|| format!("start archive entry {}", entry.archive_path.display()))?;
        let file = fs::File::open(&entry.source)
            .with_context(|| format!("open archive source {}", entry.source.display()))?;
        let mut reader = ProgressReader {
            file,
            jobs: jobs.clone(),
            id: id.to_string(),
        };
        io::copy(&mut reader, &mut zip)
            .with_context(|| format!("write archive source {}", entry.source.display()))?;
        jobs.finish_file(id);
    }

    zip.finish()?;
    Ok(())
}

fn write_tar_zst(
    output: &Path,
    source_dir: &Path,
    root_name: &str,
    entries: &[ArchiveEntry],
    jobs: &ArchiveJobs,
    id: &str,
) -> Result<()> {
    let file = fs::File::create(output)
        .with_context(|| format!("create archive output {}", output.display()))?;
    let encoder = zstd::stream::Encoder::new(file, 0)?;
    let mut tar = tar::Builder::new(encoder);
    tar.append_dir(root_name, source_dir)?;
    for entry in entries {
        let file = fs::File::open(&entry.source)
            .with_context(|| format!("open archive source {}", entry.source.display()))?;
        let metadata = file
            .metadata()
            .with_context(|| format!("stat archive source {}", entry.source.display()))?;
        let mut header = tar::Header::new_gnu();
        header.set_metadata(&metadata);
        let reader = ProgressReader {
            file,
            jobs: jobs.clone(),
            id: id.to_string(),
        };
        tar.append_data(&mut header, &entry.archive_path, reader)
            .with_context(|| format!("add archive source {}", entry.source.display()))?;
        jobs.finish_file(id);
    }
    let encoder = tar.into_inner()?;
    encoder.finish()?;
    Ok(())
}

fn archive_root_name(root: &Path, rel: &Path) -> String {
    rel.file_name()
        .or_else(|| root.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "archive".to_string())
}

fn archive_filename(root_name: &str, format: ArchiveFormat) -> String {
    format!(
        "{}.{}",
        root_name.replace(['\\', '/', '"'], "_"),
        format.extension()
    )
}

fn archive_total_bytes(entries: &[ArchiveEntry]) -> u64 {
    entries
        .iter()
        .filter_map(|entry| fs::metadata(&entry.source).ok())
        .map(|metadata| metadata.len())
        .sum()
}

fn job_status(id: &str, job: &ArchiveJob) -> JobStatus {
    JobStatus {
        id: id.to_string(),
        state: job.state.as_str(),
        format: job.format.as_str(),
        filename: job.filename.clone(),
        done_files: job.done_files,
        total_files: job.total_files,
        done_bytes: job.done_bytes,
        total_bytes: job.total_bytes,
        percent: progress_percent(job),
        download_url: (job.state == JobState::Ready).then(|| download_url(id)),
        error: job.error.clone(),
    }
}

fn progress_percent(job: &ArchiveJob) -> u8 {
    if job.state == JobState::Ready {
        return 100;
    }
    let byte_percent = job
        .done_bytes
        .saturating_mul(100)
        .checked_div(job.total_bytes);
    let file_percent = (job.done_files as u64)
        .saturating_mul(100)
        .checked_div(job.total_files as u64);
    byte_percent
        .or(file_percent)
        .unwrap_or(0)
        .min(99)
        .try_into()
        .unwrap_or(99)
}

fn status_url(id: &str) -> String {
    format!("/_ghrm/archive-jobs/{id}")
}

fn download_url(id: &str) -> String {
    format!("/_ghrm/archive-jobs/{id}/download")
}

fn server_id() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{}-{now:x}", std::process::id())
}

fn zip_path(path: &Path) -> Result<String> {
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        anyhow::bail!("invalid archive path");
    }
    let path = path_key(path);
    if path.is_empty() {
        anyhow::bail!("invalid archive path");
    }
    Ok(path)
}

fn path_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn content_disposition(filename: &str) -> String {
    let filename = filename.replace('\\', "\\\\").replace('"', "\\\"");
    format!("attachment; filename=\"{filename}\"")
}

fn not_found() -> Response {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from("404"))
        .unwrap()
}

fn server_error() -> Response {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from("500"))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;
    use std::fs;
    use zip::ZipArchive;

    #[test]
    fn visible_files_follow_explorer_filters() {
        let td = TempDir::new("ghrm-archive-visible");
        fs::create_dir_all(td.path().join("docs/.draft")).unwrap();
        fs::write(td.path().join("docs/guide.md"), "guide").unwrap();
        fs::write(td.path().join("docs/image.png"), "image").unwrap();
        fs::write(td.path().join("docs/.draft/notes.md"), "notes").unwrap();

        let nav = walk::build_all(td.path(), true, &[], &["md".to_string()], false);
        let tree = nav.get(
            walk::ViewOpts {
                show_hidden: false,
                show_excludes: false,
                filter_ext: true,
            },
            walk::Sort::Name,
            walk::SortDir::Asc,
            None,
            false,
        );

        let files = visible_files(&tree, Path::new("docs")).unwrap();

        assert_eq!(files, vec![PathBuf::from("docs/guide.md")]);
    }

    #[test]
    fn zip_archive_wraps_files_in_root_dir() {
        let td = TempDir::new("ghrm-archive-zip");
        let source = td.path().join("docs/guide.md");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, "guide").unwrap();
        let jobs = ArchiveJobs::new_in(td.path().join("jobs")).unwrap();
        let output = td.path().join("docs.zip");
        let entries = vec![ArchiveEntry {
            source,
            archive_path: PathBuf::from("docs/guide.md"),
        }];

        write_zip(&output, "docs", &entries, &jobs, "test").unwrap();
        let mut archive = ZipArchive::new(fs::File::open(output).unwrap()).unwrap();

        assert!(archive.by_name("docs/").is_ok());
        assert!(archive.by_name("docs/guide.md").is_ok());
    }

    #[tokio::test]
    async fn archive_job_status_reports_progress() {
        let td = TempDir::new("ghrm-archive-jobs");
        let jobs = ArchiveJobs::new_in(td.path().join("jobs")).unwrap();
        let source = td.path().join("guide.md");
        fs::write(&source, "guide").unwrap();
        let entries = vec![ArchiveEntry {
            source,
            archive_path: PathBuf::from("docs/guide.md"),
        }];
        let start = jobs
            .start(
                ArchiveFormat::Zip,
                td.path().to_path_buf(),
                "docs".to_string(),
                "docs.zip".to_string(),
                entries,
            )
            .unwrap();

        let status = jobs.status(&start.id).unwrap();

        assert_eq!(status.filename, "docs.zip");
        assert_eq!(status.total_files, 1);
        assert!(matches!(status.state, "running" | "ready"));
    }

    #[test]
    fn archive_filename_uses_format_extension() {
        assert_eq!(
            archive_filename("docs", ArchiveFormat::TarZst),
            "docs.tar.zst"
        );
        assert_eq!(archive_filename("docs", ArchiveFormat::Zip), "docs.zip");
    }
}
