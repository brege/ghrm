use crate::column;
use crate::search as content_search;
use crate::tmpl::{self, ContentSearchCtx, ContentSearchRow, PathSearchCtx, PathSearchRow};
use crate::view::{self, ViewConfig, ViewState};
use crate::walk;

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

pub(crate) fn path_response(rows: walk::PathSearchRows, columns: &column::Set) -> PathResponse {
    let results = rows
        .rows
        .into_iter()
        .map(|row| {
            let cells = column::RowMeta {
                modified: row.modified,
                size: row.size,
                lines: row.lines,
                commit_subject: row.commit_subject.as_deref(),
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
    resp: &content_search::SearchResponse,
    view: &ViewState,
    cfg: &ViewConfig,
) -> Option<Response> {
    let rows = resp
        .results
        .iter()
        .map(|row| ContentSearchRow {
            href: view::with_view(&format!("/{}", row.path), view, cfg),
            path: row.path.clone(),
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
