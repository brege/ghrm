use crate::http::delivery;
use crate::http::server::{AppState, HtmxContext};
use crate::http::shell;
use crate::render::Rendered;
use crate::repo::SourceState;
use crate::tmpl::{self, GistCtx};

use axum::{
    Json,
    body::{Body, Bytes},
    extract::{Path as AxPath, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::broadcast;
use tracing::warn;

const MAX_PASTE_BYTES: usize = 1024 * 1024;
const GIST_HREF: &str = "/_ghrm/gist";
const NEW_GIST_HREF: &str = "/_ghrm/gist?new=1";

#[derive(Deserialize, Default)]
pub(crate) struct ShowQuery {
    #[serde(default)]
    new: bool,
}
const RAW_HREF: &str = "/_ghrm/gist/raw";
const STASH_HREF: &str = "/_ghrm/gist/stash";
const GIST_ID_HEADER: &str = "X-Ghrm-Gist-Id";
const GIST_NAME_HEADER: &str = "X-Ghrm-Gist-Name";

#[derive(Serialize)]
struct PasteSummary {
    id: String,
    name: String,
    raw_url: &'static str,
    bytes: usize,
    lines: usize,
}

#[derive(Serialize)]
struct RenameSummary {
    id: String,
    name: String,
    href: String,
}

pub(crate) async fn show(
    State(s): State<AppState>,
    Query(query): Query<ShowQuery>,
    headers: HeaderMap,
) -> Response {
    let Some(store) = s.gist.as_ref() else {
        return not_found();
    };
    if query.new {
        return show_paste(&s, &headers, None, NEW_GIST_HREF, RAW_HREF);
    }
    let current = match store.current() {
        Ok(current) => current,
        Err(err) => {
            warn!("gist read failed: {err}");
            return server_error();
        }
    };
    show_paste(&s, &headers, current, GIST_HREF, RAW_HREF)
}

pub(crate) async fn show_id(
    State(s): State<AppState>,
    headers: HeaderMap,
    AxPath(id): AxPath<String>,
) -> Response {
    let Some(store) = s.gist.as_ref() else {
        return not_found();
    };
    let paste = match store.get(&id) {
        Ok(Some(paste)) => Some(paste),
        Ok(None) => return not_found(),
        Err(err) => {
            warn!("gist read failed: {err}");
            return not_found();
        }
    };
    let page_href = format!("{GIST_HREF}/p/{id}");
    let raw_href = format!("{RAW_HREF}/{id}");
    show_paste(&s, &headers, paste, &page_href, &raw_href)
}

fn show_paste(
    s: &AppState,
    headers: &HeaderMap,
    current: Option<crate::gist::Paste>,
    page_href: &str,
    raw_href: &str,
) -> Response {
    let hx = HtmxContext::from_headers(headers);
    let paste_id = current
        .as_ref()
        .map(|paste| paste.id.as_str())
        .unwrap_or("");
    let paste_body = current
        .as_ref()
        .map(|paste| paste.body.as_str())
        .unwrap_or("");
    let raw_html = delivery::raw_blob_html(paste_body, None);
    let body = match tmpl::gist(GistCtx {
        has_paste: current.is_some(),
        paste_id,
        page_href,
        raw_href,
        stash_href: STASH_HREF,
        paste_body,
        raw_html: &raw_html,
    }) {
        Ok(body) => body,
        Err(err) => {
            warn!("template error: {err}");
            return server_error();
        }
    };

    let title = "Gist";
    if hx.is_htmx {
        return shell::fragment(
            &body,
            title,
            None,
            SourceState::NoRepo,
            &s.runtime_paths,
            true,
        );
    }

    let rendered = Rendered {
        html: String::new(),
        title: title.to_string(),
        langs: Vec::new(),
        lang: None,
        has_mermaid: false,
        has_math: false,
        has_map: false,
    };
    shell::full_page(
        &rendered,
        &body,
        None,
        SourceState::NoRepo,
        s.auth.is_some(),
        &s.runtime_paths,
        true,
    )
}

pub(crate) async fn raw(State(s): State<AppState>) -> Response {
    let Some(store) = s.gist.as_ref() else {
        return not_found();
    };
    let current = match store.current() {
        Ok(Some(current)) => current,
        Ok(None) => return not_found(),
        Err(err) => {
            warn!("gist read failed: {err}");
            return server_error();
        }
    };

    text_response(current.body)
}

pub(crate) async fn raw_id(State(s): State<AppState>, AxPath(id): AxPath<String>) -> Response {
    let Some(store) = s.gist.as_ref() else {
        return not_found();
    };
    let paste = match store.get(&id) {
        Ok(Some(paste)) => paste,
        Ok(None) => return not_found(),
        Err(err) => {
            warn!("gist read failed: {err}");
            return not_found();
        }
    };

    text_response(paste.body)
}

pub(crate) async fn stash(State(s): State<AppState>, headers: HeaderMap) -> Response {
    let Some(store) = s.gist.as_ref() else {
        return not_found();
    };
    let entries = match store.entries() {
        Ok(entries) => entries,
        Err(err) => {
            warn!("gist stash read failed: {err}");
            return server_error();
        }
    };
    let entries: Vec<_> = entries
        .into_iter()
        .map(|entry| tmpl::GistStashEntry {
            id: entry.id.clone(),
            href: format!("{GIST_HREF}/p/{}", entry.id),
            name: entry.name,
            modified: entry.modified,
            size: crate::explorer::column::size_text(entry.size).unwrap_or_default(),
            lines: crate::explorer::column::count_text(entry.lines).unwrap_or_default(),
            current: entry.current,
        })
        .collect();
    let body = match tmpl::gist_stash(tmpl::GistStashCtx { entries: &entries }) {
        Ok(body) => body,
        Err(err) => {
            warn!("template error: {err}");
            return server_error();
        }
    };

    let title = "Gist stash";
    if HtmxContext::from_headers(&headers).is_htmx {
        return shell::fragment(
            &body,
            title,
            None,
            SourceState::NoRepo,
            &s.runtime_paths,
            true,
        );
    }

    let rendered = Rendered {
        html: String::new(),
        title: title.to_string(),
        langs: Vec::new(),
        lang: None,
        has_mermaid: false,
        has_math: false,
        has_map: false,
    };
    shell::full_page(
        &rendered,
        &body,
        None,
        SourceState::NoRepo,
        s.auth.is_some(),
        &s.runtime_paths,
        true,
    )
}

fn text_response(body: String) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from(body))
        .unwrap()
}

pub(crate) async fn create(State(s): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    let Some(store) = s.gist.as_ref() else {
        return not_found();
    };
    create_inner(store, &s.reload, &headers, body)
}

pub(crate) async fn rename(
    State(s): State<AppState>,
    AxPath(id): AxPath<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Some(store) = s.gist.as_ref() else {
        return not_found();
    };
    rename_inner(store, &s.reload, &id, &headers, &body)
}

pub(crate) async fn delete_id(State(s): State<AppState>, AxPath(id): AxPath<String>) -> Response {
    let Some(store) = s.gist.as_ref() else {
        return not_found();
    };
    delete_inner(store, &s.reload, &id)
}

fn delete_inner(
    store: &crate::gist::Store,
    reload: &broadcast::Sender<String>,
    id: &str,
) -> Response {
    if !store.has(id) {
        return not_found();
    }
    match store.delete(id) {
        Ok(()) => {
            let _ = reload.send("gist".to_string());
            Response::builder()
                .status(StatusCode::NO_CONTENT)
                .body(Body::empty())
                .unwrap()
        }
        Err(err) => {
            warn!("gist delete failed: {err}");
            server_error()
        }
    }
}

fn create_inner(
    store: &crate::gist::Store,
    reload: &broadcast::Sender<String>,
    headers: &HeaderMap,
    body: Bytes,
) -> Response {
    let Some(text) = paste_text(headers, &body) else {
        return bad_request("expected text/plain UTF-8 paste body");
    };
    if body.len() > MAX_PASTE_BYTES {
        return too_large();
    }
    let source = gist_header(headers, GIST_ID_HEADER);
    let name = gist_header(headers, GIST_NAME_HEADER);
    let paste = match store.save(source, text, name) {
        Ok(paste) => paste,
        Err(err) => {
            warn!("gist save failed: {err}");
            return bad_request("invalid or duplicate gist name");
        }
    };
    let _ = reload.send("gist".to_string());
    Json(summary(&paste)).into_response()
}

fn rename_inner(
    store: &crate::gist::Store,
    reload: &broadcast::Sender<String>,
    id: &str,
    headers: &HeaderMap,
    body: &Bytes,
) -> Response {
    let Some(name) = paste_text(headers, body) else {
        return bad_request("expected text/plain UTF-8 gist name");
    };
    let paste = match store.rename(id, name) {
        Ok(paste) => paste,
        Err(err) => {
            warn!("gist rename failed: {err}");
            return bad_request("invalid or duplicate gist name");
        }
    };
    let _ = reload.send("gist".to_string());
    Json(rename_summary(&paste)).into_response()
}

fn paste_text<'a>(headers: &HeaderMap, body: &'a Bytes) -> Option<&'a str> {
    if !is_text_plain(headers) {
        return None;
    }
    std::str::from_utf8(body).ok()
}

fn gist_header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

fn is_text_plain(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.split(';').next().unwrap_or("").trim())
        .is_some_and(|value| value.eq_ignore_ascii_case("text/plain"))
}

fn summary(paste: &crate::gist::Paste) -> PasteSummary {
    PasteSummary {
        id: paste.id.clone(),
        name: format!("{}.txt", paste.id),
        raw_url: RAW_HREF,
        bytes: paste.body.len(),
        lines: paste.body.lines().count(),
    }
}

fn rename_summary(paste: &crate::gist::Paste) -> RenameSummary {
    RenameSummary {
        id: paste.id.clone(),
        name: format!("{}.txt", paste.id),
        href: format!("{GIST_HREF}/p/{}", paste.id),
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
        .body(Body::from("paste body too large"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    fn headers(content_type: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, content_type.parse().unwrap());
        headers
    }

    fn named_headers(name: &str) -> HeaderMap {
        let mut headers = headers("text/plain; charset=utf-8");
        headers.insert(GIST_NAME_HEADER, name.parse().unwrap());
        headers
    }

    #[test]
    fn text_plain_accepts_charset() {
        assert!(is_text_plain(&headers("text/plain; charset=utf-8")));
        assert!(is_text_plain(&headers("TEXT/PLAIN")));
    }

    #[test]
    fn text_plain_rejects_other_content_types() {
        assert!(!is_text_plain(&HeaderMap::new()));
        assert!(!is_text_plain(&headers("application/json")));
    }

    #[test]
    fn paste_text_rejects_invalid_utf8() {
        let body = Bytes::from_static(&[0xff]);

        assert!(paste_text(&headers("text/plain"), &body).is_none());
    }

    #[test]
    fn create_writes_paste_and_broadcasts_event() {
        let td = TempDir::new("ghrm-gist-http");
        let store = crate::gist::Store::from_root(td.path().join("gist")).unwrap();
        let (tx, mut rx) = broadcast::channel(4);

        let response = create_inner(
            &store,
            &tx,
            &headers("text/plain; charset=utf-8"),
            Bytes::from_static(b"hello\n"),
        );

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(store.current().unwrap().unwrap().body, "hello\n");
        assert_eq!(rx.try_recv().unwrap(), "gist");
    }

    #[test]
    fn create_uses_requested_name() {
        let td = TempDir::new("ghrm-gist-named-http");
        let store = crate::gist::Store::from_root(td.path().join("gist")).unwrap();
        let (tx, _) = broadcast::channel(4);

        let response = create_inner(
            &store,
            &tx,
            &named_headers("notes"),
            Bytes::from_static(b"hello\n"),
        );

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(store.current().unwrap().unwrap().id, "notes");
        assert!(store.root().join("notes.txt").is_file());
    }

    #[test]
    fn rename_updates_paste_name() {
        let td = TempDir::new("ghrm-gist-rename-http");
        let store = crate::gist::Store::from_root(td.path().join("gist")).unwrap();
        let (tx, _) = broadcast::channel(4);
        create_inner(
            &store,
            &tx,
            &named_headers("before"),
            Bytes::from_static(b"hello\n"),
        );

        let response = rename_inner(
            &store,
            &tx,
            "before",
            &headers("text/plain; charset=utf-8"),
            &Bytes::from_static(b"after.txt"),
        );

        assert_eq!(response.status(), StatusCode::OK);
        assert!(!store.root().join("before.txt").exists());
        assert!(store.root().join("after.txt").is_file());
        assert_eq!(store.current().unwrap().unwrap().id, "after");
    }

    #[test]
    fn delete_removes_paste_and_broadcasts_event() {
        let td = TempDir::new("ghrm-gist-delete-http");
        let store = crate::gist::Store::from_root(td.path().join("gist")).unwrap();
        let (tx, mut rx) = broadcast::channel(4);
        create_inner(
            &store,
            &tx,
            &named_headers("deleteme"),
            Bytes::from_static(b"hello\n"),
        );
        rx.try_recv().unwrap();

        let response = delete_inner(&store, &tx, "deleteme");

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert!(!store.root().join("deleteme.txt").exists());
        assert!(store.current().unwrap().is_none());
        assert_eq!(rx.try_recv().unwrap(), "gist");
    }

    #[test]
    fn delete_missing_paste_returns_not_found() {
        let td = TempDir::new("ghrm-gist-delete-missing");
        let store = crate::gist::Store::from_root(td.path().join("gist")).unwrap();
        let (tx, _) = broadcast::channel(4);

        let response = delete_inner(&store, &tx, "nonexistent");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn create_rejects_large_body() {
        let td = TempDir::new("ghrm-gist-large");
        let store = crate::gist::Store::from_root(td.path().join("gist")).unwrap();
        let (tx, _) = broadcast::channel(4);
        let body = Bytes::from(vec![b'a'; MAX_PASTE_BYTES + 1]);

        let response = create_inner(&store, &tx, &headers("text/plain"), body);

        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert!(store.current().unwrap().is_none());
    }
}
