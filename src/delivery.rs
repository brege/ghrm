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
    let mut res = stream_file(&path).await;
    if res.status().is_success() {
        res.headers_mut().insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&content_disposition(&path)).unwrap(),
        );
    }
    res
}

pub(crate) async fn stream_file(path: &Path) -> Response {
    serve_file(ServeFile::new(path)).await
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
    let Ok(mut file) = tokio::fs::File::open(path).await else {
        return false;
    };
    let mut bytes = vec![0; PEEK_BYTES];
    let Ok(n) = file.read(&mut bytes).await else {
        return false;
    };
    bytes.truncate(n);
    previews_bytes(&bytes)
}

pub(crate) fn previews_text_sync(path: &Path) -> bool {
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut bytes = [0; PEEK_BYTES];
    let Ok(n) = file.read(&mut bytes) else {
        return false;
    };
    previews_bytes(&bytes[..n])
}

fn previews_bytes(bytes: &[u8]) -> bool {
    infer::get(bytes).is_none_or(|kind| kind.matcher_type() == infer::MatcherType::Text)
        && content_inspector::inspect(bytes).is_text()
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
    fn preview_detection_uses_file_signatures() {
        assert!(previews_bytes(b"# hello\n"));
        assert!(!previews_bytes(&[0x28, 0xb5, 0x2f, 0xfd]));
    }

    #[test]
    fn raw_blob_includes_hidden_source_block() {
        let html = raw_blob_html("fn main() {}\n", Some("rust"));
        assert!(html.contains("ghrm-blob-source"));
        assert!(html.contains(r#"class="language-rust""#));
        assert!(html.contains("<tbody></tbody>"));
    }
}
