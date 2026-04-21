use ignore::{WalkBuilder, WalkState};
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Filtered,
    Files,
    All,
}

impl Scope {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "filter" => Some(Self::Filtered),
            "files" | "*" => Some(Self::Files),
            "all" | "**" => Some(Self::All),
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

#[derive(Clone, Debug, Default)]
pub struct NavSet {
    pub filtered: NavTree,
    pub files: NavTree,
    pub all: NavTree,
}

impl NavSet {
    pub fn get(&self, scope: Scope) -> &NavTree {
        match scope {
            Scope::Filtered => &self.filtered,
            Scope::Files => &self.files,
            Scope::All => &self.all,
        }
    }
}

struct Snapshot {
    dirs: Vec<PathBuf>,
    direct_dirs: BTreeMap<PathBuf, Vec<PathBuf>>,
    direct_files: BTreeMap<PathBuf, Vec<PathBuf>>,
    files: Vec<PathBuf>,
    modified: BTreeMap<PathBuf, u64>,
}

pub fn build_all(
    root: &Path,
    use_ignore: bool,
    exclude_names: &[String],
    extensions: &[String],
) -> NavSet {
    let snap = scan(root, use_ignore, exclude_names);
    NavSet {
        filtered: build_tree(
            &snap,
            TreeOpts {
                show_hidden: false,
                extensions,
            },
        ),
        files: build_tree(
            &snap,
            TreeOpts {
                show_hidden: false,
                extensions: &[],
            },
        ),
        all: build_tree(
            &snap,
            TreeOpts {
                show_hidden: true,
                extensions: &[],
            },
        ),
    }
}

fn scan(root: &Path, use_ignore: bool, exclude_names: &[String]) -> Snapshot {
    let root_buf = root.to_path_buf();
    let exclude_names = exclude_names.to_vec();
    let dirs_seen: Arc<Mutex<HashSet<PathBuf>>> = Arc::new(Mutex::new({
        let mut s = HashSet::new();
        s.insert(PathBuf::new());
        s
    }));
    let direct_files: Arc<Mutex<BTreeMap<PathBuf, Vec<PathBuf>>>> =
        Arc::new(Mutex::new(BTreeMap::new()));
    let files: Arc<Mutex<Vec<PathBuf>>> = Arc::new(Mutex::new(Vec::new()));
    let modified: Arc<Mutex<BTreeMap<PathBuf, u64>>> = Arc::new(Mutex::new(BTreeMap::new()));

    WalkBuilder::new(&root_buf)
        .hidden(false)
        .follow_links(true)
        .same_file_system(true)
        .require_git(false)
        .git_ignore(use_ignore)
        .git_exclude(use_ignore)
        .git_global(use_ignore)
        .filter_entry(move |e| allow_walk_name(&e.file_name().to_string_lossy(), &exclude_names))
        .build_parallel()
        .run(|| {
            let root = root_buf.clone();
            let dirs_seen = dirs_seen.clone();
            let direct_files = direct_files.clone();
            let files = files.clone();
            let modified = modified.clone();
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
                let mtime = entry
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs());
                if let Some(ts) = mtime {
                    modified.lock().unwrap().insert(rel.clone(), ts);
                }
                {
                    let mut guard = dirs_seen.lock().unwrap();
                    guard.insert(parent.clone());
                    if file_type.is_dir() {
                        guard.insert(rel.clone());
                        return WalkState::Continue;
                    }
                }
                if file_type.is_file() {
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
    }
}

struct TreeOpts<'a> {
    show_hidden: bool,
    extensions: &'a [String],
}

fn build_tree(snap: &Snapshot, opts: TreeOpts<'_>) -> NavTree {
    let prune_empty = !opts.extensions.is_empty();
    let dirs_with_files = if prune_empty {
        compute_dirs_with_files(snap, opts.show_hidden, opts.extensions)
    } else {
        HashSet::new()
    };

    let mut dirs = BTreeMap::new();
    for dir_rel in &snap.dirs {
        if !allow_scope_dir(dir_rel, opts.show_hidden) {
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
            .filter(|child| allow_scope_path(child, opts.show_hidden))
            .filter(|child| !prune_empty || dirs_with_files.contains(*child))
        {
            entries.push(NavEntry {
                name: file_name(child_dir),
                href: dir_href(child_dir),
                is_dir: true,
                modified: snap.modified.get(child_dir).copied(),
            });
        }

        let mut readme = None;
        for file_rel in snap.direct_files.get(dir_rel).into_iter().flatten() {
            if !allow_scope_path(file_rel, opts.show_hidden) {
                continue;
            }
            if !opts.extensions.is_empty() && !has_extension(file_rel, opts.extensions) {
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
            });
        }

        sort_entries(&mut entries);
        dirs.insert(path_key(dir_rel), NavDir { entries, readme });
    }

    if !dirs.contains_key("") {
        dirs.insert(String::new(), NavDir::default());
    }

    NavTree { dirs }
}

fn compute_dirs_with_files(
    snap: &Snapshot,
    show_hidden: bool,
    extensions: &[String],
) -> HashSet<PathBuf> {
    let mut dirs_with_files = HashSet::new();
    dirs_with_files.insert(PathBuf::new());

    for file_rel in &snap.files {
        if !allow_scope_path(file_rel, show_hidden) {
            continue;
        }
        if !has_extension(file_rel, extensions) {
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

fn allow_walk_name(name: &str, exclude_names: &[String]) -> bool {
    name != ".git" && !exclude_names.iter().any(|entry| entry == name)
}

fn allow_scope_dir(path: &Path, show_hidden: bool) -> bool {
    path.as_os_str().is_empty() || allow_scope_path(path, show_hidden)
}

fn allow_scope_path(path: &Path, show_hidden: bool) -> bool {
    if show_hidden {
        return true;
    }
    for part in path.iter() {
        if part.to_string_lossy().starts_with('.') {
            return false;
        }
    }
    true
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

fn sort_entries(entries: &mut [NavEntry]) {
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
}
