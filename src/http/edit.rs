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
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use tracing::warn;

pub(crate) const MAX_EDIT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Serialize)]
struct EditSummary {
    path: String,
    bytes: usize,
    lines: usize,
}

#[derive(Serialize)]
struct RenameSummary {
    path: String,
    name: String,
    href: String,
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
    // If-None-Match: * makes the PUT create-only per conditional-request
    // semantics; otherwise it updates an existing file under If-Match.
    let creating = if_none_match_star(&headers);
    let response = if creating {
        create_at(delivery::served_base(&s), &path, &headers, body).await
    } else {
        save_at(delivery::served_base(&s), &path, &headers, body).await
    };
    finish_mutation(&s, creating, response).await
}

pub(crate) async fn remove(
    State(s): State<AppState>,
    AxPath(path): AxPath<String>,
    headers: HeaderMap,
) -> Response {
    if !s.edit {
        return not_found();
    }
    let response = remove_at(delivery::served_base(&s), &path, &headers).await;
    finish_mutation(&s, true, response).await
}

pub(crate) async fn rename(
    State(s): State<AppState>,
    AxPath(path): AxPath<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !s.edit {
        return not_found();
    }
    let response = rename_at(delivery::served_base(&s), &path, &headers, &body).await;
    finish_mutation(&s, true, response).await
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
    let Some(text) = delivery::text_plain_body(headers, &body) else {
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

async fn create_at(base: &Path, rel: &str, headers: &HeaderMap, body: Bytes) -> Response {
    let Some(text) = delivery::text_plain_body(headers, &body) else {
        return bad_request("expected text/plain UTF-8 body");
    };
    if body.len() > MAX_EDIT_BYTES {
        return too_large();
    }
    let Some(rel_path) = paths::safe_rel(rel) else {
        return not_found();
    };
    // safe_rel only proves the components are ordinary; the name this request
    // creates still has to be one the filesystem accepts, or open() fails with
    // ENAMETOOLONG and the request reads as a server fault.
    let Some(name) = rel_path.file_name().and_then(|name| name.to_str()) else {
        return not_found();
    };
    if !valid_component(name) {
        return bad_request("invalid file name");
    }
    let path = base.join(&rel_path);
    // The parent directory must already exist inside the served tree; create
    // does not make directories.
    let Some(parent) = path.parent() else {
        return not_found();
    };
    if !parent.is_dir() || !delivery::confined(parent, base) {
        return not_found();
    }
    let summary = EditSummary {
        path: rel.trim_matches('/').to_string(),
        bytes: body.len(),
        lines: text.lines().count(),
    };
    let mut file = match tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .await
    {
        Ok(file) => file,
        Err(err) if err.kind() == ErrorKind::AlreadyExists => {
            return match tokio::fs::read(&path).await {
                Ok(current) => precondition_failed(&version(&current)),
                Err(_) => precondition_failed_unknown(),
            };
        }
        Err(err) if err.kind() == ErrorKind::NotFound => return not_found(),
        Err(err) => {
            warn!("edit create failed: {err}");
            return server_error();
        }
    };
    let write = async {
        file.write_all(&body).await?;
        file.flush().await
    }
    .await;
    if let Err(err) = write {
        drop(file);
        let _ = tokio::fs::remove_file(&path).await;
        warn!("edit create failed: {err}");
        return server_error();
    }
    let mut response = (StatusCode::CREATED, Json(summary)).into_response();
    set_version(response.headers_mut(), &version(&body));
    response
}

async fn remove_at(base: &Path, rel: &str, headers: &HeaderMap) -> Response {
    let Some(path) = paths::resolve_file(base, rel) else {
        return not_found();
    };
    if !delivery::confined(&path, base) {
        return not_found();
    }
    // If-Match is optional for deletion: the explorer deletes by name, while
    // the file view sends the version it rendered.
    match expected_version(headers) {
        Ok(expected) => {
            let bytes = match tokio::fs::read(&path).await {
                Ok(bytes) => bytes,
                Err(err) => {
                    warn!("edit read failed: {err}");
                    return server_error();
                }
            };
            let current = version(&bytes);
            if expected != current {
                return precondition_failed(&current);
            }
        }
        Err(VersionError::Missing) => {}
        Err(VersionError::Invalid) => return bad_request("invalid If-Match header"),
    }
    if let Err(err) = tokio::fs::remove_file(&path).await {
        warn!("edit delete failed: {err}");
        return server_error();
    }
    Response::builder()
        .status(StatusCode::NO_CONTENT)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::empty())
        .unwrap()
}

async fn rename_at(base: &Path, rel: &str, headers: &HeaderMap, body: &Bytes) -> Response {
    let Some(name) = delivery::text_plain_body(headers, body) else {
        return bad_request("expected text/plain UTF-8 file name");
    };
    let name = name.trim();
    if !valid_component(name) {
        return bad_request("invalid file name");
    }
    let Some(path) = paths::resolve_file(base, rel) else {
        return not_found();
    };
    if !delivery::confined(&path, base) {
        return not_found();
    }
    let dest = path.with_file_name(name);
    // Link then unlink rather than rename, so an existing destination is
    // reported instead of silently overwritten.
    match tokio::fs::hard_link(&path, &dest).await {
        Ok(()) => {
            if let Err(err) = tokio::fs::remove_file(&path).await {
                warn!("edit rename failed: {err}");
                return server_error();
            }
        }
        Err(err) if err.kind() == ErrorKind::AlreadyExists => {
            let case_only = match case_only_target(&path, &dest).await {
                Ok(case_only) => case_only,
                Err(err) => {
                    warn!("edit rename inspection failed: {err}");
                    return server_error();
                }
            };
            if !case_only {
                return conflict("target name exists");
            }
            if let Err(err) = tokio::fs::rename(&path, &dest).await {
                warn!("edit rename failed: {err}");
                return server_error();
            }
        }
        Err(err) if err.kind() == ErrorKind::NotFound => return not_found(),
        Err(err) => {
            warn!("edit rename failed: {err}");
            return server_error();
        }
    }
    let rel = rel.trim_matches('/');
    let new_rel = match rel.rsplit_once('/') {
        Some((parent, _)) => format!("{parent}/{name}"),
        None => name.to_string(),
    };
    Json(RenameSummary {
        href: paths::url_path(&new_rel),
        path: new_rel,
        name: name.to_string(),
    })
    .into_response()
}

// Only a mutation that adds, removes, or renames an entry changes the shape of
// the navigation tree, and rebuilding it walks the whole target. This matches
// the filesystem watcher, which treats create, remove, and rename as nav events
// and leaves content writes alone.
async fn finish_mutation(s: &AppState, structural: bool, response: Response) -> Response {
    if structural && response.status().is_success() {
        s.refresh_nav().await;
    }
    response
}

/// Identifies an existing target as the source entry under another case
/// spelling. Distinct hard links and symlinks can resolve to the same file, so
/// two exact directory names still constitute a collision.
async fn case_only_target(left: &Path, right: &Path) -> io::Result<bool> {
    let left = left.to_path_buf();
    let right = right.to_path_buf();
    tokio::task::spawn_blocking(move || {
        if !same_file::is_same_file(&left, &right).unwrap_or(false) {
            return Ok(false);
        }
        let left_name = left
            .file_name()
            .and_then(|name| name.to_str())
            .expect("rename source has a UTF-8 file name");
        let right_name = right
            .file_name()
            .and_then(|name| name.to_str())
            .expect("rename target has a UTF-8 file name");
        if !unicase::eq(left_name, right_name) {
            return Ok(false);
        }
        if left_name == right_name {
            return Ok(true);
        }

        let parent = left.parent().expect("rename source has a parent");
        let mut left_exact = false;
        let mut right_exact = false;
        for entry in std::fs::read_dir(parent)? {
            let name = entry?.file_name();
            left_exact |= name == left_name;
            right_exact |= name == right_name;
        }
        Ok(!(left_exact && right_exact))
    })
    .await
    .expect("case-only rename inspection completes")
}

/// True for a single, ordinary path component: renames stay within the file's
/// directory and cannot introduce separators or traversal.
fn valid_component(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains('\0')
}

fn if_none_match_star(headers: &HeaderMap) -> bool {
    headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.trim() == "*")
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
    // An empty file is vacuously editable text; otherwise the complete bytes
    // must be UTF-8 text in a mode the file view renders as text.
    let editable = bytes.is_empty()
        || (std::str::from_utf8(&bytes).is_ok()
            && delivery::is_text_content(&bytes)
            && matches!(
                delivery::file_mode(&path, &bytes),
                delivery::FileMode::Markdown
                    | delivery::FileMode::Source
                    | delivery::FileMode::Dual
            ));
    if !editable {
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

fn precondition_failed_unknown() -> Response {
    Response::builder()
        .status(StatusCode::PRECONDITION_FAILED)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from("file changed on disk"))
        .unwrap()
}

fn conflict(message: &'static str) -> Response {
    Response::builder()
        .status(StatusCode::CONFLICT)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from(message))
        .unwrap()
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
    use crate::explorer::column;
    use crate::explorer::filter;
    use crate::explorer::view::ViewConfig;
    use crate::explorer::walk::{self, NavSet, ViewOpts};
    use crate::http::server::Mode;
    use crate::repo::RepoSet;
    use crate::runtime;
    use crate::testutil::TempDir;
    use std::sync::{Arc, RwLock};
    use tokio::sync::broadcast;

    fn app_state(target: &Path) -> AppState {
        AppState {
            target: target.to_path_buf(),
            mode: Mode::Dir,
            nav: Arc::new(RwLock::new(NavSet::default())),
            alternate_nav: Arc::new(RwLock::new(Some(NavSet::default()))),
            nav_generation: Arc::new(walk::NavGeneration::default()),
            repos: RepoSet::discover(target, &[]),
            reload: broadcast::channel(4).0,
            use_ignore: false,
            show_excludes: false,
            view_cfg: ViewConfig {
                default: ViewOpts::default(),
                default_use_ignore: false,
                default_groups: Vec::new(),
                default_sort: walk::Sort::Name,
                default_columns: column::Set::from_defaults(|def| def.default_visible),
                can_toggle_excludes: false,
            },
            filter_exts: Vec::new(),
            filters: filter::Set::default(),
            exclude_names: Vec::new(),
            #[cfg(feature = "archive")]
            archive_jobs: crate::http::archive::ArchiveJobs::new().unwrap(),
            search_max_rows: 10,
            home: None,
            runtime_paths: runtime::Paths::new(target, None).unwrap(),
            #[cfg(feature = "stats")]
            stats: crate::stat::Config::default(),
            auth: None,
            #[cfg(feature = "gist")]
            gist: None,
            edit: true,
        }
    }

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

    fn plain_headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, "text/plain".parse().unwrap());
        headers
    }

    fn create_headers() -> HeaderMap {
        let mut headers = plain_headers();
        headers.insert(header::IF_NONE_MATCH, "*".parse().unwrap());
        headers
    }

    #[tokio::test]
    async fn remove_deletes_file() {
        let td = TempDir::new("ghrm-edit-remove");
        let file = td.path().join("notes.md");
        std::fs::write(&file, "# old\n").unwrap();

        let response = remove_at(td.path(), "notes.md", &headers("text/plain", b"# old\n")).await;

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert!(!file.exists());
    }

    #[tokio::test]
    async fn remove_without_version_is_unconditional() {
        let td = TempDir::new("ghrm-edit-remove-any");
        let file = td.path().join("image.png");
        std::fs::write(&file, [0x89, 0x50, 0x4e, 0x47]).unwrap();

        let response = remove_at(td.path(), "image.png", &HeaderMap::new()).await;

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert!(!file.exists());
    }

    #[tokio::test]
    async fn remove_rejects_stale_version() {
        let td = TempDir::new("ghrm-edit-remove-stale");
        let file = td.path().join("notes.md");
        std::fs::write(&file, "external\n").unwrap();

        let response =
            remove_at(td.path(), "notes.md", &headers("text/plain", b"original\n")).await;

        assert_eq!(response.status(), StatusCode::PRECONDITION_FAILED);
        assert!(file.exists());
    }

    #[tokio::test]
    async fn remove_rejects_traversal_and_missing() {
        let td = TempDir::new("ghrm-edit-remove-missing");
        let secret = td.path().join("secret.md");
        std::fs::write(&secret, "keep\n").unwrap();
        let base = td.path().join("root");
        std::fs::create_dir(&base).unwrap();

        let traversal = remove_at(&base, "../secret.md", &HeaderMap::new()).await;
        let missing = remove_at(&base, "absent.md", &HeaderMap::new()).await;

        assert_eq!(traversal.status(), StatusCode::NOT_FOUND);
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
        assert!(secret.exists());
    }

    #[tokio::test]
    async fn rename_moves_file_and_reports_new_path() {
        let td = TempDir::new("ghrm-edit-rename");
        std::fs::create_dir(td.path().join("docs")).unwrap();
        let file = td.path().join("docs/old.md");
        std::fs::write(&file, "# doc\n").unwrap();

        let response = rename_at(
            td.path(),
            "docs/old.md",
            &plain_headers(),
            &Bytes::from_static(b"new.md"),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(!file.exists());
        assert!(td.path().join("docs/new.md").is_file());
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let text = std::str::from_utf8(&body).unwrap();
        assert!(text.contains("\"path\":\"docs/new.md\""));
        assert!(text.contains("\"name\":\"new.md\""));
        assert!(text.contains("\"href\":\"/docs/new.md\""));
    }

    #[tokio::test]
    async fn rename_encodes_the_reported_href() {
        let td = TempDir::new("ghrm-edit-rename-href");
        std::fs::write(td.path().join("a.md"), "a\n").unwrap();

        let response = rename_at(
            td.path(),
            "a.md",
            &plain_headers(),
            &Bytes::from_static(b"notes #1?.md"),
        )
        .await;

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let text = std::str::from_utf8(&body).unwrap();
        assert!(text.contains("\"href\":\"/notes%20%231%3F.md\""));
    }

    #[tokio::test]
    async fn rename_rejects_collision() {
        let td = TempDir::new("ghrm-edit-rename-collision");
        std::fs::write(td.path().join("a.md"), "a\n").unwrap();
        std::fs::write(td.path().join("b.md"), "b\n").unwrap();

        let response = rename_at(
            td.path(),
            "a.md",
            &plain_headers(),
            &Bytes::from_static(b"b.md"),
        )
        .await;

        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert!(td.path().join("a.md").is_file());
        assert_eq!(
            std::fs::read_to_string(td.path().join("b.md")).unwrap(),
            "b\n"
        );
    }

    #[tokio::test]
    async fn rename_to_the_same_name_succeeds() {
        let td = TempDir::new("ghrm-edit-rename-same");
        std::fs::write(td.path().join("a.md"), "a\n").unwrap();

        let response = rename_at(
            td.path(),
            "a.md",
            &plain_headers(),
            &Bytes::from_static(b"a.md"),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            std::fs::read_to_string(td.path().join("a.md")).unwrap(),
            "a\n"
        );
    }

    #[tokio::test]
    async fn rename_changes_the_name_case() {
        let td = TempDir::new("ghrm-edit-rename-case");
        std::fs::write(td.path().join("readme.md"), "a\n").unwrap();

        let response = rename_at(
            td.path(),
            "readme.md",
            &plain_headers(),
            &Bytes::from_static(b"README.md"),
        )
        .await;

        let names = std::fs::read_dir(td.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(names.iter().any(|name| name == "README.md"));
        assert!(!names.iter().any(|name| name == "readme.md"));
    }

    #[tokio::test]
    async fn rename_rejects_a_hard_link_collision() {
        let td = TempDir::new("ghrm-edit-rename-hard-link");
        let source = td.path().join("a.md");
        let target = td.path().join("b.md");
        std::fs::write(&source, "a\n").unwrap();
        std::fs::hard_link(&source, &target).unwrap();

        let response = rename_at(
            td.path(),
            "a.md",
            &plain_headers(),
            &Bytes::from_static(b"b.md"),
        )
        .await;

        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert!(source.is_file());
        assert!(target.is_file());
    }

    // Names equal under case folding and one shared inode are also what a
    // case-equal hard link looks like. Only a case-insensitive filesystem
    // reports one directory entry for both spellings, so the two entries here
    // are a collision rather than a case change.
    #[tokio::test]
    async fn rename_rejects_a_case_equal_hard_link_collision() {
        let td = TempDir::new("ghrm-edit-rename-case-link");
        let source = td.path().join("readme.md");
        let target = td.path().join("README.md");
        std::fs::write(&source, "a\n").unwrap();
        // A case-insensitive filesystem cannot hold both spellings; the branch
        // under test only exists for filesystems that can.
        if std::fs::hard_link(&source, &target).is_err() {
            return;
        }

        let response = rename_at(
            td.path(),
            "readme.md",
            &plain_headers(),
            &Bytes::from_static(b"README.md"),
        )
        .await;

        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert!(source.is_file());
        assert!(target.is_file());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn rename_rejects_a_symlink_to_the_source() {
        use std::os::unix::fs::symlink;

        let td = TempDir::new("ghrm-edit-rename-symlink-source");
        let source = td.path().join("a.md");
        let target = td.path().join("b.md");
        std::fs::write(&source, "a\n").unwrap();
        symlink("a.md", &target).unwrap();

        let response = rename_at(
            td.path(),
            "a.md",
            &plain_headers(),
            &Bytes::from_static(b"b.md"),
        )
        .await;

        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert!(source.is_file());
        assert!(target.is_symlink());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn rename_rejects_a_dangling_symlink_collision() {
        use std::os::unix::fs::symlink;

        let td = TempDir::new("ghrm-edit-rename-symlink-collision");
        std::fs::write(td.path().join("a.md"), "a\n").unwrap();
        symlink(td.path().join("missing.md"), td.path().join("b.md")).unwrap();

        let response = rename_at(
            td.path(),
            "a.md",
            &plain_headers(),
            &Bytes::from_static(b"b.md"),
        )
        .await;

        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert!(td.path().join("a.md").is_file());
        assert!(td.path().join("b.md").is_symlink());
    }

    #[tokio::test]
    async fn rename_rejects_separators_and_traversal_names() {
        let td = TempDir::new("ghrm-edit-rename-bad");
        std::fs::write(td.path().join("a.md"), "a\n").unwrap();

        for bad in ["x/y.md", "..", ".", "", "x\\y.md"] {
            let response = rename_at(
                td.path(),
                "a.md",
                &plain_headers(),
                &Bytes::from(bad.as_bytes().to_vec()),
            )
            .await;
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "name: {bad:?}");
        }
        assert!(td.path().join("a.md").is_file());
    }

    #[tokio::test]
    async fn create_writes_new_file() {
        let td = TempDir::new("ghrm-edit-create");
        std::fs::create_dir(td.path().join("docs")).unwrap();

        let response = create_at(
            td.path(),
            "docs/fresh.md",
            &create_headers(),
            Bytes::from_static(b"# fresh\n"),
        )
        .await;

        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(
            response
                .headers()
                .get(header::ETAG)
                .unwrap()
                .to_str()
                .unwrap(),
            format!("\"{}\"", version(b"# fresh\n"))
        );
        assert_eq!(
            std::fs::read_to_string(td.path().join("docs/fresh.md")).unwrap(),
            "# fresh\n"
        );
    }

    #[tokio::test]
    async fn create_rejects_existing_file() {
        let td = TempDir::new("ghrm-edit-create-exists");
        std::fs::write(td.path().join("a.md"), "keep\n").unwrap();

        let response = create_at(
            td.path(),
            "a.md",
            &create_headers(),
            Bytes::from_static(b"clobber\n"),
        )
        .await;

        assert_eq!(response.status(), StatusCode::PRECONDITION_FAILED);
        assert_eq!(
            std::fs::read_to_string(td.path().join("a.md")).unwrap(),
            "keep\n"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn create_rejects_a_dangling_symlink_without_following_it() {
        use std::os::unix::fs::symlink;

        let td = TempDir::new("ghrm-edit-create-symlink");
        let base = td.path().join("root");
        let outside = td.path().join("outside.md");
        std::fs::create_dir(&base).unwrap();
        symlink(&outside, base.join("link.md")).unwrap();

        let response = create_at(
            &base,
            "link.md",
            &create_headers(),
            Bytes::from_static(b"escape\n"),
        )
        .await;

        assert_eq!(response.status(), StatusCode::PRECONDITION_FAILED);
        assert!(!outside.exists());
        assert!(base.join("link.md").is_symlink());
    }

    #[tokio::test]
    async fn create_rejects_missing_parent_and_traversal() {
        let td = TempDir::new("ghrm-edit-create-parent");
        let base = td.path().join("root");
        std::fs::create_dir(&base).unwrap();

        let orphan = create_at(&base, "nodir/x.md", &create_headers(), Bytes::new()).await;
        let escape = create_at(&base, "../x.md", &create_headers(), Bytes::new()).await;

        assert_eq!(orphan.status(), StatusCode::NOT_FOUND);
        assert_eq!(escape.status(), StatusCode::NOT_FOUND);
        assert!(!td.path().join("x.md").exists());
    }

    // A name past the filesystem component limit reaches open() as
    // ENAMETOOLONG, which is a rejected request rather than a server fault.
    #[tokio::test]
    async fn create_rejects_a_name_the_filesystem_cannot_hold() {
        let td = TempDir::new("ghrm-edit-create-long");
        let name = format!("{}.md", "x".repeat(300));

        let response = create_at(td.path(), &name, &create_headers(), Bytes::new()).await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(!td.path().join(&name).exists());
    }

    #[tokio::test]
    async fn create_then_save_empty_file() {
        let td = TempDir::new("ghrm-edit-create-empty");

        let created = create_at(td.path(), "fresh.txt", &create_headers(), Bytes::new()).await;
        assert_eq!(created.status(), StatusCode::CREATED);

        let saved = save_at(
            td.path(),
            "fresh.txt",
            &headers("text/plain", b""),
            Bytes::from_static(b"first line\n"),
        )
        .await;

        assert_eq!(saved.status(), StatusCode::OK);
        assert_eq!(
            std::fs::read_to_string(td.path().join("fresh.txt")).unwrap(),
            "first line\n"
        );
    }

    #[tokio::test]
    async fn successful_mutation_refreshes_navigation_caches() {
        let td = TempDir::new("ghrm-edit-nav-refresh");
        let state = app_state(td.path());
        let mut reload = state.reload.subscribe();

        let response = save(
            State(state.clone()),
            AxPath("fresh.md".to_string()),
            create_headers(),
            Bytes::new(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::CREATED);
        assert!(state.alternate_nav.read().unwrap().is_none());
        assert!(
            state
                .nav
                .read()
                .unwrap()
                .entries()
                .any(|entry| entry.path == Path::new("fresh.md"))
        );
        assert_eq!(reload.try_recv().unwrap(), "nav-ready");
    }

    // Overwriting a file leaves the tree the same shape, so it must not pay for
    // a full walk. The refresh clears the alternate cache, so an intact
    // alternate proves no rebuild ran.
    #[tokio::test]
    async fn save_leaves_navigation_caches_alone() {
        let td = TempDir::new("ghrm-edit-nav-save");
        let file = td.path().join("notes.md");
        std::fs::write(&file, "# old\n").unwrap();
        let state = app_state(td.path());

        let response = save(
            State(state.clone()),
            AxPath("notes.md".to_string()),
            headers("text/plain; charset=utf-8", b"# old\n"),
            Bytes::from_static(b"# new\n"),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "# new\n");
        assert!(state.alternate_nav.read().unwrap().is_some());
    }

    #[tokio::test]
    async fn rename_refreshes_navigation_caches() {
        let td = TempDir::new("ghrm-edit-nav-rename");
        std::fs::write(td.path().join("a.md"), "a\n").unwrap();
        let state = app_state(td.path());

        let response = rename(
            State(state.clone()),
            AxPath("a.md".to_string()),
            plain_headers(),
            Bytes::from_static(b"b.md"),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(state.alternate_nav.read().unwrap().is_none());
    }

    // A stale walk must not reinstate the tree it scanned. Taking a newer
    // ticket while an older one is outstanding retires the older one.
    #[test]
    fn navigation_generation_retires_superseded_rebuilds() {
        let generation = walk::NavGeneration::default();
        let mut installed = None;

        let older = generation.ticket();
        let newer = generation.ticket();
        assert_eq!(
            generation.install(older, || installed = Some("older")),
            None
        );
        assert_eq!(
            generation.install(newer, || installed = Some("newer")),
            Some(())
        );

        assert_eq!(installed, Some("newer"));
    }

    #[test]
    fn navigation_generation_serializes_ticket_and_install() {
        use std::sync::mpsc;
        use std::time::Duration;

        let generation = Arc::new(walk::NavGeneration::default());
        let ticket = generation.ticket();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let installing = generation.clone();
        let installer = std::thread::spawn(move || {
            assert_eq!(
                installing.install(ticket, || {
                    entered_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                }),
                Some(())
            );
        });

        entered_rx.recv().unwrap();
        let (issued_tx, issued_rx) = mpsc::channel();
        let issuing = generation.clone();
        let issuer = std::thread::spawn(move || issued_tx.send(issuing.ticket()).unwrap());
        assert!(issued_rx.recv_timeout(Duration::from_millis(25)).is_err());

        release_tx.send(()).unwrap();
        installer.join().unwrap();
        assert_eq!(
            issued_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
            ticket + 1
        );
        issuer.join().unwrap();
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
