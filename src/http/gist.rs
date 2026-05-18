use crate::http::delivery;
use crate::http::server::{AppState, HtmxContext};
use crate::http::shell;
use crate::render::Rendered;
use crate::repo::SourceState;
use crate::tmpl::{self, GistCtx};

use axum::{
    Json,
    body::{Body, Bytes},
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Serialize;
use tokio::sync::broadcast;
use tracing::warn;

const MAX_PASTE_BYTES: usize = 1024 * 1024;
const RAW_HREF: &str = "/_ghrm/gist/raw";

#[derive(Serialize)]
struct PasteSummary {
    id: String,
    raw_url: &'static str,
    bytes: usize,
    lines: usize,
}

pub(crate) async fn show(State(s): State<AppState>, headers: HeaderMap) -> Response {
    let Some(store) = s.gist.as_ref() else {
        return not_found();
    };
    let hx = HtmxContext::from_headers(&headers);
    let current = match store.current() {
        Ok(current) => current,
        Err(err) => {
            warn!("gist read failed: {err}");
            return server_error();
        }
    };
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
        raw_href: RAW_HREF,
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
        return shell::fragment(&body, title, SourceState::NoRepo);
    }

    let rendered = Rendered {
        html: String::new(),
        title: title.to_string(),
        lang: None,
        has_mermaid: false,
        has_math: false,
        has_map: false,
    };
    shell::full_page(
        &rendered,
        &body,
        SourceState::NoRepo,
        s.auth.is_some(),
        &s.runtime_paths,
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

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from(current.body))
        .unwrap()
}

pub(crate) async fn create(State(s): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    let Some(store) = s.gist.as_ref() else {
        return not_found();
    };
    create_inner(store, &s.reload, &headers, body)
}

fn create_inner(
    store: &crate::gist::Store,
    reload: &broadcast::Sender<&'static str>,
    headers: &HeaderMap,
    body: Bytes,
) -> Response {
    let Some(text) = paste_text(headers, &body) else {
        return bad_request("expected text/plain UTF-8 paste body");
    };
    if body.len() > MAX_PASTE_BYTES {
        return too_large();
    }
    let paste = match store.write(text) {
        Ok(paste) => paste,
        Err(err) => {
            warn!("gist write failed: {err}");
            return server_error();
        }
    };
    let _ = reload.send("gist");
    Json(summary(&paste)).into_response()
}

fn paste_text<'a>(headers: &HeaderMap, body: &'a Bytes) -> Option<&'a str> {
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

fn summary(paste: &crate::gist::Paste) -> PasteSummary {
    PasteSummary {
        id: paste.id.clone(),
        raw_url: RAW_HREF,
        bytes: paste.body.len(),
        lines: paste.body.lines().count(),
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
