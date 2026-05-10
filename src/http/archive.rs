use crate::explorer::view::{self, ViewQuery};
use crate::explorer::walk;
use crate::http::server::{AppState, Mode};
use crate::paths;

use anyhow::{Context, Result};
use axum::{
    body::Body,
    extract::{Path as AxPath, Query, RawQuery, State},
    http::{StatusCode, header},
    response::Response,
};
use serde::Deserialize;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;

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

pub(crate) async fn download(
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
    let content_type = format.content_type();

    let archive = tokio::task::spawn_blocking(move || {
        write_archive(format, &source_dir, &root_name, &entries)
    })
    .await
    .ok()
    .and_then(Result::ok);

    match archive {
        Some(bytes) => attachment_response(bytes, content_type, &filename),
        None => server_error(),
    }
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

fn write_archive(
    format: ArchiveFormat,
    source_dir: &Path,
    root_name: &str,
    entries: &[ArchiveEntry],
) -> Result<Vec<u8>> {
    match format {
        ArchiveFormat::Zip => write_zip(root_name, entries),
        ArchiveFormat::TarZst => write_tar_zst(source_dir, root_name, entries),
    }
}

fn write_zip(root_name: &str, entries: &[ArchiveEntry]) -> Result<Vec<u8>> {
    let cursor = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(cursor);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);
    let dir_options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .unix_permissions(0o755);

    zip.add_directory(format!("{root_name}/"), dir_options)?;
    for entry in entries {
        let archive_path = zip_path(&entry.archive_path)?;
        zip.start_file(archive_path, options)?;
        let mut file = std::fs::File::open(&entry.source)
            .with_context(|| format!("open archive source {}", entry.source.display()))?;
        std::io::copy(&mut file, &mut zip)?;
    }

    Ok(zip.finish()?.into_inner())
}

fn write_tar_zst(source_dir: &Path, root_name: &str, entries: &[ArchiveEntry]) -> Result<Vec<u8>> {
    let encoder = zstd::stream::Encoder::new(Vec::new(), 0)?;
    let mut tar = tar::Builder::new(encoder);
    tar.append_dir(root_name, source_dir)?;
    for entry in entries {
        tar.append_path_with_name(&entry.source, &entry.archive_path)
            .with_context(|| format!("add archive source {}", entry.source.display()))?;
    }
    let encoder = tar.into_inner()?;
    Ok(encoder.finish()?)
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

fn attachment_response(bytes: Vec<u8>, content_type: &str, filename: &str) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::CONTENT_DISPOSITION, content_disposition(filename))
        .body(Body::from(bytes))
        .unwrap()
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
        let entries = vec![ArchiveEntry {
            source,
            archive_path: PathBuf::from("docs/guide.md"),
        }];

        let bytes = write_zip("docs", &entries).unwrap();
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();

        assert!(archive.by_name("docs/").is_ok());
        assert!(archive.by_name("docs/guide.md").is_ok());
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
