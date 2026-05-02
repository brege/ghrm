use crate::column;
use crate::render;
use crate::repo::RepoSet;
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
use std::path::{Path, PathBuf};
use tracing::warn;

#[derive(Serialize)]
struct TreeResponse {
    mode: &'static str,
    root: String,
    ready: bool,
    dirs: BTreeMap<String, crate::walk::NavDir>,
}

pub(crate) async fn tree(
    State(s): State<AppState>,
    RawQuery(raw_query): RawQuery,
    Query(q): Query<ViewQuery>,
) -> Response {
    let view = view::from_query(&q, raw_query.as_deref(), &s.view_cfg, &s.filters);
    let matcher = view::matcher(&view, &s.filters);
    let tree = if view.use_ignore == s.use_ignore {
        s.cached_nav_tree(&view, matcher.as_ref())
    } else {
        Some(s.nav_tree(&view, matcher.as_ref()))
    };
    let root = s
        .target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let resp = TreeResponse {
        mode: if s.mode == Mode::Dir { "dir" } else { "file" },
        root,
        ready: tree.is_some(),
        dirs: tree.map(|tree| tree.dirs.clone()).unwrap_or_default(),
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
    cells: Vec<column::Cell>,
}

#[derive(Serialize)]
struct PathSearchResponse {
    results: Vec<PathSearchResult>,
    truncated: bool,
    max_rows: usize,
    pending: bool,
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
    let load_lines = view.sort == walk::Sort::Lines
        || column::required_meta(&view.columns).contains(column::MetaReq::LINES);
    let load_commit_meta = column::required_meta(&view.columns).contains(column::MetaReq::COMMIT);
    let rows = if view.use_ignore == s.use_ignore {
        let nav = s.nav.read().unwrap();
        if !nav.is_ready() {
            return json_response(
                &PathSearchResponse {
                    results: Vec::new(),
                    truncated: false,
                    max_rows: s.search_max_rows,
                    pending: true,
                },
                "api_path_search",
            );
        }
        walk::path_search_nav(
            &nav,
            walk::NavPathSearchSpec {
                current_path,
                query,
                max_rows: s.search_max_rows,
                sort: view.sort,
                dir: view.sort_dir,
                opts: view.opts,
                matcher: matcher.as_ref(),
                load_lines,
                load_commit_meta,
            },
            |rows| load_path_search_commits(rows, Some(&s.target), Some(&s.repos)),
        )
    } else {
        let mut guard = s.alternate_nav.write().unwrap();
        if guard.is_none() {
            *guard = Some(walk::build_all(
                &s.target,
                view.use_ignore,
                &s.exclude_names,
                &s.filter_exts,
                s.no_excludes,
            ));
        }
        walk::path_search_nav(
            guard.as_ref().unwrap(),
            walk::NavPathSearchSpec {
                current_path,
                query,
                max_rows: s.search_max_rows,
                sort: view.sort,
                dir: view.sort_dir,
                opts: view.opts,
                matcher: matcher.as_ref(),
                load_lines,
                load_commit_meta,
            },
            |rows| load_path_search_commits(rows, Some(&s.target), Some(&s.repos)),
        )
    };
    let resp = path_search_response(rows, &view.columns);

    json_response(&resp, "api_path_search")
}

#[cfg(test)]
struct PathSearchSpec<'a> {
    tree: &'a walk::NavTree,
    current_path: &'a str,
    query: &'a str,
    max_rows: usize,
    sort: walk::Sort,
    dir: walk::SortDir,
    columns: &'a column::Set,
    target: Option<&'a Path>,
    repos: Option<&'a RepoSet>,
}

#[cfg(test)]
fn path_search_results(spec: PathSearchSpec<'_>) -> PathSearchResponse {
    let rows = walk::path_search(
        walk::PathSearchSpec {
            tree: spec.tree,
            current_path: spec.current_path,
            query: spec.query,
            max_rows: spec.max_rows,
            sort: spec.sort,
            dir: spec.dir,
            load_commit_meta: column::required_meta(spec.columns).contains(column::MetaReq::COMMIT),
        },
        |rows| load_path_search_commits(rows, spec.target, spec.repos),
    );
    path_search_response(rows, spec.columns)
}

fn load_path_search_commits(
    rows: &mut [walk::PathSearchRow],
    target: Option<&Path>,
    repos: Option<&RepoSet>,
) {
    let (Some(target), Some(repos)) = (target, repos) else {
        return;
    };
    let paths = rows
        .iter()
        .map(|row| path_search_abs(target, &row.href))
        .collect::<Vec<_>>();
    let commits = repos.commit_info(&paths);
    for (row, path) in rows.iter_mut().zip(paths) {
        if let Some(commit) = commits.get(&path) {
            row.commit_subject = Some(commit.subject.clone());
            row.commit_timestamp = Some(commit.timestamp);
        }
    }
}

fn path_search_abs(target: &Path, href: &str) -> PathBuf {
    target.join(href.trim_matches('/'))
}

fn path_search_response(rows: walk::PathSearchRows, columns: &column::Set) -> PathSearchResponse {
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
            PathSearchResult {
                href: row.href,
                display: row.display,
                is_dir: row.is_dir,
                cells,
            }
        })
        .collect();
    PathSearchResponse {
        results,
        truncated: rows.truncated,
        max_rows: rows.max_rows,
        pending: false,
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
    use crate::testutil::{TempDir, nav_entry};
    use std::fs;

    #[test]
    fn path_search_uses_selected_sort() {
        let td = TempDir::new("ghrm-path-search");
        fs::write(td.path().join("newer.md"), "one\ntwo\nthree\n").unwrap();
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
                        size: Some(2048),
                        lines: Some(3),
                        ..nav_entry("newer.md", false, Some(9))
                    },
                ],
                readme: None,
            },
        );
        let tree = walk::NavTree { dirs };
        let resp = path_search_results(PathSearchSpec {
            tree: &tree,
            current_path: "",
            query: "m",
            max_rows: 10,
            sort: walk::Sort::Size,
            dir: walk::SortDir::Desc,
            columns: &column::Set::from_defaults(|def| def.key == "date" || def.key == "size"),
            target: None,
            repos: None,
        });
        let date_cell = resp.results[0]
            .cells
            .iter()
            .find(|cell| cell.key == "date")
            .unwrap();
        assert_eq!(date_cell.timestamp, Some(9));
        assert!(!date_cell.hidden);

        let size_cell = resp.results[0]
            .cells
            .iter()
            .find(|cell| cell.key == "size")
            .unwrap();
        assert_eq!(size_cell.text.as_deref(), Some("2.0 KB"));
        assert!(!size_cell.hidden);

        let line_cell = resp.results[0]
            .cells
            .iter()
            .find(|cell| cell.key == "lines")
            .unwrap();
        assert_eq!(line_cell.text.as_deref(), Some("3"));
        assert!(line_cell.hidden);

        let commit_cell = resp.results[0]
            .cells
            .iter()
            .find(|cell| cell.key == "commit")
            .unwrap();
        assert!(commit_cell.hidden);

        let names: Vec<_> = resp.results.into_iter().map(|row| row.display).collect();
        assert_eq!(names, vec!["newer.md", "older.md"]);
    }

    #[test]
    fn path_search_truncates_after_ranking() {
        let mut dirs = BTreeMap::new();
        dirs.insert(
            String::new(),
            walk::NavDir {
                entries: vec![
                    walk::NavEntry {
                        href: "/match-small.md".to_string(),
                        size: Some(1),
                        ..nav_entry("match-small.md", false, Some(1))
                    },
                    walk::NavEntry {
                        href: "/match-large.md".to_string(),
                        size: Some(3000),
                        ..nav_entry("match-large.md", false, Some(1))
                    },
                    walk::NavEntry {
                        href: "/match-mid.md".to_string(),
                        size: Some(2000),
                        ..nav_entry("match-mid.md", false, Some(1))
                    },
                ],
                readme: None,
            },
        );
        let tree = walk::NavTree { dirs };

        let resp = path_search_results(PathSearchSpec {
            tree: &tree,
            current_path: "",
            query: "match",
            max_rows: 2,
            sort: walk::Sort::Size,
            dir: walk::SortDir::Desc,
            columns: &column::Set::from_defaults(|_| false),
            target: None,
            repos: None,
        });

        assert!(resp.truncated);
        let names: Vec<_> = resp.results.into_iter().map(|row| row.display).collect();
        assert_eq!(names, vec!["match-large.md", "match-mid.md"]);
    }
}
