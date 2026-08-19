use crate::http::delivery;
use crate::http::server::AppState;
use crate::paths;

use axum::{
    Json,
    body::{Body, Bytes},
    extract::{Path as AxPath, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use base64::Engine;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tracing::warn;

pub(crate) const MAX_EDIT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Serialize)]
struct EditSummary {
    path: String,
    bytes: usize,
    lines: usize,
}

struct Target {
    path: PathBuf,
    version: String,
}

enum VersionError {
    Missing,
    Invalid,
}

pub(crate) async fn save(
    State(s): State<AppState>,
    AxPath(path): AxPath<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !s.edit {
        return not_found();
    }
    save_at(delivery::served_base(&s), &path, &headers, body).await
}

pub(crate) async fn current(State(s): State<AppState>, AxPath(path): AxPath<String>) -> Response {
    if !s.edit {
        return not_found();
    }
    let target = match target_at(delivery::served_base(&s), &path).await {
        Ok(target) => target,
        Err(response) => return response,
    };
    empty_with_version(StatusCode::OK, &target.version)
}

async fn save_at(base: &Path, rel: &str, headers: &HeaderMap, body: Bytes) -> Response {
    let Some(text) = text_body(headers, &body) else {
        return bad_request("expected text/plain UTF-8 body");
    };
    if body.len() > MAX_EDIT_BYTES {
        return too_large();
    }
    let target = match target_at(base, rel).await {
        Ok(target) => target,
        Err(response) => return response,
    };
    let expected = match expected_version(headers) {
        Ok(version) => version,
        Err(VersionError::Missing) => return precondition_required(),
        Err(VersionError::Invalid) => return bad_request("invalid If-Match header"),
    };
    if expected != target.version {
        return precondition_failed(&target.version);
    }
    let version = version(&body);
    let summary = EditSummary {
        path: rel.trim_matches('/').to_string(),
        bytes: body.len(),
        lines: text.lines().count(),
    };
    if let Err(err) = tokio::fs::write(&target.path, &body).await {
        warn!("edit write failed: {err}");
        return server_error();
    }
    let mut response = Json(summary).into_response();
    set_version(response.headers_mut(), &version);
    response
}

async fn target_at(base: &Path, rel: &str) -> Result<Target, Response> {
    // resolve_file rejects parent traversal and requires an existing file.
    // confined additionally rejects symlinks whose target escapes the tree.
    let Some(path) = paths::resolve_file(base, rel) else {
        return Err(not_found());
    };
    if !delivery::confined(&path, base) {
        return Err(not_found());
    }
    let bytes = match tokio::fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(err) => {
            warn!("edit read failed: {err}");
            return Err(server_error());
        }
    };
    if std::str::from_utf8(&bytes).is_err()
        || !delivery::is_text_content(&bytes)
        || !matches!(
            delivery::file_mode(&path, &bytes),
            delivery::FileMode::Markdown | delivery::FileMode::Source | delivery::FileMode::Dual
        )
    {
        return Err(unsupported("target is not an editable text file"));
    }
    Ok(Target {
        path,
        version: version(&bytes),
    })
}

fn expected_version(headers: &HeaderMap) -> Result<&str, VersionError> {
    let Some(value) = headers.get(header::IF_MATCH) else {
        return Err(VersionError::Missing);
    };
    let Ok(value) = value.to_str() else {
        return Err(VersionError::Invalid);
    };
    let Some(value) = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) else {
        return Err(VersionError::Invalid);
    };
    if value.is_empty() || value.contains('"') {
        return Err(VersionError::Invalid);
    }
    Ok(value)
}

pub(crate) fn version(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(bytes))
}

pub(crate) fn line_ending_attr(text: &str) -> &'static str {
    let bytes = text.as_bytes();
    let mut crlf = 0;
    let mut other = 0;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => {
                crlf += 1;
                index += 2;
            }
            b'\r' => {
                other += 1;
                index += 1;
            }
            b'\n' => {
                other += 1;
                index += 1;
            }
            _ => index += 1,
        }
    }
    if crlf > other {
        " data-ghrm-eol=\"crlf\""
    } else {
        ""
    }
}

fn text_body<'a>(headers: &HeaderMap, body: &'a Bytes) -> Option<&'a str> {
    if !is_text_plain(headers) {
        return None;
    }
    std::str::from_utf8(body).ok()
}

fn is_text_plain(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.split(';').next().unwrap_or("").trim())
        .is_some_and(|value| value.eq_ignore_ascii_case("text/plain"))
}

fn bad_request(message: &'static str) -> Response {
    Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from(message))
        .unwrap()
}

fn too_large() -> Response {
    Response::builder()
        .status(StatusCode::PAYLOAD_TOO_LARGE)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from("edit body too large"))
        .unwrap()
}

fn precondition_required() -> Response {
    Response::builder()
        .status(StatusCode::PRECONDITION_REQUIRED)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from("If-Match is required"))
        .unwrap()
}

fn precondition_failed(version: &str) -> Response {
    let mut response = Response::builder()
        .status(StatusCode::PRECONDITION_FAILED)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from("file changed on disk"))
        .unwrap();
    set_version(response.headers_mut(), version);
    response
}

fn unsupported(message: &'static str) -> Response {
    Response::builder()
        .status(StatusCode::UNSUPPORTED_MEDIA_TYPE)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from(message))
        .unwrap()
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

fn empty_with_version(status: StatusCode, version: &str) -> Response {
    let mut response = Response::builder()
        .status(status)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::empty())
        .unwrap();
    set_version(response.headers_mut(), version);
    response
}

fn set_version(headers: &mut HeaderMap, version: &str) {
    let value = HeaderValue::from_str(&format!("\"{version}\"")).unwrap();
    headers.insert(header::ETAG, value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    fn headers(content_type: &str, current: &[u8]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, content_type.parse().unwrap());
        headers.insert(
            header::IF_MATCH,
            format!("\"{}\"", version(current)).parse().unwrap(),
        );
        headers
    }

    #[tokio::test]
    async fn save_overwrites_existing_file() {
        let td = TempDir::new("ghrm-edit-write");
        let file = td.path().join("notes.md");
        std::fs::write(&file, "# old\n").unwrap();

        let response = save_at(
            td.path(),
            "notes.md",
            &headers("text/plain; charset=utf-8", b"# old\n"),
            Bytes::from_static(b"# new\nbody\n"),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::ETAG)
                .unwrap()
                .to_str()
                .unwrap(),
            format!("\"{}\"", version(b"# new\nbody\n"))
        );
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "# new\nbody\n");
    }

    #[tokio::test]
    async fn save_requires_current_version() {
        let td = TempDir::new("ghrm-edit-version");
        let file = td.path().join("notes.md");
        std::fs::write(&file, "# old\n").unwrap();
        let mut request_headers = HeaderMap::new();
        request_headers.insert(header::CONTENT_TYPE, "text/plain".parse().unwrap());

        let response = save_at(
            td.path(),
            "notes.md",
            &request_headers,
            Bytes::from_static(b"# new\n"),
        )
        .await;

        assert_eq!(response.status(), StatusCode::PRECONDITION_REQUIRED);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "# old\n");
    }

    #[tokio::test]
    async fn save_rejects_stale_version() {
        let td = TempDir::new("ghrm-edit-stale");
        let file = td.path().join("notes.md");
        std::fs::write(&file, "external\n").unwrap();

        let response = save_at(
            td.path(),
            "notes.md",
            &headers("text/plain", b"original\n"),
            Bytes::from_static(b"browser\n"),
        )
        .await;

        assert_eq!(response.status(), StatusCode::PRECONDITION_FAILED);
        assert_eq!(
            response
                .headers()
                .get(header::ETAG)
                .unwrap()
                .to_str()
                .unwrap(),
            format!("\"{}\"", version(b"external\n"))
        );
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "external\n");
    }

    #[tokio::test]
    async fn save_rejects_non_text_content_type() {
        let td = TempDir::new("ghrm-edit-content-type");
        let file = td.path().join("notes.md");
        std::fs::write(&file, "# old\n").unwrap();

        let response = save_at(
            td.path(),
            "notes.md",
            &headers("application/json", b"# old\n"),
            Bytes::from_static(b"{}"),
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "# old\n");
    }

    #[tokio::test]
    async fn save_rejects_missing_file() {
        let td = TempDir::new("ghrm-edit-missing");

        let response = save_at(
            td.path(),
            "absent.md",
            &headers("text/plain", b""),
            Bytes::from_static(b"hi\n"),
        )
        .await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn save_rejects_parent_traversal() {
        let td = TempDir::new("ghrm-edit-traversal");
        let secret = td.path().join("secret.md");
        std::fs::write(&secret, "keep\n").unwrap();
        let base = td.path().join("root");
        std::fs::create_dir(&base).unwrap();

        let response = save_at(
            &base,
            "../secret.md",
            &headers("text/plain", b"keep\n"),
            Bytes::from_static(b"pwned\n"),
        )
        .await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(std::fs::read_to_string(&secret).unwrap(), "keep\n");
    }

    #[tokio::test]
    async fn save_rejects_large_body() {
        let td = TempDir::new("ghrm-edit-large");
        let file = td.path().join("big.txt");
        std::fs::write(&file, "small\n").unwrap();

        let response = save_at(
            td.path(),
            "big.txt",
            &headers("text/plain", b"small\n"),
            Bytes::from(vec![b'a'; MAX_EDIT_BYTES + 1]),
        )
        .await;

        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "small\n");
    }

    #[tokio::test]
    async fn save_rejects_non_text_target() {
        let td = TempDir::new("ghrm-edit-binary");
        let file = td.path().join("image.png");
        let png = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
        std::fs::write(&file, png).unwrap();

        let response = save_at(
            td.path(),
            "image.png",
            &headers("text/plain", &png),
            Bytes::from_static(b"overwrite\n"),
        )
        .await;

        assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(std::fs::read(&file).unwrap(), png);
    }

    #[tokio::test]
    async fn save_rejects_invalid_utf8_markdown() {
        let td = TempDir::new("ghrm-edit-invalid-markdown");
        let file = td.path().join("notes.md");
        let invalid = [b'#', b' ', 0xff, b'\n'];
        std::fs::write(&file, invalid).unwrap();

        let response = save_at(
            td.path(),
            "notes.md",
            &headers("text/plain", &invalid),
            Bytes::from_static(b"overwrite\n"),
        )
        .await;

        assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(std::fs::read(&file).unwrap(), invalid);
    }

    #[tokio::test]
    async fn save_rejects_utf8_binary_markdown() {
        let td = TempDir::new("ghrm-edit-binary-markdown");
        let file = td.path().join("notes.md");
        let binary = b"# heading\n\0binary\n";
        std::fs::write(&file, binary).unwrap();

        let response = save_at(
            td.path(),
            "notes.md",
            &headers("text/plain", binary),
            Bytes::from_static(b"overwrite\n"),
        )
        .await;

        assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(std::fs::read(&file).unwrap(), binary);
    }

    #[test]
    fn line_ending_attr_uses_the_dominant_convention() {
        assert_eq!(line_ending_attr("one\ntwo\n"), "");
        assert_eq!(
            line_ending_attr("one\r\ntwo\r\n"),
            " data-ghrm-eol=\"crlf\""
        );
        assert_eq!(line_ending_attr("one\rtwo\r"), "");
        assert_eq!(line_ending_attr("one\r\ntwo\nthree\rfour"), "");
        assert_eq!(
            line_ending_attr("one\r\ntwo\r\nthree\nfour"),
            " data-ghrm-eol=\"crlf\""
        );
    }
}
