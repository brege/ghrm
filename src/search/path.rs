use crate::walk::{self, NavSet, Sort, SortDir, ViewOpts};

use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};
use std::path::Path;

#[derive(Clone, Debug)]
pub(crate) struct Row {
    pub(crate) href: String,
    pub(crate) display: String,
    pub(crate) is_dir: bool,
    pub(crate) modified: Option<u64>,
    pub(crate) size: Option<u64>,
    pub(crate) lines: Option<u64>,
    pub(crate) commit_subject: Option<String>,
    pub(crate) commit_timestamp: Option<u64>,
}

pub(crate) struct Rows {
    pub(crate) rows: Vec<Row>,
    pub(crate) truncated: bool,
    pub(crate) max_rows: usize,
}

#[cfg(test)]
pub(crate) struct TreeSpec<'a> {
    pub(crate) tree: &'a walk::NavTree,
    pub(crate) current_path: &'a str,
    pub(crate) query: &'a str,
    pub(crate) max_rows: usize,
    pub(crate) sort: Sort,
    pub(crate) dir: SortDir,
    pub(crate) load_commit_meta: bool,
}

pub(crate) struct NavSpec<'a> {
    pub(crate) current_path: &'a str,
    pub(crate) query: &'a str,
    pub(crate) max_rows: usize,
    pub(crate) sort: Sort,
    pub(crate) dir: SortDir,
    pub(crate) opts: ViewOpts,
    pub(crate) matcher: Option<&'a crate::filter::Matcher>,
    pub(crate) load_lines: bool,
    pub(crate) load_commit_meta: bool,
}

#[cfg(test)]
pub(crate) fn tree(spec: TreeSpec<'_>, mut load_commits: impl FnMut(&mut [Row])) -> Rows {
    let needle = spec.query.trim().to_lowercase();
    if needle.is_empty() {
        return empty(spec.max_rows);
    }

    if matches!(spec.sort, Sort::CommitMessage | Sort::CommitDate) {
        let mut rows = Vec::new();
        visit_tree_rows(&spec, &needle, |row| rows.push(row));
        load_commits(&mut rows);
        let mut rows = sort_rows(rows, &needle, spec.sort, spec.dir);
        let truncated = rows.len() > spec.max_rows;
        rows.truncate(spec.max_rows);
        return Rows {
            rows,
            truncated,
            max_rows: spec.max_rows,
        };
    }

    let mut selector = Selector::new(spec.max_rows, &needle, spec.sort, spec.dir);
    visit_tree_rows(&spec, &needle, |row| selector.push(row));
    let (mut rows, truncated) = selector.finish();
    if spec.load_commit_meta {
        load_commits(&mut rows);
    }
    Rows {
        rows,
        truncated,
        max_rows: spec.max_rows,
    }
}

pub(crate) fn nav(
    nav: &NavSet,
    spec: NavSpec<'_>,
    mut load_commits: impl FnMut(&mut [Row]),
) -> Rows {
    let needle = spec.query.trim().to_lowercase();
    if needle.is_empty() {
        return empty(spec.max_rows);
    }

    if matches!(spec.sort, Sort::CommitMessage | Sort::CommitDate) {
        let mut rows = Vec::new();
        visit_nav_rows(nav, &spec, &needle, |row| rows.push(row));
        load_commits(&mut rows);
        let mut rows = sort_rows(rows, &needle, spec.sort, spec.dir);
        let truncated = rows.len() > spec.max_rows;
        rows.truncate(spec.max_rows);
        return Rows {
            rows,
            truncated,
            max_rows: spec.max_rows,
        };
    }

    let mut selector = Selector::new(spec.max_rows, &needle, spec.sort, spec.dir);
    visit_nav_rows(nav, &spec, &needle, |row| selector.push(row));
    let (mut rows, truncated) = selector.finish();
    if spec.load_commit_meta {
        load_commits(&mut rows);
    }
    Rows {
        rows,
        truncated,
        max_rows: spec.max_rows,
    }
}

fn empty(max_rows: usize) -> Rows {
    Rows {
        rows: Vec::new(),
        truncated: false,
        max_rows,
    }
}

#[cfg(test)]
fn visit_tree_rows(spec: &TreeSpec<'_>, needle: &str, mut visit: impl FnMut(Row)) {
    let prefix = (!spec.current_path.is_empty()).then(|| format!("{}/", spec.current_path));
    for (dir, nav_dir) in &spec.tree.dirs {
        if let Some(prefix) = &prefix {
            if dir != spec.current_path && !dir.starts_with(prefix) {
                continue;
            }
        }

        let rel_dir = if dir == spec.current_path {
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
            if !rel_path.to_lowercase().contains(needle) {
                continue;
            }
            visit(Row {
                href: entry.href.clone(),
                display: if entry.is_dir {
                    format!("{rel_path}/")
                } else {
                    rel_path
                },
                is_dir: entry.is_dir,
                modified: entry.modified,
                size: entry.size,
                lines: entry.lines,
                commit_subject: None,
                commit_timestamp: None,
            });
        }
    }
}

fn visit_nav_rows(nav: &NavSet, spec: &NavSpec<'_>, needle: &str, mut visit: impl FnMut(Row)) {
    let load_lines = spec.load_lines || spec.sort == Sort::Lines;
    let prune_empty = spec.opts.filter_ext;
    let dirs_with_files = if prune_empty {
        nav.dirs_with_files(spec.matcher, spec.opts)
    } else {
        HashSet::new()
    };
    let prefix = (!spec.current_path.is_empty()).then(|| format!("{}/", spec.current_path));

    for entry in nav.entries() {
        if !nav.allow_path(entry.path, spec.opts) {
            continue;
        }
        if prune_empty {
            if entry.is_dir {
                if !dirs_with_files.contains(entry.path) {
                    continue;
                }
            } else if !nav.matches_filter(entry.path, spec.matcher) {
                continue;
            }
        }

        let (display, display_lower) = if let Some(prefix) = &prefix {
            let Some(display) = entry.key.strip_prefix(prefix) else {
                continue;
            };
            (display, Cow::Owned(display.to_lowercase()))
        } else {
            (entry.key, Cow::Borrowed(entry.key_lower))
        };
        if !display_lower.contains(needle) {
            continue;
        }

        visit(Row {
            href: if entry.is_dir {
                walk::dir_href(entry.path)
            } else {
                walk::file_href(entry.path)
            },
            display: if entry.is_dir {
                format!("{display}/")
            } else {
                display.to_string()
            },
            is_dir: entry.is_dir,
            modified: entry.modified,
            size: entry.size,
            lines: if load_lines && !entry.is_dir {
                walk::line_count(&nav.root().join(entry.path), entry.size)
            } else {
                None
            },
            commit_subject: None,
            commit_timestamp: None,
        });
    }
}

fn sort_rows(rows: Vec<Row>, needle: &str, sort: Sort, dir: SortDir) -> Vec<Row> {
    let mut ranked = rows
        .into_iter()
        .map(|row| RankedRow::new(row, needle, sort, dir))
        .collect::<Vec<_>>();
    ranked.sort();
    ranked.into_iter().map(|ranked| ranked.row).collect()
}

struct Selector<'a> {
    heap: BinaryHeap<RankedRow>,
    count: usize,
    max_rows: usize,
    needle: &'a str,
    sort: Sort,
    dir: SortDir,
}

impl<'a> Selector<'a> {
    fn new(max_rows: usize, needle: &'a str, sort: Sort, dir: SortDir) -> Self {
        Self {
            heap: BinaryHeap::with_capacity(max_rows),
            count: 0,
            max_rows,
            needle,
            sort,
            dir,
        }
    }

    fn push(&mut self, row: Row) {
        self.count += 1;
        if self.max_rows == 0 {
            return;
        }

        let ranked = RankedRow::new(row, self.needle, self.sort, self.dir);
        if self.heap.len() < self.max_rows {
            self.heap.push(ranked);
            return;
        }
        if self
            .heap
            .peek()
            .is_some_and(|worst| ranked.cmp(worst) == Ordering::Less)
        {
            self.heap.pop();
            self.heap.push(ranked);
        }
    }

    fn finish(self) -> (Vec<Row>, bool) {
        let truncated = self.count > self.max_rows;
        let mut ranked = self.heap.into_vec();
        ranked.sort();
        (
            ranked.into_iter().map(|ranked| ranked.row).collect(),
            truncated,
        )
    }
}

struct RankedRow {
    row: Row,
    base_match: bool,
    display_lower: String,
    ext: String,
    commit_subject_lower: Option<String>,
    sort: Sort,
    dir: SortDir,
}

impl RankedRow {
    fn new(row: Row, needle: &str, sort: Sort, dir: SortDir) -> Self {
        let display_lower = row.display.to_lowercase();
        let base_match = display_lower
            .rsplit('/')
            .next()
            .is_some_and(|name| name.contains(needle));
        let ext = path_ext(&row.display);
        let commit_subject_lower = row
            .commit_subject
            .as_ref()
            .map(|subject| subject.to_lowercase());
        Self {
            row,
            base_match,
            display_lower,
            ext,
            commit_subject_lower,
            sort,
            dir,
        }
    }
}

impl PartialEq for RankedRow {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for RankedRow {}

impl PartialOrd for RankedRow {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RankedRow {
    fn cmp(&self, other: &Self) -> Ordering {
        cmp_rows(self, other, self.sort, self.dir)
    }
}

fn cmp_rows(a: &RankedRow, b: &RankedRow, sort: Sort, dir: SortDir) -> Ordering {
    b.base_match
        .cmp(&a.base_match)
        .then_with(|| dir_first(a.row.is_dir, b.row.is_dir))
        .then_with(|| match sort {
            Sort::Name => apply_dir(a.display_lower.cmp(&b.display_lower), dir),
            Sort::Type => apply_dir(
                a.ext
                    .cmp(&b.ext)
                    .then_with(|| a.display_lower.cmp(&b.display_lower)),
                dir,
            ),
            Sort::Timestamp => apply_dir(
                a.row
                    .modified
                    .cmp(&b.row.modified)
                    .then_with(|| a.display_lower.cmp(&b.display_lower)),
                dir,
            ),
            Sort::Size => apply_dir(
                a.row
                    .size
                    .cmp(&b.row.size)
                    .then_with(|| a.display_lower.cmp(&b.display_lower)),
                dir,
            ),
            Sort::Lines => apply_dir(
                a.row
                    .lines
                    .cmp(&b.row.lines)
                    .then_with(|| a.display_lower.cmp(&b.display_lower)),
                dir,
            ),
            Sort::CommitMessage => apply_dir(
                a.commit_subject_lower
                    .cmp(&b.commit_subject_lower)
                    .then_with(|| a.display_lower.cmp(&b.display_lower)),
                dir,
            ),
            Sort::CommitDate => apply_dir(
                a.row
                    .commit_timestamp
                    .cmp(&b.row.commit_timestamp)
                    .then_with(|| a.display_lower.cmp(&b.display_lower)),
                dir,
            ),
        })
}

fn path_ext(display: &str) -> String {
    Path::new(display.trim_end_matches('/'))
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_lowercase()
}

fn dir_first(a_is_dir: bool, b_is_dir: bool) -> Ordering {
    match (a_is_dir, b_is_dir) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => Ordering::Equal,
    }
}

fn apply_dir(order: Ordering, dir: SortDir) -> Ordering {
    match dir {
        SortDir::Asc => order,
        SortDir::Desc => order.reverse(),
    }
}
