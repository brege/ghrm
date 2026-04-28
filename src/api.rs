use crate::column;
use crate::render;
use crate::search as content_search;
use crate::server::{AppState, Mode};
use crate::view::{self, ViewQuery};
use crate::walk;

use axum::{
    body::Body,
    extract::{Query, RawQuery, State},
    http::{StatusCode, header},
    response::Response,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use tracing::warn;

#[derive(Serialize)]
struct TreeResponse {
    mode: &'static str,
    root: String,
    dirs: BTreeMap<String, crate::walk::NavDir>,
}

pub(crate) async fn tree(
    State(s): State<AppState>,
    RawQuery(raw_query): RawQuery,
    Query(q): Query<ViewQuery>,
) -> Response {
    let nav = s.nav.read().unwrap();
    let view = view::from_query(&q, raw_query.as_deref(), &s.view_cfg, &s.filters);
    let matcher = view::matcher(&view, &s.filters);
    let tree = if view.use_ignore == s.use_ignore {
        nav.get(view.opts, view.sort, view.sort_dir, matcher.as_ref())
    } else {
        drop(nav);
        s.nav_tree(&view, matcher.as_ref())
    };
    let root = s
        .target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let resp = TreeResponse {
        mode: if s.mode == Mode::Dir { "dir" } else { "file" },
        root,
        dirs: tree.dirs.clone(),
    };
    json_response(&resp, "api_tree")
}

#[derive(Deserialize)]
pub(crate) struct SearchQuery {
    q: Option<String>,
    hidden: Option<u8>,
    excludes: Option<u8>,
    ignore: Option<u8>,
    filter: Option<u8>,
    sort: Option<String>,
    dir: Option<String>,
}

pub(crate) async fn search(
    State(s): State<AppState>,
    RawQuery(raw_query): RawQuery,
    Query(q): Query<SearchQuery>,
) -> Response {
    let query = match q.q.as_deref() {
        Some(q) if !q.is_empty() => q,
        _ => return bad_request(r#"{"error":"missing query"}"#),
    };

    let view = view::from_query(
        &ViewQuery {
            hidden: q.hidden.map(|value| value.to_string()),
            excludes: q.excludes.map(|value| value.to_string()),
            ignore: q.ignore.map(|value| value.to_string()),
            filter: q.filter.map(|value| value.to_string()),
            sort: q.sort.clone(),
            dir: q.dir.clone(),
            extra: Default::default(),
        },
        raw_query.as_deref(),
        &s.view_cfg,
        &s.filters,
    );
    let exclude_names = if view.opts.show_excludes {
        &[][..]
    } else {
        &s.exclude_names
    };
    let matcher = view::matcher(&view, &s.filters);
    let filter_exts = view::filter_exts(&view, &s.filter_exts);

    let resp = content_search::search(content_search::SearchOpts {
        query,
        root: &s.target,
        use_ignore: view.use_ignore,
        hidden: view.opts.show_hidden,
        exclude_names,
        filter_exts,
        group_filter: matcher.as_ref(),
        max_rows: s.search_max_rows,
    });

    json_response(&resp, "api_search")
}

#[derive(Serialize)]
struct PathSearchResult {
    href: String,
    display: String,
    is_dir: bool,
    modified: Option<u64>,
    commit_message: Option<String>,
    commit_date: Option<u64>,
}

#[derive(Serialize)]
struct PathSearchResponse {
    results: Vec<PathSearchResult>,
    truncated: bool,
    max_rows: usize,
}

#[derive(Default, Deserialize)]
pub(crate) struct PathSearchQuery {
    q: Option<String>,
    path: Option<String>,
    #[serde(flatten)]
    view: ViewQuery,
}

pub(crate) async fn path_search(
    State(s): State<AppState>,
    RawQuery(raw_query): RawQuery,
    Query(q): Query<PathSearchQuery>,
) -> Response {
    let query = match q.q.as_deref() {
        Some(q) if !q.is_empty() => q,
        _ => return bad_request(r#"{"error":"missing query"}"#),
    };

    let view = view::from_query(&q.view, raw_query.as_deref(), &s.view_cfg, &s.filters);
    let current_path = q.path.as_deref().unwrap_or("").trim_matches('/');
    let matcher = view::matcher(&view, &s.filters);
    let tree = s.nav_tree(&view, matcher.as_ref());
    let mut resp = path_search_results(
        &tree,
        current_path,
        query,
        s.search_max_rows,
        view.sort,
        view.sort_dir,
    );
    if view.columns.is_visible(column::Id::CommitMessage)
        || view.columns.is_visible(column::Id::CommitDate)
    {
        let paths: Vec<_> = resp
            .results
            .iter()
            .map(|row| row.href.trim_matches('/'))
            .map(|rel| s.target.join(rel))
            .collect();
        let commits = s.repos.commit_info(&paths);
        for (row, path) in resp.results.iter_mut().zip(paths) {
            if let Some(commit) = commits.get(&path) {
                row.commit_message = Some(commit.subject.clone());
                row.commit_date = Some(commit.timestamp);
            }
        }
    }

    json_response(&resp, "api_path_search")
}

fn path_search_results(
    tree: &walk::NavTree,
    current_path: &str,
    query: &str,
    max_rows: usize,
    sort: walk::Sort,
    dir: walk::SortDir,
) -> PathSearchResponse {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return PathSearchResponse {
            results: Vec::new(),
            truncated: false,
            max_rows,
        };
    }

    let prefix = (!current_path.is_empty()).then(|| format!("{current_path}/"));
    let mut rows: Vec<PathSearchResult> = Vec::new();

    for (dir, nav_dir) in &tree.dirs {
        if let Some(prefix) = &prefix {
            if dir != current_path && !dir.starts_with(prefix) {
                continue;
            }
        }

        let rel_dir = if dir == current_path {
            ""
        } else if let Some(prefix) = &prefix {
            dir.strip_prefix(prefix).unwrap_or(dir.as_str())
        } else {
            dir.as_str()
        };

        for entry in &nav_dir.entries {
            let rel_path = if rel_dir.is_empty() {
                entry.name.clone()
            } else {
                format!("{rel_dir}/{}", entry.name)
            };
            if !rel_path.to_lowercase().contains(&needle) {
                continue;
            }
            rows.push(PathSearchResult {
                href: entry.href.clone(),
                display: if entry.is_dir {
                    format!("{rel_path}/")
                } else {
                    rel_path
                },
                is_dir: entry.is_dir,
                modified: entry.modified,
                commit_message: None,
                commit_date: None,
            });
        }
    }

    rows.sort_by(|a, b| {
        let a_name = a.display.to_lowercase();
        let b_name = b.display.to_lowercase();
        let a_base = a_name
            .rsplit('/')
            .next()
            .is_some_and(|name| name.contains(&needle)) as u8;
        let b_base = b_name
            .rsplit('/')
            .next()
            .is_some_and(|name| name.contains(&needle)) as u8;
        b_base
            .cmp(&a_base)
            .then_with(|| cmp_path_rows(a, b, sort, dir))
    });

    let truncated = rows.len() > max_rows;
    rows.truncate(max_rows);

    PathSearchResponse {
        results: rows,
        truncated,
        max_rows,
    }
}

fn cmp_path_rows(
    a: &PathSearchResult,
    b: &PathSearchResult,
    sort: walk::Sort,
    dir: walk::SortDir,
) -> std::cmp::Ordering {
    match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => match sort {
            walk::Sort::Name => {
                apply_path_dir(a.display.to_lowercase().cmp(&b.display.to_lowercase()), dir)
            }
            walk::Sort::Type => apply_path_dir(
                path_ext(&a.display)
                    .cmp(&path_ext(&b.display))
                    .then_with(|| a.display.to_lowercase().cmp(&b.display.to_lowercase())),
                dir,
            ),
            walk::Sort::Timestamp => apply_path_dir(
                a.modified
                    .cmp(&b.modified)
                    .then_with(|| a.display.to_lowercase().cmp(&b.display.to_lowercase())),
                dir,
            ),
        },
    }
}

fn path_ext(display: &str) -> String {
    Path::new(display.trim_end_matches('/'))
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_lowercase()
}

fn apply_path_dir(order: std::cmp::Ordering, dir: walk::SortDir) -> std::cmp::Ordering {
    match dir {
        walk::SortDir::Asc => order,
        walk::SortDir::Desc => order.reverse(),
    }
}

#[derive(Deserialize)]
pub(crate) struct RenderQuery {
    path: Option<String>,
}

pub(crate) async fn render(State(s): State<AppState>, Query(q): Query<RenderQuery>) -> Response {
    let rel = q.path.as_deref().unwrap_or("").trim_matches('/');

    let (file_path, root) = if s.mode == Mode::File {
        let parent = s.target.parent().unwrap_or(&s.target).to_path_buf();
        let fp = if rel.is_empty() {
            s.target.clone()
        } else {
            parent.join(rel)
        };
        (fp, parent)
    } else {
        let fp = if rel.is_empty() {
            s.target.clone()
        } else {
            s.target.join(rel)
        };
        (fp, s.target.clone())
    };

    let md = match tokio::fs::read_to_string(&file_path).await {
        Ok(m) => m,
        Err(_) => return not_found(),
    };

    let rendered = render::render_at(
        &md,
        Some(render::RenderPath {
            root: &root,
            src: &file_path,
        }),
    );

    json_response(&rendered, "api_render")
}

fn json_response<T: Serialize>(value: &T, label: &str) -> Response {
    match serde_json::to_string(value) {
        Ok(json) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json))
            .unwrap(),
        Err(e) => {
            warn!("{label} error: {}", e);
            not_found()
        }
    }
}

fn bad_request(body: &'static str) -> Response {
    Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap()
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
    use crate::testutil::nav_entry;

    #[test]
    fn path_search_uses_selected_sort() {
        let mut dirs = BTreeMap::new();
        dirs.insert(
            String::new(),
            walk::NavDir {
                entries: vec![
                    walk::NavEntry {
                        href: "/src/".to_string(),
                        ..nav_entry("src", true, Some(3))
                    },
                    walk::NavEntry {
                        href: "/older.md".to_string(),
                        ..nav_entry("older.md", false, Some(1))
                    },
                    walk::NavEntry {
                        href: "/newer.md".to_string(),
                        ..nav_entry("newer.md", false, Some(9))
                    },
                ],
                readme: None,
            },
        );
        let tree = walk::NavTree { dirs };

        let resp = path_search_results(
            &tree,
            "",
            "m",
            10,
            walk::Sort::Timestamp,
            walk::SortDir::Desc,
        );
        let names: Vec<_> = resp.results.into_iter().map(|row| row.display).collect();
        assert_eq!(names, vec!["newer.md", "older.md"]);
    }
}
