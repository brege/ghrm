use crate::http::vendor;
use crate::render::Rendered;
use crate::repo::SourceState;
use crate::runtime;
use crate::tmpl::{self, AboutPeek, AboutSource, PageShell};

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
    let about = about_html(&source, runtime_paths, false);
    let source = source_html(&source);
    let assets = vendor::plan(r);
    let shell = PageShell {
        title,
        body,
        source: &source,
        about: &about,
        show_logout,
        asset_json: vendor::client_json(),
        vendor_styles: &assets.styles,
        vendor_scripts: &assets.scripts,
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

pub(crate) fn fragment(
    body: &str,
    title: &str,
    source: SourceState,
    runtime_paths: &runtime::Paths,
) -> Response {
    let source_oob = source_oob_html(&source);
    let about_oob = about_html(&source, runtime_paths, true);
    let html = format!("{body}{source_oob}{about_oob}");
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

fn about_html(source: &SourceState, runtime_paths: &runtime::Paths, oob: bool) -> String {
    let project_version = env!("CARGO_PKG_VERSION");
    let project_release_href = format!("{PROJECT_URL}/releases/tag/v{project_version}");
    let source = about_source(source);
    let about = AboutPeek {
        oob,
        runtime_paths: runtime_paths.rows(),
        source: &source,
        project_href: PROJECT_URL,
        project_release_href: &project_release_href,
        project_version,
    };
    match tmpl::about(about) {
        Ok(html) => html,
        Err(e) => {
            warn!("about template error: {}", e);
            String::new()
        }
    }
}

fn about_source(source: &SourceState) -> AboutSource {
    match source {
        SourceState::Web { url, raw, .. } => AboutSource {
            label: "remote",
            value: raw.clone(),
            href: Some(url.clone()),
            title: format!("Open source remote: {url}"),
        },
        SourceState::Transport { raw } => AboutSource {
            label: "remote",
            value: raw.clone(),
            href: None,
            title: format!("Transport-only remote: {raw}"),
        },
        SourceState::NoRemote => AboutSource {
            label: "source",
            value: "git repo / no remote".to_string(),
            href: None,
            title: "Git repository has no remote".to_string(),
        },
        SourceState::NoRepo => AboutSource {
            label: "source",
            value: "local path".to_string(),
            href: None,
            title: "Local path".to_string(),
        },
    }
}

fn source_html_inner(source: &SourceState, oob: bool) -> String {
    let oob_attr = if oob { " hx-swap-oob=\"true\"" } else { "" };
    match source {
        SourceState::Web { url, raw, .. } => linked_source_html(url, raw, oob_attr),
        SourceState::Transport { raw } => plain_source_html(
            "Transport-only remote",
            raw,
            &format!("Transport-only remote: {raw}"),
            oob_attr,
        ),
        SourceState::NoRemote => plain_source_html(
            "Git repository has no remote",
            "git repo / no remote",
            "Git repository has no remote",
            oob_attr,
        ),
        SourceState::NoRepo => {
            plain_source_html("Local path", "local path", "Local path", oob_attr)
        }
    }
}

fn status_badge_html() -> &'static str {
    "<button type=\"button\" class=\"ghrm-source-badge\" aria-expanded=\"false\" aria-controls=\"ghrm-about-peek\" aria-label=\"Show ghrm status\" title=\"Show ghrm status\"><span class=\"ghrm-status-dot\" aria-hidden=\"true\"></span></button>"
}

fn plain_source_html(aria: &str, text: &str, title: &str, oob_attr: &str) -> String {
    let aria = html_escape::encode_double_quoted_attribute(aria);
    let title = html_escape::encode_double_quoted_attribute(title);
    let text = html_escape::encode_text(text);
    format!(
        "<span id=\"ghrm-source-slot\"{oob_attr} class=\"ghrm-source-link is-muted\" aria-label=\"{aria}\" title=\"{title}\">{badge}<span class=\"ghrm-source-text\"><span class=\"ghrm-source-value\">{text}</span></span></span>",
        badge = status_badge_html(),
    )
}

fn linked_source_html(url: &str, raw: &str, oob_attr: &str) -> String {
    let href = html_escape::encode_double_quoted_attribute(url);
    let title_attr = html_escape::encode_double_quoted_attribute(url);
    let text = html_escape::encode_text(raw);

    format!(
        "<span id=\"ghrm-source-slot\"{oob_attr} class=\"ghrm-source-link is-muted\">{badge}<span class=\"ghrm-source-text\"><a class=\"ghrm-source-value\" href=\"{href}\" target=\"_blank\" rel=\"noopener noreferrer\" aria-label=\"Open source remote: {title_attr}\" title=\"Open source remote: {title_attr}\">{text}</a></span></span>",
        badge = status_badge_html(),
    )
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
    use crate::testutil::TempDir;

    fn test_runtime_paths() -> runtime::Paths {
        let td = TempDir::new("ghrm-shell-runtime-paths");
        runtime::Paths::new(td.path(), None).unwrap()
    }

    #[test]
    fn web_source_displays_configured_remote() {
        let html = source_html(&SourceState::Web {
            url: "https://github.com/brege/ghrm".to_string(),
            raw: "git@github.com:brege/ghrm.git".to_string(),
            forge: crate::repo::Forge::GitHub,
        });
        assert!(html.contains("href=\"https://github.com/brege/ghrm\""));
        assert!(html.contains(">git@github.com:brege/ghrm.git</a>"));
        assert!(!html.contains("github.com/brege/ghrm</a>"));
    }

    #[test]
    fn no_remote_source_is_descriptive() {
        let html = source_html(&SourceState::NoRemote);
        assert!(html.contains("git repo / no remote"));
    }

    #[test]
    fn no_repo_source_is_local_path() {
        let html = source_html(&SourceState::NoRepo);
        assert!(html.contains("local path"));
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
    fn about_html_renders_current_source() {
        let runtime_paths = test_runtime_paths();
        let html = about_html(
            &SourceState::Web {
                url: "https://github.com/brege/ghrm".to_string(),
                raw: "git@github.com:brege/ghrm.git".to_string(),
                forge: crate::repo::Forge::GitHub,
            },
            &runtime_paths,
            false,
        );
        assert!(html.contains("Current Source"));
        assert!(html.contains(">git@github.com:brege/ghrm.git</a>"));
        assert!(html.contains(">brege/ghrm</span>"));
    }

    #[test]
    fn about_oob_includes_swap_attribute() {
        let runtime_paths = test_runtime_paths();
        let html = about_html(&SourceState::NoRepo, &runtime_paths, true);
        assert!(html.contains("id=\"ghrm-about-peek\""));
        assert!(html.contains("hx-swap-oob=\"true\""));
    }

    #[test]
    fn fragment_response_varies_on_hx_request() {
        let runtime_paths = test_runtime_paths();
        let response = fragment("body", "Test", SourceState::NoRepo, &runtime_paths);
        assert_eq!(response.headers().get(header::VARY).unwrap(), "HX-Request");
        assert_eq!(response.headers().get("HX-Title").unwrap(), "Test");
    }

    #[test]
    fn fragment_response_encodes_title_header() {
        let runtime_paths = test_runtime_paths();
        let response = fragment("body", "Test Title\nλ", SourceState::NoRepo, &runtime_paths);
        assert_eq!(
            response.headers().get("HX-Title").unwrap(),
            "Test%20Title%0A%CE%BB"
        );
    }
}
