use crate::paths;

use ignore::{WalkBuilder, WalkState};
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;

const VIEW_COMBINATIONS: usize = 8;
const SORT_VARIANTS: usize = 6;

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
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortDir {
    #[default]
    Asc,
    Desc,
}

impl Sort {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Type => "type",
            Self::Timestamp => "timestamp",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "name" => Some(Self::Name),
            "type" => Some(Self::Type),
            "timestamp" => Some(Self::Timestamp),
            _ => None,
        }
    }

    pub fn default_dir(self) -> SortDir {
        match self {
            Self::Timestamp => SortDir::Desc,
            Self::Name | Self::Type => SortDir::Asc,
        }
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

type TreeSet = [Arc<NavTree>; VIEW_COMBINATIONS * SORT_VARIANTS];

#[derive(Clone, Debug)]
pub struct NavSet {
    trees: TreeSet,
    snapshot: Arc<Snapshot>,
    exclude_names: Arc<Vec<String>>,
    extensions: Arc<Vec<String>>,
}

impl NavSet {
    pub fn get(
        &self,
        opts: ViewOpts,
        sort: Sort,
        dir: SortDir,
        matcher: Option<&crate::filter::Matcher>,
    ) -> Arc<NavTree> {
        let idx = tree_key(opts, sort, dir);
        if opts.filter_ext && matcher.is_some() {
            return Arc::new(build_tree(
                &self.snapshot,
                &self.exclude_names,
                &self.extensions,
                matcher,
                opts,
                sort,
                dir,
            ));
        }
        self.trees[idx].clone()
    }
}

impl Default for NavSet {
    fn default() -> Self {
        Self {
            trees: build_trees(&Snapshot::default(), &[], &[], None),
            snapshot: Arc::new(Snapshot::default()),
            exclude_names: Arc::new(Vec::new()),
            extensions: Arc::new(Vec::new()),
        }
    }
}

#[derive(Debug, Default)]
struct Snapshot {
    dirs: Vec<PathBuf>,
    direct_dirs: BTreeMap<PathBuf, Vec<PathBuf>>,
    direct_files: BTreeMap<PathBuf, Vec<PathBuf>>,
    files: Vec<PathBuf>,
    modified: BTreeMap<PathBuf, u64>,
    sizes: BTreeMap<PathBuf, u64>,
}

pub fn build_all(
    root: &Path,
    use_ignore: bool,
    exclude_names: &[String],
    extensions: &[String],
    no_excludes: bool,
) -> NavSet {
    let snapshot = Arc::new(scan(root, use_ignore, exclude_names, no_excludes));
    let trees = build_trees(&snapshot, exclude_names, extensions, None);
    NavSet {
        trees,
        snapshot,
        exclude_names: Arc::new(exclude_names.to_vec()),
        extensions: Arc::new(extensions.to_vec()),
    }
}

fn build_trees(
    snap: &Snapshot,
    exclude_names: &[String],
    extensions: &[String],
    matcher: Option<&crate::filter::Matcher>,
) -> TreeSet {
    std::array::from_fn(|idx| {
        let sort = sort_from_index(idx);
        let dir = dir_from_index(idx);
        let view = idx % VIEW_COMBINATIONS;
        let opts = ViewOpts {
            show_hidden: (view & 1) != 0,
            show_excludes: (view & 2) != 0,
            filter_ext: (view & 4) != 0,
        };
        Arc::new(build_tree(
            snap,
            exclude_names,
            extensions,
            matcher,
            opts,
            sort,
            dir,
        ))
    })
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
                if let Some(metadata) = metadata {
                    sizes.lock().unwrap().insert(rel.clone(), metadata.len());
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

    Snapshot {
        dirs,
        direct_dirs,
        direct_files,
        files,
        modified,
        sizes,
    }
}

fn build_tree(
    snap: &Snapshot,
    exclude_names: &[String],
    extensions: &[String],
    matcher: Option<&crate::filter::Matcher>,
    opts: ViewOpts,
    sort: Sort,
    dir: SortDir,
) -> NavTree {
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
            entries.push(NavEntry {
                name: file_name(file_rel),
                href: file_href(file_rel),
                is_dir: false,
                modified: snap.modified.get(file_rel).copied(),
                size: snap.sizes.get(file_rel).copied(),
            });
        }

        sort_entries(&mut entries, sort, dir);
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
            });
        } else if file_type.is_file() {
            if spec.opts.filter_ext && !matches_filter(&entry_rel, spec.extensions, spec.matcher) {
                continue;
            }
            if is_readme(&entry_rel) {
                readme = Some(path_key(&entry_rel));
            }
            entries.push(NavEntry {
                name,
                href: file_href(&entry_rel),
                is_dir: false,
                modified: mtime,
                size: metadata.map(|m| m.len()),
            });
        }
    }

    sort_entries(&mut entries, spec.order.sort, spec.order.dir);
    Some(NavDir { entries, readme })
}

fn sort_entries(entries: &mut [NavEntry], sort: Sort, dir: SortDir) {
    entries.sort_by(|a, b| cmp_entries(a, b, sort, dir));
}

fn view_key(opts: ViewOpts) -> u8 {
    (opts.show_hidden as u8) | ((opts.show_excludes as u8) << 1) | ((opts.filter_ext as u8) << 2)
}

fn tree_key(opts: ViewOpts, sort: Sort, dir: SortDir) -> usize {
    (sort_index(sort) * 2 + dir_index(dir)) * VIEW_COMBINATIONS + view_key(opts) as usize
}

fn sort_index(sort: Sort) -> usize {
    match sort {
        Sort::Name => 0,
        Sort::Type => 1,
        Sort::Timestamp => 2,
    }
}

fn sort_from_index(idx: usize) -> Sort {
    match (idx / VIEW_COMBINATIONS) / 2 {
        0 => Sort::Name,
        1 => Sort::Type,
        _ => Sort::Timestamp,
    }
}

fn dir_from_index(idx: usize) -> SortDir {
    match (idx / VIEW_COMBINATIONS) % 2 {
        0 => SortDir::Asc,
        _ => SortDir::Desc,
    }
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
