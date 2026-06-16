use crate::explorer::column;
use crate::explorer::view::{self, ViewConfig, ViewState};
use crate::tmpl::{self, ContentSearchCtx, ContentSearchRow, PathSearchCtx, PathSearchRow};

use axum::{
    body::Body,
    http::{StatusCode, header},
    response::Response,
};
use serde::Serialize;
use tracing::warn;

const CONTENT_SNIPPET_MAX: usize = 88;

#[derive(Serialize)]
pub(crate) struct PathResult {
    pub(crate) href: String,
    pub(crate) display: String,
    pub(crate) is_dir: bool,
    pub(crate) cells: Vec<column::Cell>,
}

#[derive(Serialize)]
pub(crate) struct PathResponse {
    pub(crate) results: Vec<PathResult>,
    pub(crate) truncated: bool,
    pub(crate) max_rows: usize,
    pub(crate) pending: bool,
}

impl PathResponse {
    pub(crate) fn pending(max_rows: usize) -> Self {
        Self {
            results: Vec::new(),
            truncated: false,
            max_rows,
            pending: true,
        }
    }
}

pub(crate) fn path_response(rows: super::path::Rows, columns: &column::Set) -> PathResponse {
    let results = rows
        .rows
        .into_iter()
        .map(|row| {
            let cells = column::RowMeta {
                modified: row.modified,
                size: row.size,
                lines: row.lines,
                commit_subject: row.commit_subject.as_deref(),
                commit_author: row.commit_author.as_deref(),
                commit_timestamp: row.commit_timestamp,
            }
            .cells(columns);
            PathResult {
                href: row.href,
                display: row.display,
                is_dir: row.is_dir,
                cells,
            }
        })
        .collect();
    PathResponse {
        results,
        truncated: rows.truncated,
        max_rows: rows.max_rows,
        pending: false,
    }
}

pub(crate) fn path_fragment(
    resp: &PathResponse,
    query: &str,
    view: &ViewState,
    cfg: &ViewConfig,
) -> Option<Response> {
    let rows = resp
        .results
        .iter()
        .map(|row| PathSearchRow {
            href: view::with_view(&row.href, view, cfg),
            html: highlight_match(&row.display, query),
            is_dir: row.is_dir,
            cells: &row.cells,
        })
        .collect::<Vec<_>>();
    let body = match tmpl::path_search(PathSearchCtx {
        pending: resp.pending,
        rows: &rows,
        empty_colspan: view.columns.visible_len() + 2,
    }) {
        Ok(body) => body,
        Err(e) => {
            warn!("path search template error: {}", e);
            return None;
        }
    };
    Some(html_response(
        body,
        rows.len(),
        resp.truncated,
        resp.pending,
        resp.max_rows,
    ))
}

pub(crate) fn content_fragment(
    resp: &super::SearchResponse,
    view: &ViewState,
    cfg: &ViewConfig,
    scope_prefix: Option<&str>,
) -> Option<Response> {
    let rows = resp
        .results
        .iter()
        .map(|row| ContentSearchRow {
            href: view::with_view(&format!("/{}", row.path), view, cfg),
            path: scoped_display_path(&row.path, scope_prefix),
            line: row.line,
            html: format_content_snippet(&row.text, &row.ranges),
            modified: row.modified,
        })
        .collect::<Vec<_>>();
    let body = match tmpl::content_search(ContentSearchCtx {
        rows: &rows,
        truncated: resp.truncated,
        max_rows: resp.max_rows,
        empty_colspan: column::DEFS.len() + 2,
        content_colspan: content_colspan(),
        summary_colspan: column::DEFS.len() + 1,
    }) {
        Ok(body) => body,
        Err(e) => {
            warn!("content search template error: {}", e);
            return None;
        }
    };
    Some(html_response(
        body,
        rows.len(),
        resp.truncated,
        false,
        resp.max_rows,
    ))
}

fn scoped_display_path(path: &str, scope_prefix: Option<&str>) -> String {
    scope_prefix
        .and_then(|prefix| path.strip_prefix(prefix))
        .map(|s| s.trim_start_matches('/').to_string())
        .unwrap_or_else(|| path.to_string())
}

fn content_colspan() -> usize {
    column::DEFS
        .iter()
        .position(|def| def.key == "date")
        .map_or(column::DEFS.len() + 1, |idx| idx + 1)
}

fn highlight_match(value: &str, query: &str) -> String {
    let lower = value.to_ascii_lowercase();
    let needle = query.to_ascii_lowercase();
    let mut start = 0;
    let mut out = String::new();

    while let Some(offset) = lower[start..].find(&needle) {
        let idx = start + offset;
        out.push_str(&html_escape::encode_text(&value[start..idx]));
        out.push_str(r#"<strong class="ghrm-search-hit">"#);
        out.push_str(&html_escape::encode_text(&value[idx..idx + needle.len()]));
        out.push_str("</strong>");
        start = idx + needle.len();
    }

    out.push_str(&html_escape::encode_text(&value[start..]));
    out
}

fn format_content_snippet(text: &str, ranges: &[(usize, usize)]) -> String {
    let window = content_window(text, ranges);
    let mut out = String::new();
    if window.prefix {
        out.push_str("... ");
    }
    out.push_str(&highlight_ranges(window.text, &window.ranges));
    if window.suffix {
        out.push_str(" ...");
    }
    out
}

struct ContentWindow<'a> {
    text: &'a str,
    ranges: Vec<(usize, usize)>,
    prefix: bool,
    suffix: bool,
}

fn content_window<'a>(text: &'a str, ranges: &[(usize, usize)]) -> ContentWindow<'a> {
    if text.len() <= CONTENT_SNIPPET_MAX {
        return ContentWindow {
            text,
            ranges: ranges.to_vec(),
            prefix: false,
            suffix: false,
        };
    }
    let center = ranges
        .first()
        .map(|(start, end)| start + ((end - start) / 2))
        .unwrap_or(CONTENT_SNIPPET_MAX / 2);
    let raw_start = center.saturating_sub(CONTENT_SNIPPET_MAX / 2);
    let raw_end = (raw_start + CONTENT_SNIPPET_MAX).min(text.len());
    let start = char_boundary_before(text, raw_start);
    let end = char_boundary_before(text, raw_end);
    let clipped = ranges
        .iter()
        .filter_map(|(range_start, range_end)| {
            if *range_end <= start || *range_start >= end {
                None
            } else {
                Some((
                    range_start.saturating_sub(start),
                    (*range_end).min(end) - start,
                ))
            }
        })
        .collect();

    ContentWindow {
        text: &text[start..end],
        ranges: clipped,
        prefix: start > 0,
        suffix: end < text.len(),
    }
}

fn char_boundary_before(text: &str, idx: usize) -> usize {
    let mut idx = idx.min(text.len());
    while !text.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn highlight_ranges(text: &str, ranges: &[(usize, usize)]) -> String {
    let mut out = String::new();
    let mut pos = 0;
    for (start, end) in ranges {
        let start = char_boundary_before(text, *start);
        let end = char_boundary_before(text, *end);
        if start > pos {
            out.push_str(&html_escape::encode_text(&text[pos..start]));
        }
        out.push_str("<mark>");
        out.push_str(&html_escape::encode_text(&text[start..end]));
        out.push_str("</mark>");
        pos = end;
    }
    out.push_str(&html_escape::encode_text(&text[pos..]));
    out
}

fn html_response(
    body: String,
    count: usize,
    truncated: bool,
    pending: bool,
    max_rows: usize,
) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::VARY, "HX-Request, Accept")
        .header("X-Ghrm-Search-Count", count.to_string())
        .header("X-Ghrm-Search-Truncated", if truncated { "1" } else { "0" })
        .header("X-Ghrm-Search-Pending", if pending { "1" } else { "0" })
        .header("X-Ghrm-Search-Max-Rows", max_rows.to_string())
        .body(Body::from(body))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::explorer::walk::{Sort, ViewOpts};
    use axum::body::to_bytes;

    async fn response_text(response: Response) -> String {
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    fn test_columns() -> column::Set {
        column::Set::from_defaults(|def| def.default_visible)
    }

    fn test_view_config() -> ViewConfig {
        ViewConfig {
            default: ViewOpts::default(),
            default_use_ignore: true,
            default_groups: Vec::new(),
            default_sort: Sort::Name,
            default_columns: test_columns(),
            can_toggle_excludes: false,
        }
    }

    fn test_view_state(cfg: &ViewConfig) -> ViewState {
        ViewState {
            opts: cfg.default,
            use_ignore: cfg.default_use_ignore,
            groups: Vec::new(),
            sort: cfg.default_sort,
            sort_dir: cfg.default_sort.default_dir(),
            columns: cfg.default_columns.clone(),
            show_headers: false,
        }
    }

    #[test]
    fn highlight_match_wraps_query() {
        let result = highlight_match("src/main.rs", "main");
        assert!(result.contains(r#"<strong class="ghrm-search-hit">main</strong>"#));
    }

    #[test]
    fn highlight_match_case_insensitive() {
        let result = highlight_match("README.md", "readme");
        assert!(result.contains(r#"<strong class="ghrm-search-hit">README</strong>"#));
    }

    #[test]
    fn highlight_match_escapes_html() {
        let result = highlight_match("<script>test</script>", "test");
        assert!(result.contains("&lt;script&gt;"));
        assert!(result.contains("&lt;/script&gt;"));
        assert!(result.contains(r#"<strong class="ghrm-search-hit">test</strong>"#));
    }

    #[test]
    fn highlight_match_multiple_occurrences() {
        let result = highlight_match("test/test.rs", "test");
        let count = result
            .matches(r#"<strong class="ghrm-search-hit">"#)
            .count();
        assert_eq!(count, 2);
    }

    #[test]
    fn highlight_match_no_match() {
        let result = highlight_match("src/lib.rs", "xyz");
        assert!(!result.contains("<strong"));
        assert_eq!(result, "src/lib.rs");
    }

    #[test]
    fn highlight_ranges_marks_positions() {
        let result = highlight_ranges("hello world", &[(0, 5)]);
        assert_eq!(result, "<mark>hello</mark> world");
    }

    #[test]
    fn highlight_ranges_multiple() {
        let result = highlight_ranges("foo bar baz", &[(0, 3), (8, 11)]);
        assert_eq!(result, "<mark>foo</mark> bar <mark>baz</mark>");
    }

    #[test]
    fn highlight_ranges_escapes_html() {
        let result = highlight_ranges("<tag>", &[(1, 4)]);
        assert!(result.contains("&lt;"));
        assert!(result.contains("<mark>tag</mark>"));
        assert!(result.contains("&gt;"));
    }

    #[test]
    fn highlight_ranges_empty() {
        let result = highlight_ranges("unchanged", &[]);
        assert_eq!(result, "unchanged");
    }

    #[test]
    fn content_window_short_text_unchanged() {
        let text = "short line";
        let window = content_window(text, &[(0, 5)]);
        assert_eq!(window.text, text);
        assert_eq!(window.ranges, vec![(0, 5)]);
        assert!(!window.prefix);
        assert!(!window.suffix);
    }

    #[test]
    fn content_window_long_text_clips() {
        let text = "a".repeat(200);
        let window = content_window(&text, &[(100, 105)]);
        assert!(window.text.len() <= CONTENT_SNIPPET_MAX);
        assert!(window.prefix || window.suffix);
    }

    #[test]
    fn content_window_adjusts_ranges() {
        let text = "a".repeat(200);
        let window = content_window(&text, &[(100, 105)]);
        assert!(!window.ranges.is_empty());
        for (start, end) in &window.ranges {
            assert!(*start < window.text.len());
            assert!(*end <= window.text.len());
        }
    }

    #[test]
    fn content_window_range_outside_window_filtered() {
        let text = "a".repeat(200);
        let window = content_window(&text, &[(0, 5)]);
        let has_range_in_window = window.ranges.iter().any(|(s, e)| *e > *s);
        if window.prefix {
            assert!(window.ranges.is_empty() || has_range_in_window);
        }
    }

    #[test]
    fn path_response_pending_state() {
        let resp = PathResponse::pending(50);
        assert!(resp.pending);
        assert!(resp.results.is_empty());
        assert!(!resp.truncated);
        assert_eq!(resp.max_rows, 50);
    }

    #[test]
    fn path_response_converts_rows() {
        let columns = column::Set::from_defaults(|def| def.default_visible);
        let rows = super::super::path::Rows {
            rows: vec![super::super::path::Row {
                href: "/file.rs".to_string(),
                display: "file.rs".to_string(),
                is_dir: false,
                modified: Some(1700000000),
                size: Some(1024),
                lines: Some(100),
                commit_subject: Some("test commit".to_string()),
                commit_author: Some("Test Author".to_string()),
                commit_timestamp: Some(1700000000),
            }],
            truncated: true,
            max_rows: 100,
        };
        let resp = path_response(rows, &columns);

        assert!(!resp.pending);
        assert!(resp.truncated);
        assert_eq!(resp.max_rows, 100);
        assert_eq!(resp.results.len(), 1);
        assert_eq!(resp.results[0].href, "/file.rs");
        assert_eq!(resp.results[0].display, "file.rs");
        assert!(!resp.results[0].is_dir);
    }

    #[test]
    fn html_response_sets_headers() {
        let resp = html_response("body".to_string(), 5, false, false, 100);
        assert_eq!(resp.headers().get("X-Ghrm-Search-Count").unwrap(), "5");
        assert_eq!(resp.headers().get("X-Ghrm-Search-Truncated").unwrap(), "0");
        assert_eq!(resp.headers().get("X-Ghrm-Search-Pending").unwrap(), "0");
        assert_eq!(resp.headers().get("X-Ghrm-Search-Max-Rows").unwrap(), "100");
        assert_eq!(
            resp.headers().get("Content-Type").unwrap(),
            "text/html; charset=utf-8"
        );
    }

    #[test]
    fn html_response_truncated_header() {
        let resp = html_response("body".to_string(), 100, true, false, 100);
        assert_eq!(resp.headers().get("X-Ghrm-Search-Truncated").unwrap(), "1");
    }

    #[test]
    fn html_response_pending_header() {
        let resp = html_response("body".to_string(), 0, false, true, 50);
        assert_eq!(resp.headers().get("X-Ghrm-Search-Pending").unwrap(), "1");
        assert_eq!(resp.headers().get("X-Ghrm-Search-Max-Rows").unwrap(), "50");
    }

    #[test]
    fn scoped_display_path_strips_prefix() {
        assert_eq!(
            scoped_display_path("src/lib/utils.rs", Some("src/lib")),
            "utils.rs"
        );
    }

    #[test]
    fn scoped_display_path_no_prefix() {
        assert_eq!(scoped_display_path("src/main.rs", None), "src/main.rs");
    }

    #[test]
    fn scoped_display_path_prefix_mismatch() {
        assert_eq!(
            scoped_display_path("tests/main.rs", Some("src")),
            "tests/main.rs"
        );
    }

    #[test]
    fn content_colspan_finds_date_column() {
        let colspan = content_colspan();
        assert!(colspan > 0);
        assert!(colspan <= column::DEFS.len() + 1);
    }

    #[test]
    fn format_content_snippet_short() {
        let result = format_content_snippet("short text", &[(6, 10)]);
        assert!(result.contains("<mark>text</mark>"));
        assert!(!result.contains("..."));
    }

    #[test]
    fn format_content_snippet_long_adds_ellipsis() {
        let long_text = "a".repeat(200);
        let result = format_content_snippet(&long_text, &[(100, 105)]);
        assert!(result.contains("..."));
    }

    #[tokio::test]
    async fn path_fragment_returns_some_with_rows() {
        let cfg = test_view_config();
        let view = test_view_state(&cfg);
        let resp = PathResponse {
            results: vec![PathResult {
                href: "/file.rs".to_string(),
                display: "file.rs".to_string(),
                is_dir: false,
                cells: Vec::new(),
            }],
            truncated: false,
            max_rows: 100,
            pending: false,
        };

        let result = path_fragment(&resp, "file", &view, &cfg);

        assert!(result.is_some(), "path_fragment must return Some");
        let response = result.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get("X-Ghrm-Search-Count").unwrap(), "1");
        let body = response_text(response).await;
        assert!(body.contains("ghrm-search-path"));
        assert!(body.contains(r#"<strong class="ghrm-search-hit">file</strong>.rs"#));
    }

    #[tokio::test]
    async fn path_fragment_pending_returns_some() {
        let cfg = test_view_config();
        let view = test_view_state(&cfg);
        let resp = PathResponse::pending(50);

        let result = path_fragment(&resp, "q", &view, &cfg);

        assert!(result.is_some(), "pending path_fragment must return Some");
        let response = result.unwrap();
        assert_eq!(
            response.headers().get("X-Ghrm-Search-Pending").unwrap(),
            "1"
        );
        let body = response_text(response).await;
        assert!(body.contains("Indexing paths"));
    }

    #[tokio::test]
    async fn content_fragment_returns_some_with_rows() {
        use super::super::{SearchResponse, SearchResult};
        let cfg = test_view_config();
        let view = test_view_state(&cfg);
        let resp = SearchResponse {
            results: vec![SearchResult {
                path: "src/main.rs".to_string(),
                line: 10,
                text: "fn main() {}".to_string(),
                ranges: vec![(3, 7)],
                modified: Some(1700000000),
            }],
            truncated: false,
            max_rows: 100,
        };

        let result = content_fragment(&resp, &view, &cfg, None);

        assert!(result.is_some(), "content_fragment must return Some");
        let response = result.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get("X-Ghrm-Search-Count").unwrap(), "1");
        let body = response_text(response).await;
        assert!(body.contains("ghrm-content-result"));
        assert!(body.contains("src/main.rs"));
        assert!(body.contains("<mark>main</mark>"));
    }

    #[tokio::test]
    async fn content_fragment_scoped_display_path() {
        use super::super::{SearchResponse, SearchResult};
        let cfg = test_view_config();
        let view = test_view_state(&cfg);
        let resp = SearchResponse {
            results: vec![SearchResult {
                path: "src/lib/utils.rs".to_string(),
                line: 5,
                text: "let x = 1;".to_string(),
                ranges: vec![(4, 5)],
                modified: None,
            }],
            truncated: false,
            max_rows: 100,
        };

        let result = content_fragment(&resp, &view, &cfg, Some("src/lib"));

        assert!(result.is_some());
        let response = result.unwrap();
        assert_eq!(response.headers().get("X-Ghrm-Search-Count").unwrap(), "1");
        let body = response_text(response).await;
        assert!(body.contains("utils.rs"));
        assert!(!body.contains(">src/lib/utils.rs<"));
    }

    #[test]
    fn content_colspan_value_reasonable() {
        let colspan = content_colspan();
        let date_idx = column::DEFS.iter().position(|def| def.key == "date");
        let expected = date_idx.map_or(column::DEFS.len() + 1, |idx| idx + 1);
        assert_eq!(colspan, expected);
    }
}
