use crate::paths;

use ignore::{WalkBuilder, WalkState};
use serde::Serialize;
use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;

const VIEW_COMBINATIONS: usize = 8;
const LINE_COUNT_MAX_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ViewOpts {
    pub show_hidden: bool,
    pub show_excludes: bool,
    pub filter_ext: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sort {
    #[default]
    Name,
    Type,
    Timestamp,
    Size,
    Lines,
    CommitMessage,
    CommitDate,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SortDef {
    pub(crate) sort: Sort,
    pub(crate) key: &'static str,
    pub(crate) label: &'static str,
    pub(crate) title: &'static str,
    pub(crate) default_dir: SortDir,
    pub(crate) column_key: Option<&'static str>,
}

pub(crate) const SORT_DEFS: &[SortDef] = &[
    SortDef {
        sort: Sort::Name,
        key: "name",
        label: "Sort by name",
        title: "Sort explorer entries by name",
        default_dir: SortDir::Asc,
        column_key: None,
    },
    SortDef {
        sort: Sort::Type,
        key: "type",
        label: "Sort by type",
        title: "Sort explorer entries by type",
        default_dir: SortDir::Asc,
        column_key: None,
    },
    SortDef {
        sort: Sort::Timestamp,
        key: "timestamp",
        label: "Sort by modified date",
        title: "Sort explorer entries by modified date",
        default_dir: SortDir::Desc,
        column_key: Some("date"),
    },
    SortDef {
        sort: Sort::Size,
        key: "size",
        label: "Sort by size",
        title: "Sort explorer entries by file size",
        default_dir: SortDir::Desc,
        column_key: Some("size"),
    },
    SortDef {
        sort: Sort::Lines,
        key: "lines",
        label: "Sort by lines",
        title: "Sort explorer entries by line count",
        default_dir: SortDir::Desc,
        column_key: Some("lines"),
    },
    SortDef {
        sort: Sort::CommitMessage,
        key: "commit",
        label: "Sort by commit message",
        title: "Sort explorer entries by commit message",
        default_dir: SortDir::Asc,
        column_key: Some("commit"),
    },
    SortDef {
        sort: Sort::CommitDate,
        key: "commit_date",
        label: "Sort by commit date",
        title: "Sort explorer entries by commit date",
        default_dir: SortDir::Desc,
        column_key: Some("commit_date"),
    },
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortDir {
    #[default]
    Asc,
    Desc,
}

impl Sort {
    pub fn as_str(self) -> &'static str {
        self.def().key
    }

    pub fn parse(raw: &str) -> Option<Self> {
        SORT_DEFS
            .iter()
            .find(|def| def.key == raw)
            .map(|def| def.sort)
    }

    pub fn default_dir(self) -> SortDir {
        self.def().default_dir
    }

    pub(crate) fn column_key(self) -> Option<&'static str> {
        self.def().column_key
    }

    fn def(self) -> &'static SortDef {
        SORT_DEFS
            .iter()
            .find(|def| def.sort == self)
            .expect("sort definition exists")
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SortSpec {
    pub sort: Sort,
    pub dir: SortDir,
}

pub struct ListSpec<'a> {
    pub use_ignore: bool,
    pub exclude_names: &'a [String],
    pub extensions: &'a [String],
    pub matcher: Option<&'a crate::filter::Matcher>,
    pub opts: ViewOpts,
    pub order: SortSpec,
}

struct TreeBuildSpec<'a> {
    exclude_names: &'a [String],
    extensions: &'a [String],
    matcher: Option<&'a crate::filter::Matcher>,
    opts: ViewOpts,
    order: SortSpec,
    load_lines: bool,
}

pub fn line_count(path: &Path, size: Option<u64>) -> Option<u64> {
    if size.is_some_and(|size| size > LINE_COUNT_MAX_BYTES) {
        return None;
    }
    if !crate::delivery::previews_text_sync(path) {
        return None;
    }

    let bytes = std::fs::read(path).ok()?;
    if bytes.contains(&0) {
        return None;
    }
    if bytes.is_empty() {
        return Some(0);
    }

    let newlines = bytes.iter().filter(|&&b| b == b'\n').count() as u64;
    Some(newlines + u64::from(bytes.last() != Some(&b'\n')))
}

impl SortDir {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "asc" => Some(Self::Asc),
            "desc" => Some(Self::Desc),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct NavEntry {
    pub name: String,
    pub href: String,
    pub is_dir: bool,
    pub modified: Option<u64>,
    pub size: Option<u64>,
    pub lines: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct NavDir {
    pub entries: Vec<NavEntry>,
    pub readme: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct NavTree {
    pub dirs: BTreeMap<String, NavDir>,
}

#[derive(Clone, Debug)]
pub struct PathSearchRow {
    pub href: String,
    pub display: String,
    pub is_dir: bool,
    pub modified: Option<u64>,
    pub size: Option<u64>,
    pub lines: Option<u64>,
    pub commit_subject: Option<String>,
    pub commit_timestamp: Option<u64>,
}

#[cfg(test)]
pub struct PathSearchSpec<'a> {
    pub tree: &'a NavTree,
    pub current_path: &'a str,
    pub query: &'a str,
    pub max_rows: usize,
    pub sort: Sort,
    pub dir: SortDir,
    pub load_commit_meta: bool,
}

pub struct NavPathSearchSpec<'a> {
    pub current_path: &'a str,
    pub query: &'a str,
    pub max_rows: usize,
    pub sort: Sort,
    pub dir: SortDir,
    pub opts: ViewOpts,
    pub matcher: Option<&'a crate::filter::Matcher>,
    pub load_lines: bool,
    pub load_commit_meta: bool,
}

pub struct PathSearchRows {
    pub rows: Vec<PathSearchRow>,
    pub truncated: bool,
    pub max_rows: usize,
}

#[derive(Clone, Debug)]
pub struct NavSet {
    trees: Arc<Mutex<BTreeMap<usize, Arc<NavTree>>>>,
    snapshot: Arc<Snapshot>,
    exclude_names: Arc<Vec<String>>,
    extensions: Arc<Vec<String>>,
    ready: bool,
}

impl NavSet {
    pub fn is_ready(&self) -> bool {
        self.ready
    }

    pub fn get(
        &self,
        opts: ViewOpts,
        sort: Sort,
        dir: SortDir,
        matcher: Option<&crate::filter::Matcher>,
        load_lines: bool,
    ) -> Arc<NavTree> {
        let load_lines = load_lines || sort == Sort::Lines;
        let key = tree_cache_key(opts, sort, dir, load_lines);
        if matcher.is_none() {
            if let Some(tree) = self.trees.lock().unwrap().get(&key).cloned() {
                return tree;
            }
        }
        let tree = Arc::new(build_tree(
            &self.snapshot,
            TreeBuildSpec {
                exclude_names: &self.exclude_names,
                extensions: &self.extensions,
                matcher,
                opts,
                order: SortSpec { sort, dir },
                load_lines,
            },
        ));
        if matcher.is_none() {
            let mut guard = self.trees.lock().unwrap();
            if guard.len() >= 4 {
                guard.clear();
            }
            return guard.entry(key).or_insert_with(|| tree.clone()).clone();
        }
        tree
    }
}

impl Default for NavSet {
    fn default() -> Self {
        Self {
            trees: Arc::new(Mutex::new(BTreeMap::new())),
            snapshot: Arc::new(Snapshot::default()),
            exclude_names: Arc::new(Vec::new()),
            extensions: Arc::new(Vec::new()),
            ready: false,
        }
    }
}

#[derive(Debug, Default)]
struct Snapshot {
    root: PathBuf,
    dirs: Vec<PathBuf>,
    direct_dirs: BTreeMap<PathBuf, Vec<PathBuf>>,
    direct_files: BTreeMap<PathBuf, Vec<PathBuf>>,
    files: Vec<PathBuf>,
    entries: Vec<SnapshotEntry>,
    modified: BTreeMap<PathBuf, u64>,
    sizes: BTreeMap<PathBuf, u64>,
}

#[derive(Debug)]
struct SnapshotEntry {
    path: PathBuf,
    key: String,
    key_lower: String,
    is_dir: bool,
    modified: Option<u64>,
    size: Option<u64>,
}

pub fn build_all(
    root: &Path,
    use_ignore: bool,
    exclude_names: &[String],
    extensions: &[String],
    no_excludes: bool,
) -> NavSet {
    let snapshot = Arc::new(scan(root, use_ignore, exclude_names, no_excludes));
    NavSet {
        trees: Arc::new(Mutex::new(BTreeMap::new())),
        snapshot,
        exclude_names: Arc::new(exclude_names.to_vec()),
        extensions: Arc::new(extensions.to_vec()),
        ready: true,
    }
}

fn scan(root: &Path, use_ignore: bool, exclude_names: &[String], no_excludes: bool) -> Snapshot {
    let root_buf = root.to_path_buf();
    let filter_excludes = exclude_names.to_vec();
    let check_excludes: Arc<Vec<String>> = Arc::new(exclude_names.to_vec());
    let dirs_seen: Arc<Mutex<HashSet<PathBuf>>> = Arc::new(Mutex::new({
        let mut s = HashSet::new();
        s.insert(PathBuf::new());
        s
    }));
    let direct_files: Arc<Mutex<BTreeMap<PathBuf, Vec<PathBuf>>>> =
        Arc::new(Mutex::new(BTreeMap::new()));
    let files: Arc<Mutex<Vec<PathBuf>>> = Arc::new(Mutex::new(Vec::new()));
    let modified: Arc<Mutex<BTreeMap<PathBuf, u64>>> = Arc::new(Mutex::new(BTreeMap::new()));
    let sizes: Arc<Mutex<BTreeMap<PathBuf, u64>>> = Arc::new(Mutex::new(BTreeMap::new()));

    let mut builder = WalkBuilder::new(&root_buf);
    builder
        .hidden(false)
        .follow_links(true)
        .same_file_system(true)
        .require_git(false)
        .git_ignore(use_ignore)
        .git_exclude(use_ignore)
        .git_global(use_ignore);

    if !no_excludes {
        builder.filter_entry(move |e| {
            paths::allowed_name(&e.file_name().to_string_lossy(), &filter_excludes)
        });
    }

    builder.build_parallel().run(|| {
        let root = root_buf.clone();
        let dirs_seen = dirs_seen.clone();
        let direct_files = direct_files.clone();
        let files = files.clone();
        let modified = modified.clone();
        let sizes = sizes.clone();
        let excludes = check_excludes.clone();
        Box::new(move |res| {
            let entry = match res {
                Ok(e) => e,
                Err(_) => return WalkState::Continue,
            };
            let path = entry.path();
            if path == root {
                return WalkState::Continue;
            }
            let Some(file_type) = entry.file_type() else {
                return WalkState::Continue;
            };
            let rel = match path.strip_prefix(&root) {
                Ok(r) => r.to_path_buf(),
                Err(_) => return WalkState::Continue,
            };
            let parent = rel.parent().unwrap_or(Path::new("")).to_path_buf();
            let metadata = entry.metadata().ok();
            let mtime = metadata
                .as_ref()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs());
            if let Some(ts) = mtime {
                modified.lock().unwrap().insert(rel.clone(), ts);
            }

            let name = entry.file_name().to_string_lossy();
            let is_excluded = no_excludes && !paths::allowed_name(&name, &excludes);

            {
                let mut guard = dirs_seen.lock().unwrap();
                guard.insert(parent.clone());
                if file_type.is_dir() {
                    guard.insert(rel);
                    if is_excluded {
                        return WalkState::Skip;
                    }
                    return WalkState::Continue;
                }
            }
            if file_type.is_file() {
                let size = metadata.as_ref().map(|m| m.len());
                if let Some(s) = size {
                    sizes.lock().unwrap().insert(rel.clone(), s);
                }
                files.lock().unwrap().push(rel.clone());
                direct_files
                    .lock()
                    .unwrap()
                    .entry(parent)
                    .or_default()
                    .push(rel);
            }
            WalkState::Continue
        })
    });

    let dirs_seen = Arc::try_unwrap(dirs_seen).unwrap().into_inner().unwrap();
    let mut direct_files = Arc::try_unwrap(direct_files).unwrap().into_inner().unwrap();
    let files = Arc::try_unwrap(files).unwrap().into_inner().unwrap();
    let modified = Arc::try_unwrap(modified).unwrap().into_inner().unwrap();
    let sizes = Arc::try_unwrap(sizes).unwrap().into_inner().unwrap();

    let mut dirs: Vec<PathBuf> = dirs_seen.into_iter().collect();
    dirs.sort_by_key(|path| path.to_string_lossy().to_lowercase());
    let mut direct_dirs: BTreeMap<PathBuf, Vec<PathBuf>> = BTreeMap::new();
    for dir_rel in &dirs {
        if dir_rel.as_os_str().is_empty() {
            continue;
        }
        let parent = dir_rel.parent().unwrap_or(Path::new("")).to_path_buf();
        direct_dirs.entry(parent).or_default().push(dir_rel.clone());
    }

    for child_dirs in direct_dirs.values_mut() {
        child_dirs.sort_by(|a, b| {
            file_name(a)
                .to_lowercase()
                .cmp(&file_name(b).to_lowercase())
        });
    }
    for child_files in direct_files.values_mut() {
        child_files.sort_by(|a, b| {
            file_name(a)
                .to_lowercase()
                .cmp(&file_name(b).to_lowercase())
        });
    }

    let mut entries = Vec::with_capacity(dirs.len().saturating_sub(1) + files.len());
    for dir in dirs.iter().filter(|dir| !dir.as_os_str().is_empty()) {
        let key = path_key(dir);
        entries.push(SnapshotEntry {
            path: dir.clone(),
            key_lower: key.to_lowercase(),
            key,
            is_dir: true,
            modified: modified.get(dir).copied(),
            size: None,
        });
    }
    for file in &files {
        let key = path_key(file);
        entries.push(SnapshotEntry {
            path: file.clone(),
            key_lower: key.to_lowercase(),
            key,
            is_dir: false,
            modified: modified.get(file).copied(),
            size: sizes.get(file).copied(),
        });
    }

    Snapshot {
        root: root.to_path_buf(),
        dirs,
        direct_dirs,
        direct_files,
        files,
        entries,
        modified,
        sizes,
    }
}

fn build_tree(snap: &Snapshot, spec: TreeBuildSpec<'_>) -> NavTree {
    let TreeBuildSpec {
        exclude_names,
        extensions,
        matcher,
        opts,
        order,
        load_lines,
    } = spec;
    let prune_empty = opts.filter_ext;
    let dirs_with_files = if prune_empty {
        compute_dirs_with_files(snap, exclude_names, extensions, matcher, opts)
    } else {
        HashSet::new()
    };

    let mut dirs = BTreeMap::new();
    for dir_rel in &snap.dirs {
        if !allow_dir(dir_rel, exclude_names, opts) {
            continue;
        }
        if prune_empty && !dirs_with_files.contains(dir_rel) {
            continue;
        }

        let mut entries = Vec::new();
        for child_dir in snap
            .direct_dirs
            .get(dir_rel)
            .into_iter()
            .flatten()
            .filter(|child| allow_path(child, exclude_names, opts))
            .filter(|child| !prune_empty || dirs_with_files.contains(*child))
        {
            entries.push(NavEntry {
                name: file_name(child_dir),
                href: dir_href(child_dir),
                is_dir: true,
                modified: snap.modified.get(child_dir).copied(),
                size: None,
                lines: None,
            });
        }

        let mut readme = None;
        for file_rel in snap.direct_files.get(dir_rel).into_iter().flatten() {
            if !allow_path(file_rel, exclude_names, opts) {
                continue;
            }
            if opts.filter_ext && !matches_filter(file_rel, extensions, matcher) {
                continue;
            }
            if is_readme(file_rel) {
                readme = Some(path_key(file_rel));
            }
            let size = snap.sizes.get(file_rel).copied();
            entries.push(NavEntry {
                name: file_name(file_rel),
                href: file_href(file_rel),
                is_dir: false,
                modified: snap.modified.get(file_rel).copied(),
                size,
                lines: if load_lines {
                    line_count(&snap.root.join(file_rel), size)
                } else {
                    None
                },
            });
        }

        sort_entries(&mut entries, order.sort, order.dir);
        dirs.insert(path_key(dir_rel), NavDir { entries, readme });
    }

    if !dirs.contains_key("") {
        dirs.insert(String::new(), NavDir::default());
    }

    NavTree { dirs }
}

fn compute_dirs_with_files(
    snap: &Snapshot,
    exclude_names: &[String],
    extensions: &[String],
    matcher: Option<&crate::filter::Matcher>,
    opts: ViewOpts,
) -> HashSet<PathBuf> {
    let mut dirs_with_files = HashSet::new();
    dirs_with_files.insert(PathBuf::new());

    for file_rel in &snap.files {
        if !allow_path(file_rel, exclude_names, opts) {
            continue;
        }
        if !matches_filter(file_rel, extensions, matcher) {
            continue;
        }
        let mut current = file_rel.parent().unwrap_or(Path::new("")).to_path_buf();
        loop {
            dirs_with_files.insert(current.clone());
            if current.as_os_str().is_empty() {
                break;
            }
            current = current.parent().unwrap_or(Path::new("")).to_path_buf();
        }
    }
    dirs_with_files
}

fn allow_dir(path: &Path, exclude_names: &[String], opts: ViewOpts) -> bool {
    path.as_os_str().is_empty() || allow_path(path, exclude_names, opts)
}

fn allow_path(path: &Path, exclude_names: &[String], opts: ViewOpts) -> bool {
    if !opts.show_hidden && paths::has_hidden_part(path) {
        return false;
    }
    if !opts.show_excludes && paths::has_excluded_part(path, exclude_names) {
        return false;
    }
    true
}

fn matches_filter(
    path: &Path,
    extensions: &[String],
    matcher: Option<&crate::filter::Matcher>,
) -> bool {
    if let Some(matcher) = matcher {
        matcher.matches(path)
    } else {
        has_extension(path, extensions)
    }
}

fn has_extension(path: &Path, extensions: &[String]) -> bool {
    let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
        return false;
    };
    extensions.iter().any(|entry| entry == &ext.to_lowercase())
}

fn is_readme(path: &Path) -> bool {
    path.file_name()
        .and_then(|s| s.to_str())
        .map(|name| name.eq_ignore_ascii_case("README.md"))
        .unwrap_or(false)
}

fn path_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn file_href(path: &Path) -> String {
    format!("/{}", path_key(path))
}

fn dir_href(path: &Path) -> String {
    let key = path_key(path);
    if key.is_empty() {
        "/".to_string()
    } else {
        format!("/{key}/")
    }
}

pub fn list_dir(root: &Path, rel: &Path, spec: ListSpec<'_>) -> Option<NavDir> {
    let abs = root.join(rel);
    if !abs.is_dir() {
        return None;
    }

    let mut entries = Vec::new();
    let mut readme = None;
    let mut walker = WalkBuilder::new(&abs);
    walker
        .max_depth(Some(1))
        .hidden(false)
        .follow_links(true)
        .same_file_system(true)
        .require_git(false)
        .git_ignore(spec.use_ignore)
        .git_exclude(spec.use_ignore)
        .git_global(spec.use_ignore);

    for entry in walker.build().filter_map(Result::ok) {
        let path = entry.path();
        if path == abs {
            continue;
        }
        let Some(file_type) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().into_owned();

        let entry_rel = rel.join(&name);
        if !allow_path(&entry_rel, spec.exclude_names, spec.opts) {
            continue;
        }
        let metadata = entry.metadata().ok();
        let mtime = metadata
            .as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs());

        if file_type.is_dir() {
            entries.push(NavEntry {
                name,
                href: dir_href(&entry_rel),
                is_dir: true,
                modified: mtime,
                size: None,
                lines: None,
            });
        } else if file_type.is_file() {
            if spec.opts.filter_ext && !matches_filter(&entry_rel, spec.extensions, spec.matcher) {
                continue;
            }
            if is_readme(&entry_rel) {
                readme = Some(path_key(&entry_rel));
            }
            let size = metadata.map(|m| m.len());
            entries.push(NavEntry {
                name,
                href: file_href(&entry_rel),
                is_dir: false,
                modified: mtime,
                size,
                lines: line_count(path, size),
            });
        }
    }

    sort_entries(&mut entries, spec.order.sort, spec.order.dir);
    Some(NavDir { entries, readme })
}

#[cfg(test)]
pub fn path_search(
    spec: PathSearchSpec<'_>,
    mut load_commits: impl FnMut(&mut [PathSearchRow]),
) -> PathSearchRows {
    let needle = spec.query.trim().to_lowercase();
    if needle.is_empty() {
        return PathSearchRows {
            rows: Vec::new(),
            truncated: false,
            max_rows: spec.max_rows,
        };
    }

    if matches!(spec.sort, Sort::CommitMessage | Sort::CommitDate) {
        let mut rows = Vec::new();
        visit_path_search_rows(&spec, &needle, |row| rows.push(row));
        load_commits(&mut rows);
        let mut rows = sort_path_search_rows(rows, &needle, spec.sort, spec.dir);
        let truncated = rows.len() > spec.max_rows;
        rows.truncate(spec.max_rows);
        return PathSearchRows {
            rows,
            truncated,
            max_rows: spec.max_rows,
        };
    }

    let mut selector = PathSearchSelector::new(spec.max_rows, &needle, spec.sort, spec.dir);
    visit_path_search_rows(&spec, &needle, |row| selector.push(row));
    let (mut rows, truncated) = selector.finish();
    if spec.load_commit_meta {
        load_commits(&mut rows);
    }
    PathSearchRows {
        rows,
        truncated,
        max_rows: spec.max_rows,
    }
}

pub fn path_search_nav(
    nav: &NavSet,
    spec: NavPathSearchSpec<'_>,
    mut load_commits: impl FnMut(&mut [PathSearchRow]),
) -> PathSearchRows {
    let needle = spec.query.trim().to_lowercase();
    if needle.is_empty() {
        return PathSearchRows {
            rows: Vec::new(),
            truncated: false,
            max_rows: spec.max_rows,
        };
    }

    if matches!(spec.sort, Sort::CommitMessage | Sort::CommitDate) {
        let mut rows = Vec::new();
        visit_nav_path_search_rows(nav, &spec, &needle, |row| rows.push(row));
        load_commits(&mut rows);
        let mut rows = sort_path_search_rows(rows, &needle, spec.sort, spec.dir);
        let truncated = rows.len() > spec.max_rows;
        rows.truncate(spec.max_rows);
        return PathSearchRows {
            rows,
            truncated,
            max_rows: spec.max_rows,
        };
    }

    let mut selector = PathSearchSelector::new(spec.max_rows, &needle, spec.sort, spec.dir);
    visit_nav_path_search_rows(nav, &spec, &needle, |row| selector.push(row));
    let (mut rows, truncated) = selector.finish();
    if spec.load_commit_meta {
        load_commits(&mut rows);
    }
    PathSearchRows {
        rows,
        truncated,
        max_rows: spec.max_rows,
    }
}

#[cfg(test)]
fn visit_path_search_rows(
    spec: &PathSearchSpec<'_>,
    needle: &str,
    mut visit: impl FnMut(PathSearchRow),
) {
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
            visit(PathSearchRow {
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

fn visit_nav_path_search_rows(
    nav: &NavSet,
    spec: &NavPathSearchSpec<'_>,
    needle: &str,
    mut visit: impl FnMut(PathSearchRow),
) {
    let load_lines = spec.load_lines || spec.sort == Sort::Lines;
    let prune_empty = spec.opts.filter_ext;
    let dirs_with_files = if prune_empty {
        compute_dirs_with_files(
            &nav.snapshot,
            &nav.exclude_names,
            &nav.extensions,
            spec.matcher,
            spec.opts,
        )
    } else {
        HashSet::new()
    };
    let prefix = (!spec.current_path.is_empty()).then(|| format!("{}/", spec.current_path));

    for entry in &nav.snapshot.entries {
        if !allow_path(&entry.path, &nav.exclude_names, spec.opts) {
            continue;
        }
        if prune_empty {
            if entry.is_dir {
                if !dirs_with_files.contains(&entry.path) {
                    continue;
                }
            } else if !matches_filter(&entry.path, &nav.extensions, spec.matcher) {
                continue;
            }
        }

        let (display, display_lower) = if let Some(prefix) = &prefix {
            let Some(display) = entry.key.strip_prefix(prefix) else {
                continue;
            };
            (display, Cow::Owned(display.to_lowercase()))
        } else {
            (entry.key.as_str(), Cow::Borrowed(entry.key_lower.as_str()))
        };
        if !display_lower.contains(needle) {
            continue;
        }

        visit(PathSearchRow {
            href: if entry.is_dir {
                dir_href(&entry.path)
            } else {
                file_href(&entry.path)
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
                line_count(&nav.snapshot.root.join(&entry.path), entry.size)
            } else {
                None
            },
            commit_subject: None,
            commit_timestamp: None,
        });
    }
}

fn sort_path_search_rows(
    rows: Vec<PathSearchRow>,
    needle: &str,
    sort: Sort,
    dir: SortDir,
) -> Vec<PathSearchRow> {
    let mut ranked = rows
        .into_iter()
        .map(|row| RankedPathSearchRow::new(row, needle, sort, dir))
        .collect::<Vec<_>>();
    ranked.sort();
    ranked.into_iter().map(|ranked| ranked.row).collect()
}

struct PathSearchSelector<'a> {
    heap: BinaryHeap<RankedPathSearchRow>,
    count: usize,
    max_rows: usize,
    needle: &'a str,
    sort: Sort,
    dir: SortDir,
}

impl<'a> PathSearchSelector<'a> {
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

    fn push(&mut self, row: PathSearchRow) {
        self.count += 1;
        if self.max_rows == 0 {
            return;
        }

        let ranked = RankedPathSearchRow::new(row, self.needle, self.sort, self.dir);
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

    fn finish(self) -> (Vec<PathSearchRow>, bool) {
        let truncated = self.count > self.max_rows;
        let mut ranked = self.heap.into_vec();
        ranked.sort();
        (
            ranked.into_iter().map(|ranked| ranked.row).collect(),
            truncated,
        )
    }
}

struct RankedPathSearchRow {
    row: PathSearchRow,
    base_match: bool,
    display_lower: String,
    ext: String,
    commit_subject_lower: Option<String>,
    sort: Sort,
    dir: SortDir,
}

impl RankedPathSearchRow {
    fn new(row: PathSearchRow, needle: &str, sort: Sort, dir: SortDir) -> Self {
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

impl PartialEq for RankedPathSearchRow {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for RankedPathSearchRow {}

impl PartialOrd for RankedPathSearchRow {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RankedPathSearchRow {
    fn cmp(&self, other: &Self) -> Ordering {
        cmp_path_search_rows(self, other, self.sort, self.dir)
    }
}

fn cmp_path_search_rows(
    a: &RankedPathSearchRow,
    b: &RankedPathSearchRow,
    sort: Sort,
    dir: SortDir,
) -> Ordering {
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

fn sort_entries(entries: &mut [NavEntry], sort: Sort, dir: SortDir) {
    entries.sort_by(|a, b| cmp_entries(a, b, sort, dir));
}

fn view_key(opts: ViewOpts) -> u8 {
    (opts.show_hidden as u8) | ((opts.show_excludes as u8) << 1) | ((opts.filter_ext as u8) << 2)
}

fn tree_cache_key(opts: ViewOpts, sort: Sort, dir: SortDir, load_lines: bool) -> usize {
    tree_key(opts, sort, dir) * 2 + usize::from(load_lines)
}

fn tree_key(opts: ViewOpts, sort: Sort, dir: SortDir) -> usize {
    (sort_index(sort) * 2 + dir_index(dir)) * VIEW_COMBINATIONS + view_key(opts) as usize
}

fn sort_index(sort: Sort) -> usize {
    SORT_DEFS
        .iter()
        .position(|def| def.sort == sort)
        .expect("sort definition exists")
}

fn dir_index(dir: SortDir) -> usize {
    match dir {
        SortDir::Asc => 0,
        SortDir::Desc => 1,
    }
}

fn cmp_entries(a: &NavEntry, b: &NavEntry, sort: Sort, dir: SortDir) -> Ordering {
    dir_first(a.is_dir, b.is_dir).then_with(|| match sort {
        Sort::Name => apply_dir(cmp_names(&a.name, &b.name), dir),
        Sort::Type => apply_dir(cmp_types(&a.name, &b.name, a.is_dir, b.is_dir), dir),
        Sort::Timestamp => apply_dir(
            cmp_timestamps(a.modified, b.modified, &a.name, &b.name),
            dir,
        ),
        Sort::Size => apply_dir(cmp_opt(a.size, b.size, &a.name, &b.name), dir),
        Sort::Lines => apply_dir(cmp_opt(a.lines, b.lines, &a.name, &b.name), dir),
        Sort::CommitMessage | Sort::CommitDate => apply_dir(cmp_names(&a.name, &b.name), dir),
    })
}

fn dir_first(a_is_dir: bool, b_is_dir: bool) -> Ordering {
    match (a_is_dir, b_is_dir) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => Ordering::Equal,
    }
}

fn cmp_names(a: &str, b: &str) -> Ordering {
    a.to_lowercase().cmp(&b.to_lowercase())
}

fn cmp_types(a: &str, b: &str, a_is_dir: bool, b_is_dir: bool) -> Ordering {
    if a_is_dir || b_is_dir {
        return cmp_names(a, b);
    }
    ext_key(a).cmp(&ext_key(b)).then_with(|| cmp_names(a, b))
}

fn cmp_timestamps(a: Option<u64>, b: Option<u64>, a_name: &str, b_name: &str) -> Ordering {
    a.cmp(&b).then_with(|| cmp_names(a_name, b_name))
}

fn cmp_opt(a: Option<u64>, b: Option<u64>, a_name: &str, b_name: &str) -> Ordering {
    a.cmp(&b).then_with(|| cmp_names(a_name, b_name))
}

fn ext_key(name: &str) -> (&str, &str) {
    let path = Path::new(name);
    (
        path.extension().and_then(|ext| ext.to_str()).unwrap_or(""),
        name,
    )
}

fn apply_dir(order: Ordering, dir: SortDir) -> Ordering {
    match dir {
        SortDir::Asc => order,
        SortDir::Desc => order.reverse(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{TempDir, nav_entry};
    use std::fs;

    #[test]
    fn sort_name_keeps_dirs_first() {
        let mut entries = vec![
            nav_entry("zeta.rs", false, Some(1)),
            nav_entry("alpha", true, Some(9)),
            nav_entry("beta.md", false, Some(3)),
        ];
        sort_entries(&mut entries, Sort::Name, SortDir::Asc);
        let names: Vec<_> = entries.into_iter().map(|entry| entry.name).collect();
        assert_eq!(names, vec!["alpha", "beta.md", "zeta.rs"]);
    }

    #[test]
    fn sort_type_groups_extensions() {
        let mut entries = vec![
            nav_entry("b.rs", false, Some(1)),
            nav_entry("docs", true, Some(9)),
            nav_entry("a.md", false, Some(3)),
            nav_entry("a.rs", false, Some(2)),
        ];
        sort_entries(&mut entries, Sort::Type, SortDir::Asc);
        let names: Vec<_> = entries.into_iter().map(|entry| entry.name).collect();
        assert_eq!(names, vec!["docs", "a.md", "a.rs", "b.rs"]);
    }

    #[test]
    fn sort_timestamp_prefers_newest() {
        let mut entries = vec![
            nav_entry("old.md", false, Some(1)),
            nav_entry("missing.md", false, None),
            nav_entry("new.md", false, Some(9)),
        ];
        sort_entries(&mut entries, Sort::Timestamp, SortDir::Desc);
        let names: Vec<_> = entries.into_iter().map(|entry| entry.name).collect();
        assert_eq!(names, vec!["new.md", "old.md", "missing.md"]);
    }

    #[test]
    fn sort_name_desc_reverses_within_kind() {
        let mut entries = vec![
            nav_entry("alpha", true, Some(9)),
            nav_entry("beta", true, Some(8)),
            nav_entry("a.md", false, Some(3)),
            nav_entry("b.md", false, Some(1)),
        ];
        sort_entries(&mut entries, Sort::Name, SortDir::Desc);
        let names: Vec<_> = entries.into_iter().map(|entry| entry.name).collect();
        assert_eq!(names, vec!["beta", "alpha", "b.md", "a.md"]);
    }

    #[test]
    fn list_dir_toggles_gitignore() {
        let td = TempDir::new("ghrm-walk-test");
        fs::write(td.path().join(".gitignore"), "ignored.txt\n").unwrap();
        fs::write(td.path().join("ignored.txt"), "ignored\n").unwrap();
        fs::write(td.path().join("visible.txt"), "visible\n").unwrap();

        let opts = ViewOpts {
            show_hidden: true,
            show_excludes: true,
            filter_ext: false,
        };
        let order = SortSpec {
            sort: Sort::Name,
            dir: SortDir::Asc,
        };
        let ignored = list_dir(
            td.path(),
            Path::new(""),
            ListSpec {
                use_ignore: true,
                exclude_names: &[],
                extensions: &[],
                matcher: None,
                opts,
                order,
            },
        )
        .unwrap()
        .entries;
        let ignored_names: Vec<_> = ignored.into_iter().map(|entry| entry.name).collect();
        assert!(!ignored_names.contains(&"ignored.txt".to_string()));

        let shown = list_dir(
            td.path(),
            Path::new(""),
            ListSpec {
                use_ignore: false,
                exclude_names: &[],
                extensions: &[],
                matcher: None,
                opts,
                order,
            },
        )
        .unwrap()
        .entries;
        let shown_names: Vec<_> = shown.into_iter().map(|entry| entry.name).collect();
        assert!(shown_names.contains(&"ignored.txt".to_string()));
    }
}
