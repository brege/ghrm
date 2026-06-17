use crate::http::{about, vendor};
use crate::render::Rendered;
use crate::repo::SourceState;
use crate::runtime;
use crate::tmpl::{self, AboutSidebar, AboutStats, PageShell};

use axum::{
    body::Body,
    http::{HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Response},
};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use tracing::warn;

pub(crate) fn full_page(
    r: &Rendered,
    body: &str,
    explorer_path: Option<&str>,
    source: SourceState,
    show_logout: bool,
    runtime_paths: &runtime::Paths,
    gist_active: bool,
) -> Response {
    let title = if r.title.is_empty() {
        "Preview"
    } else {
        &r.title
    };
    let stats = AboutStats::default();
    let about = about::html(runtime_paths, &stats, false);
    let (layout_class, sidebar) = match explorer_path {
        Some(path) => ("ghrm-layout-explorer", explorer_sidebar_html(path, false)),
        None => ("", String::new()),
    };
    let source = source_html(&source);
    let gist_nav = gist_nav_html(runtime_paths.has_gist(), gist_active, false);
    let assets = vendor::plan(r);
    let shell = PageShell {
        title,
        body,
        layout_class,
        source: &source,
        about: &about,
        sidebar: &sidebar,
        show_logout,
        gist_nav: &gist_nav,
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
    explorer_path: Option<&str>,
    source: SourceState,
    runtime_paths: &runtime::Paths,
    gist_active: bool,
) -> Response {
    let source_oob = source_oob_html(&source);
    let gist_oob = gist_nav_html(runtime_paths.has_gist(), gist_active, true);
    let sidebar_oob = match explorer_path {
        Some(path) => explorer_sidebar_html(path, true),
        None => hidden_sidebar_html(true),
    };
    let html = format!("{body}{source_oob}{gist_oob}{sidebar_oob}");
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

fn gist_nav_html(show_gist: bool, active: bool, oob: bool) -> String {
    let oob_attr = if oob {
        " hx-swap-oob=\"outerHTML\""
    } else {
        ""
    };
    if !show_gist {
        return format!("<span id=\"ghrm-gist-slot\"{oob_attr} hidden></span>");
    }
    let (id, href, label, icon, boost) = if active {
        (
            "ghrm-home-link",
            "/",
            "Home",
            "ghrm-icon-home",
            " hx-boost=\"false\"",
        )
    } else {
        (
            "ghrm-gist-link",
            "/_ghrm/gist",
            "Gist",
            "ghrm-icon-note",
            " hx-boost=\"false\"",
        )
    };
    format!(
        "<span id=\"ghrm-gist-slot\"{oob_attr}><a id=\"{id}\" href=\"{href}\"{boost} aria-label=\"{label}\" title=\"{label}\"><svg aria-hidden=\"true\" focusable=\"false\"><use href=\"/_ghrm/assets/js/icons.svg#{icon}\"></use></svg></a></span>"
    )
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

fn about_sidebar_href(current_path: &str) -> String {
    format!(
        "/_ghrm/about?sidebar=true&path={}",
        utf8_percent_encode(current_path, NON_ALPHANUMERIC)
    )
}

fn sidebar_shell(about_href: Option<&str>, oob: bool) -> String {
    let request_attrs = about_href.map_or_else(String::new, |href| {
        format!(
            " hx-get=\"{href}\" hx-trigger=\"load, ghrm:contentready from:document\" hx-target=\"this\" hx-swap=\"outerHTML\" hx-push-url=\"false\""
        )
    });
    let oob_attr = if oob {
        " hx-swap-oob=\"outerHTML\""
    } else {
        ""
    };
    format!(
        "<aside id=\"ghrm-sidebar\" class=\"ghrm-sidebar\" data-loaded=\"false\"{request_attrs}{oob_attr} hidden></aside>"
    )
}

fn explorer_sidebar_html(current_path: &str, oob: bool) -> String {
    let href = about_sidebar_href(current_path);
    sidebar_shell(Some(&href), oob)
}

fn hidden_sidebar_html(oob: bool) -> String {
    sidebar_shell(None, oob)
}

fn has_sidebar_stats(stats: &AboutStats) -> bool {
    !stats.metadata.is_empty()
        || !stats.history.is_empty()
        || !stats.languages.is_empty()
        || !stats.activity.is_empty()
}

pub(crate) fn sidebar_html(stats: &AboutStats, oob: bool) -> String {
    let sidebar = AboutSidebar {
        stats,
        has_stats: has_sidebar_stats(stats),
        oob,
    };
    match tmpl::sidebar(sidebar) {
        Ok(html) => html,
        Err(e) => {
            warn!("sidebar template error: {}", e);
            String::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    fn runtime_paths(show_gist: bool) -> runtime::Paths {
        let td = TempDir::new("ghrm-shell-runtime");
        let paths = runtime::Paths::new(td.path(), None).unwrap();
        if show_gist {
            paths.with_gist(Some(&td.path().join("gist")))
        } else {
            paths
        }
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
    fn fragment_response_varies_on_hx_request() {
        let paths = runtime_paths(false);
        let response = fragment("body", "Test", None, SourceState::NoRepo, &paths, false);
        assert_eq!(response.headers().get(header::VARY).unwrap(), "HX-Request");
        assert_eq!(response.headers().get("HX-Title").unwrap(), "Test");
    }

    #[test]
    fn fragment_response_encodes_title_header() {
        let paths = runtime_paths(false);
        let response = fragment(
            "body",
            "Test Title\nλ",
            None,
            SourceState::NoRepo,
            &paths,
            false,
        );
        assert_eq!(
            response.headers().get("HX-Title").unwrap(),
            "Test%20Title%0A%CE%BB"
        );
    }

    #[test]
    fn explorer_sidebar_request_encodes_current_path() {
        let html = explorer_sidebar_html("/docs/space name/", false);

        assert!(html.contains("id=\"ghrm-sidebar\""));
        assert!(
            html.contains("hx-get=\"/_ghrm/about?sidebar=true&path=%2Fdocs%2Fspace%20name%2F\"")
        );
        assert!(html.contains("hx-trigger=\"load, ghrm:contentready from:document\""));
        assert!(html.contains("hx-target=\"this\""));
        assert!(html.contains("hx-swap=\"outerHTML\""));
        assert!(html.contains("hx-push-url=\"false\""));
    }

    #[test]
    fn hidden_sidebar_oob_replaces_existing_sidebar() {
        let html = hidden_sidebar_html(true);

        assert!(html.contains("id=\"ghrm-sidebar\""));
        assert!(html.contains("hx-swap-oob=\"outerHTML\""));
        assert!(!html.contains("hx-get="));
    }

    #[test]
    fn gist_nav_uses_home_link_on_gist_pages() {
        let html = gist_nav_html(true, true, false);

        assert!(html.contains("id=\"ghrm-home-link\""));
        assert!(html.contains("href=\"/\""));
        assert!(html.contains("hx-boost=\"false\""));
        assert!(html.contains("href=\"/_ghrm/assets/js/icons.svg#ghrm-icon-home\""));
        assert!(!html.contains("id=\"ghrm-gist-link\""));
    }

    #[test]
    fn gist_nav_uses_gist_link_on_content_pages() {
        let html = gist_nav_html(true, false, false);

        assert!(html.contains("id=\"ghrm-gist-link\""));
        assert!(html.contains("href=\"/_ghrm/gist\""));
        assert!(html.contains("hx-boost=\"false\""));
        assert!(html.contains("href=\"/_ghrm/assets/js/icons.svg#ghrm-icon-note\""));
        assert!(!html.contains("id=\"ghrm-home-link\""));
    }

    #[test]
    fn gist_nav_oob_keeps_stable_slot() {
        let html = gist_nav_html(true, true, true);

        assert!(html.contains("id=\"ghrm-gist-slot\""));
        assert!(html.contains("hx-swap-oob=\"outerHTML\""));
    }
}
