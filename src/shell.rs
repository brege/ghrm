use crate::render::Rendered;
use crate::repo::SourceState;
use crate::runtime;
use crate::tmpl::{self, PageShell};
use crate::vendor;

use axum::{
    body::Body,
    http::{HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Response},
};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use tracing::warn;

pub(crate) const PROJECT_URL: &str = "https://github.com/brege/ghrm";

pub(crate) fn full_page(
    r: &Rendered,
    body: &str,
    source: SourceState,
    show_logout: bool,
    runtime_paths: &runtime::Paths,
) -> Response {
    let title = if r.title.is_empty() {
        "Preview"
    } else {
        &r.title
    };
    let source = source_html(&source);
    let project_version = env!("CARGO_PKG_VERSION");
    let project_release_href = format!("{PROJECT_URL}/releases/tag/v{project_version}");
    let assets = vendor::plan(r);
    let shell = PageShell {
        title,
        body,
        source: &source,
        project_href: PROJECT_URL,
        project_release_href: &project_release_href,
        project_version,
        show_logout,
        asset_json: vendor::client_json(),
        vendor_styles: &assets.styles,
        vendor_scripts: &assets.scripts,
        runtime_paths: runtime_paths.rows(),
    };
    let html = match tmpl::base(shell) {
        Ok(h) => h,
        Err(e) => {
            warn!("template error: {}", e);
            return not_found();
        }
    };
    let mut res = Html(html).into_response();
    res.headers_mut()
        .insert(header::VARY, HeaderValue::from_static("HX-Request"));
    res
}

pub(crate) fn fragment(body: &str, title: &str, source: SourceState) -> Response {
    let source_oob = source_oob_html(&source);
    let html = format!("{body}{source_oob}");
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::VARY, "HX-Request")
        .header("HX-Title", hx_title(title))
        .body(Body::from(html))
        .unwrap()
}

pub(crate) fn redirect(location: &str) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header("HX-Redirect", location)
        .header(header::VARY, "HX-Request")
        .body(Body::empty())
        .unwrap()
}

fn hx_title(title: &str) -> String {
    utf8_percent_encode(title, NON_ALPHANUMERIC).to_string()
}

pub(crate) fn source_html(source: &SourceState) -> String {
    source_html_inner(source, false)
}

pub(crate) fn source_oob_html(source: &SourceState) -> String {
    source_html_inner(source, true)
}

fn source_html_inner(source: &SourceState, oob: bool) -> String {
    let oob_attr = if oob { " hx-swap-oob=\"true\"" } else { "" };
    match source {
        SourceState::Web { url, label, .. } => web_source_html(url, label, oob_attr),
        SourceState::Transport { raw } => format!(
            "<span id=\"ghrm-source-slot\"{oob_attr} class=\"ghrm-source-link is-muted\" aria-label=\"Transport-only remote\" title=\"Transport-only remote: {raw}\">{badge}<span class=\"ghrm-source-text\">{text}</span></span>",
            raw = html_escape::encode_double_quoted_attribute(raw),
            badge = status_badge_html(),
            text = html_escape::encode_text(raw),
        ),
        SourceState::NoRemote => format!(
            "<span id=\"ghrm-source-slot\"{oob_attr} class=\"ghrm-source-link is-muted\" aria-label=\"Git repository has no remote\" title=\"Git repository has no remote\">{badge}<span class=\"ghrm-source-text\">no remote</span></span>",
            badge = status_badge_html(),
        ),
        SourceState::NoRepo => project_source_html(oob_attr),
    }
}

fn status_badge_html() -> &'static str {
    "<button type=\"button\" class=\"ghrm-source-badge\" aria-expanded=\"false\" aria-controls=\"ghrm-about-peek\" aria-label=\"Show ghrm status\" title=\"Show ghrm status\"><span class=\"ghrm-status-dot\" aria-hidden=\"true\"></span></button>"
}

fn project_source_html(oob_attr: &str) -> String {
    format!(
        "<span id=\"ghrm-source-slot\"{oob_attr} class=\"ghrm-source-link is-muted\">{badge}<span class=\"ghrm-source-text\"><span class=\"ghrm-source-repo\">ghrm</span></span></span>",
        badge = status_badge_html(),
    )
}

fn web_source_html(url: &str, label: &str, oob_attr: &str) -> String {
    let href = html_escape::encode_double_quoted_attribute(url);
    let title_attr = html_escape::encode_double_quoted_attribute(url);
    let (host, repo) = source_display(url, label);
    let host_href = if host.is_empty() {
        None
    } else {
        Some(format!("https://{host}"))
    };
    let host = html_escape::encode_text(&host);
    let repo = html_escape::encode_text(&repo);

    let host_html = match host_href {
        Some(host_href) => {
            let host_href = html_escape::encode_double_quoted_attribute(&host_href);
            format!(
                "<a class=\"ghrm-source-host\" href=\"{host_href}\" target=\"_blank\" rel=\"noopener noreferrer\" title=\"Open {host}\">{host}</a>"
            )
        }
        None => String::new(),
    };

    format!(
        "<span id=\"ghrm-source-slot\"{oob_attr} class=\"ghrm-source-link is-muted\">{badge}<span class=\"ghrm-source-text\">{host_html}<a class=\"ghrm-source-repo\" href=\"{href}\" target=\"_blank\" rel=\"noopener noreferrer\" aria-label=\"Open source remote: {title_attr}\" title=\"Open source remote: {title_attr}\">{repo}</a></span></span>",
        badge = status_badge_html(),
    )
}

fn source_display(url: &str, label: &str) -> (String, String) {
    let after_scheme = url.find("://").map_or(0, |i| i + 3);
    let host_end = after_scheme
        + url[after_scheme..]
            .find('/')
            .unwrap_or(url.len() - after_scheme);
    let host = url[after_scheme..host_end].trim_end_matches('/');
    let repo = url[host_end..].trim_matches('/');
    if host.is_empty() || repo.is_empty() {
        let repo = label.replace(" / ", "/");
        return (String::new(), repo);
    }
    (host.to_string(), repo.to_string())
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
    fn source_display_splits_host_and_repo() {
        let (host, repo) = source_display("https://github.com/brege/ghrm", "brege / ghrm");
        assert_eq!(host, "github.com");
        assert_eq!(repo, "brege/ghrm");
    }

    #[test]
    fn source_oob_includes_swap_attribute() {
        let html = source_oob_html(&SourceState::NoRepo);
        assert!(html.contains("hx-swap-oob=\"true\""));
        assert!(html.contains("id=\"ghrm-source-slot\""));
    }

    #[test]
    fn source_html_omits_oob_attribute() {
        let html = source_html(&SourceState::NoRepo);
        assert!(!html.contains("hx-swap-oob"));
        assert!(html.contains("id=\"ghrm-source-slot\""));
    }

    #[test]
    fn fragment_response_varies_on_hx_request() {
        let response = fragment("body", "Test", SourceState::NoRepo);
        assert_eq!(response.headers().get(header::VARY).unwrap(), "HX-Request");
        assert_eq!(response.headers().get("HX-Title").unwrap(), "Test");
    }

    #[test]
    fn fragment_response_encodes_title_header() {
        let response = fragment("body", "Test Title\nλ", SourceState::NoRepo);
        assert_eq!(
            response.headers().get("HX-Title").unwrap(),
            "Test%20Title%0A%CE%BB"
        );
    }
}
