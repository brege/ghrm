use crate::server::{AppState, Mode};

use axum::{
    body::Body,
    extract::{Path as AxPath, State},
    http::{StatusCode, header},
    response::Response,
};
use std::path::{Component, Path, PathBuf};
use tracing::warn;

#[derive(Clone, Copy)]
pub(crate) struct FileView {
    pub(crate) kind: &'static str,
    pub(crate) preview_hidden: bool,
    pub(crate) raw_hidden: bool,
}

impl FileView {
    pub(crate) fn markdown() -> Self {
        Self {
            kind: "markdown",
            preview_hidden: false,
            raw_hidden: true,
        }
    }

    pub(crate) fn raw() -> Self {
        Self {
            kind: "raw",
            preview_hidden: true,
            raw_hidden: false,
        }
    }
}

pub(crate) async fn theme_asset(AxPath(path): AxPath<String>) -> Response {
    let base = match crate::theme::dir() {
        Ok(d) => d,
        Err(e) => {
            warn!("theme dir error: {}", e);
            return not_found();
        }
    };
    let rel = path.trim_start_matches('/');
    for comp in PathBuf::from(rel).components() {
        if !matches!(comp, Component::Normal(_)) {
            return not_found();
        }
    }
    stream_file(&base.join(rel)).await
}

pub(crate) async fn vendor(AxPath(path): AxPath<String>) -> Response {
    let path = match crate::vendor::path(&path) {
        Ok(p) => p,
        Err(_) => return not_found(),
    };
    stream_file(&path).await
}

pub(crate) async fn raw_file(State(s): State<AppState>, AxPath(path): AxPath<String>) -> Response {
    let Some(path) = resolve_internal_file(&s, &path) else {
        return not_found();
    };
    stream_export(&path, false).await
}

pub(crate) async fn html_file(State(s): State<AppState>, AxPath(path): AxPath<String>) -> Response {
    let Some(path) = resolve_internal_file(&s, &path) else {
        return not_found();
    };
    let bytes = match tokio::fs::read(&path).await {
        Ok(b) => b,
        Err(_) => return not_found(),
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime_guess(&path))
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from(bytes))
        .unwrap()
}

pub(crate) async fn download_file(
    State(s): State<AppState>,
    AxPath(path): AxPath<String>,
) -> Response {
    let Some(path) = resolve_internal_file(&s, &path) else {
        return not_found();
    };
    stream_export(&path, true).await
}

pub(crate) fn is_binary_ext(ext: &str) -> bool {
    matches!(
        ext,
        "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "svg"
            | "webp"
            | "ico"
            | "bmp"
            | "tiff"
            | "tif"
            | "pdf"
            | "woff"
            | "woff2"
            | "ttf"
            | "otf"
            | "eot"
            | "zip"
            | "gz"
            | "tar"
            | "bz2"
            | "xz"
            | "7z"
            | "rar"
            | "zst"
            | "exe"
            | "bin"
            | "so"
            | "dylib"
            | "dll"
            | "o"
            | "a"
            | "lib"
            | "mp3"
            | "mp4"
            | "wav"
            | "ogg"
            | "flac"
            | "mkv"
            | "avi"
            | "mov"
            | "webm"
            | "sqlite"
            | "db"
            | "sqlite3"
            | "class"
            | "jar"
            | "pyc"
    )
}

pub(crate) fn stream_bytes(path: &Path, bytes: Vec<u8>) -> Response {
    let mime = mime_guess(path);
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from(bytes))
        .unwrap()
}

pub(crate) async fn stream_file(path: &Path) -> Response {
    let bytes = match tokio::fs::read(path).await {
        Ok(b) => b,
        Err(_) => return not_found(),
    };
    stream_bytes(path, bytes)
}

fn mime_guess(path: &Path) -> &'static str {
    match path.extension().and_then(|s| s.to_str()) {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("css") => "text/css; charset=utf-8",
        Some("js") | Some("mjs") => "application/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("html") => "text/html; charset=utf-8",
        _ => "application/octet-stream",
    }
}

pub(crate) fn raw_blob_html(text: &str, lang: Option<&str>) -> String {
    let attrs = lang
        .map(|lang| {
            format!(
                r#" class="language-{lang}" data-lang="{lang}""#,
                lang = html_escape::encode_double_quoted_attribute(lang),
            )
        })
        .unwrap_or_default();
    format!(
        "<div class=\"ghrm-blob\">{}<div class=\"highlight ghrm-blob-source\" hidden><pre tabindex=\"0\" class=\"chroma\"><code{attrs}>{body}</code></pre></div><table class=\"ghrm-blob-table\" role=\"presentation\"><tbody></tbody></table></div>",
        raw_source_html(text),
        attrs = attrs,
        body = html_escape::encode_text(text),
    )
}

fn raw_source_html(text: &str) -> String {
    format!(
        "<template class=\"ghrm-data\">{}</template>",
        html_escape::encode_text(text),
    )
}

pub(crate) fn file_view_attrs(rel: &str, view: FileView) -> String {
    format!(
        "data-ghrm-view-kind=\"{kind}\" data-ghrm-raw-url=\"{raw}\" data-ghrm-download-url=\"{download}\"",
        kind = view.kind,
        raw = html_escape::encode_double_quoted_attribute(&internal_file_href("raw", rel)),
        download =
            html_escape::encode_double_quoted_attribute(&internal_file_href("download", rel)),
    )
}

fn internal_file_href(kind: &str, rel: &str) -> String {
    format!("/_ghrm/{kind}/{}", rel.trim_matches('/'))
}

fn resolve_internal_file(s: &AppState, rel: &str) -> Option<PathBuf> {
    let clean = rel.trim_matches('/');
    if clean.is_empty() {
        return None;
    }

    let rel_path = Path::new(clean);
    for comp in rel_path.components() {
        if !matches!(comp, Component::Normal(_)) {
            return None;
        }
    }

    let base = if s.mode == Mode::File {
        s.target.parent().unwrap_or(s.target.as_path())
    } else {
        s.target.as_path()
    };
    let path = base.join(rel_path);
    if path.is_file() { Some(path) } else { None }
}

async fn stream_export(path: &Path, attachment: bool) -> Response {
    let bytes = match tokio::fs::read(path).await {
        Ok(b) => b,
        Err(_) => return not_found(),
    };

    let mut res = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, export_mime(path, &bytes))
        .header(header::CACHE_CONTROL, "no-cache");
    if attachment {
        res = res.header(header::CONTENT_DISPOSITION, content_disposition(path));
    }
    res.body(Body::from(bytes)).unwrap()
}

fn export_mime(path: &Path, bytes: &[u8]) -> &'static str {
    let is_binary = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|ext| is_binary_ext(&ext.to_lowercase()))
        .unwrap_or(false)
        || bytes.contains(&0);
    if is_binary {
        mime_guess(path)
    } else {
        "text/plain; charset=utf-8"
    }
}

fn content_disposition(path: &Path) -> String {
    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    format!("attachment; filename=\"{filename}\"")
}

fn not_found() -> Response {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from("404"))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_disposition_escapes_quotes() {
        let value = content_disposition(Path::new("odd\"name.md"));
        assert_eq!(value, "attachment; filename=\"odd\\\"name.md\"");
    }

    #[test]
    fn export_mime_prefers_text_plain_for_text_files() {
        assert_eq!(
            export_mime(Path::new("README.md"), b"# hello\n"),
            "text/plain; charset=utf-8",
        );
    }

    #[test]
    fn raw_blob_includes_hidden_source_block() {
        let html = raw_blob_html("fn main() {}\n", Some("rust"));
        assert!(html.contains("ghrm-blob-source"));
        assert!(html.contains(r#"class="language-rust""#));
        assert!(html.contains("<tbody></tbody>"));
    }
}
