use crate::paths;
use crate::server::{AppState, Mode};

use axum::{
    body::Body,
    extract::{Path as AxPath, State},
    http::{HeaderValue, Request, StatusCode, header},
    response::{IntoResponse, Response},
};
use std::io::Read;
use std::path::{Path, PathBuf};
use tokio::io::AsyncReadExt;
use tower_http::services::ServeFile;
use tracing::warn;

const PEEK_BYTES: usize = 8192;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FileMode {
    Markdown,
    Source,
    Dual,
    Native,
    Download,
}

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

    pub(crate) fn source() -> Self {
        Self {
            kind: "source",
            preview_hidden: true,
            raw_hidden: false,
        }
    }

    pub(crate) fn dual() -> Self {
        Self {
            kind: "dual",
            preview_hidden: false,
            raw_hidden: true,
        }
    }
}

pub(crate) fn file_mode(path: &Path, bytes: &[u8]) -> FileMode {
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    if ext.eq_ignore_ascii_case("md") {
        return FileMode::Markdown;
    }
    if ext.eq_ignore_ascii_case("svg") && is_text_content(bytes) {
        return FileMode::Dual;
    }
    if let Some(kind) = infer::get(bytes) {
        return match kind.matcher_type() {
            infer::MatcherType::Image | infer::MatcherType::Video | infer::MatcherType::Audio => {
                FileMode::Native
            }
            infer::MatcherType::App if is_pdf(kind.mime_type()) => FileMode::Native,
            infer::MatcherType::Text => FileMode::Source,
            _ => FileMode::Download,
        };
    }
    if is_text_content(bytes) {
        FileMode::Source
    } else {
        FileMode::Download
    }
}

pub(crate) fn file_mode_sync(path: &Path) -> FileMode {
    let Ok(mut file) = std::fs::File::open(path) else {
        return FileMode::Download;
    };
    let mut bytes = [0; PEEK_BYTES];
    let Ok(n) = file.read(&mut bytes) else {
        return FileMode::Download;
    };
    file_mode(path, &bytes[..n])
}

pub(crate) async fn file_mode_async(path: &Path) -> FileMode {
    let Ok(mut file) = tokio::fs::File::open(path).await else {
        return FileMode::Download;
    };
    let mut bytes = vec![0; PEEK_BYTES];
    let Ok(n) = file.read(&mut bytes).await else {
        return FileMode::Download;
    };
    bytes.truncate(n);
    file_mode(path, &bytes)
}

fn is_text_content(bytes: &[u8]) -> bool {
    infer::get(bytes).is_none_or(|kind| kind.matcher_type() == infer::MatcherType::Text)
        && content_inspector::inspect(bytes).is_text()
}

fn is_pdf(mime: &str) -> bool {
    mime == "application/pdf"
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
    let Some(rel) = paths::safe_rel(rel) else {
        return not_found();
    };
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
    if previews_text(&path).await {
        stream_text_file(&path).await
    } else {
        stream_file(&path).await
    }
}

pub(crate) async fn html_file(State(s): State<AppState>, AxPath(path): AxPath<String>) -> Response {
    let Some(path) = resolve_internal_file(&s, &path) else {
        return not_found();
    };
    stream_file(&path).await
}

pub(crate) async fn download_file(
    State(s): State<AppState>,
    AxPath(path): AxPath<String>,
) -> Response {
    let Some(path) = resolve_internal_file(&s, &path) else {
        return not_found();
    };
    stream_download(&path).await
}

pub(crate) async fn stream_file(path: &Path) -> Response {
    serve_file(ServeFile::new(path)).await
}

pub(crate) async fn stream_download(path: &Path) -> Response {
    let mut res = stream_file(path).await;
    if res.status().is_success() {
        res.headers_mut().insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&content_disposition(path)).unwrap(),
        );
    }
    res
}

async fn stream_text_file(path: &Path) -> Response {
    let mut res = stream_file(path).await;
    if res.status().is_success() {
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        );
    }
    res
}

async fn serve_file(mut file: ServeFile) -> Response {
    match file.try_call(Request::new(Body::empty())).await {
        Ok(res) => {
            let mut res = res.map(Body::new).into_response();
            res.headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
            res
        }
        Err(_) => not_found(),
    }
}

pub(crate) async fn previews_text(path: &Path) -> bool {
    matches!(
        file_mode_async(path).await,
        FileMode::Markdown | FileMode::Source | FileMode::Dual
    )
}

pub(crate) fn previews_text_sync(path: &Path) -> bool {
    matches!(
        file_mode_sync(path),
        FileMode::Markdown | FileMode::Source | FileMode::Dual
    )
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
    let base = if s.mode == Mode::File {
        s.target.parent().unwrap_or(s.target.as_path())
    } else {
        s.target.as_path()
    };
    paths::resolve_file(base, rel)
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
    fn file_mode_markdown_by_extension() {
        assert_eq!(
            file_mode(Path::new("README.md"), b"# Title\n"),
            FileMode::Markdown
        );
        assert_eq!(file_mode(Path::new("DOC.MD"), b"text"), FileMode::Markdown);
    }

    #[test]
    fn file_mode_svg_is_dual() {
        let svg = b"<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>";
        assert_eq!(file_mode(Path::new("icon.svg"), svg), FileMode::Dual);
        assert_eq!(file_mode(Path::new("LOGO.SVG"), svg), FileMode::Dual);
    }

    #[test]
    fn file_mode_text_is_source() {
        assert_eq!(
            file_mode(Path::new("main.rs"), b"fn main() {}\n"),
            FileMode::Source
        );
        assert_eq!(
            file_mode(Path::new("script.py"), b"print('hi')\n"),
            FileMode::Source
        );
    }

    #[test]
    fn file_mode_png_is_native() {
        let png = &[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
        assert_eq!(file_mode(Path::new("image.png"), png), FileMode::Native);
    }

    #[test]
    fn file_mode_zstd_is_download() {
        let zstd = &[0x28, 0xb5, 0x2f, 0xfd];
        assert_eq!(
            file_mode(Path::new("archive.tar.zst"), zstd),
            FileMode::Download
        );
    }

    #[test]
    fn file_mode_binary_is_download() {
        let elf = &[
            0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];
        assert_eq!(file_mode(Path::new("program"), elf), FileMode::Download);
    }

    #[test]
    fn raw_blob_includes_hidden_source_block() {
        let html = raw_blob_html("fn main() {}\n", Some("rust"));
        assert!(html.contains("ghrm-blob-source"));
        assert!(html.contains(r#"class="language-rust""#));
        assert!(html.contains("<tbody></tbody>"));
    }
}
