use ignore::WalkBuilder;
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Md,
    Files,
    All,
}

impl Scope {
    pub fn parse(raw: Option<&str>) -> Self {
        match raw {
            Some("files") | Some("*") => Self::Files,
            Some("all") | Some("**") => Self::All,
            _ => Self::Md,
        }
    }

    pub fn query(self) -> Option<&'static str> {
        match self {
            Self::Md => None,
            Self::Files => Some("files"),
            Self::All => Some("all"),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct NavEntry {
    pub name: String,
    pub href: String,
    pub is_dir: bool,
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
    pub md: NavTree,
    pub files: NavTree,
    pub all: NavTree,
}

impl NavSet {
    pub fn get(&self, scope: Scope) -> &NavTree {
        match scope {
            Scope::Md => &self.md,
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
}

pub fn build_all(root: &Path, use_ignore: bool) -> NavSet {
    let snap = scan(root, use_ignore);
    NavSet {
        md: build_md(&snap, false),
        files: build_files(&snap, false),
        all: build_files(&snap, true),
    }
}

fn scan(root: &Path, use_ignore: bool) -> Snapshot {
    let mut dirs_seen: HashSet<PathBuf> = HashSet::new();
    dirs_seen.insert(PathBuf::new());
    let mut direct_files: BTreeMap<PathBuf, Vec<PathBuf>> = BTreeMap::new();
    let mut files: Vec<PathBuf> = Vec::new();

    let walker = WalkBuilder::new(root)
        .hidden(false)
        .require_git(false)
        .git_ignore(use_ignore)
        .git_exclude(use_ignore)
        .git_global(use_ignore)
        .filter_entry(|e| allow_walk_name(&e.file_name().to_string_lossy()))
        .build();

    for entry in walker.flatten() {
        let path = entry.path();
        if path == root {
            continue;
        }
        let Some(file_type) = entry.file_type() else {
            continue;
        };
        let rel = path.strip_prefix(root).unwrap().to_path_buf();
        let parent = rel.parent().unwrap_or(Path::new("")).to_path_buf();
        dirs_seen.insert(parent.clone());
        if file_type.is_dir() {
            dirs_seen.insert(rel);
        } else if file_type.is_file() {
            files.push(rel.clone());
            direct_files.entry(parent).or_default().push(rel);
        }
    }

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

    files.sort_by_key(|path| path.to_string_lossy().to_lowercase());
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
    }
}

fn build_md(snap: &Snapshot, show_hidden: bool) -> NavTree {
    let mut dirs_with_md: HashSet<PathBuf> = HashSet::new();
    dirs_with_md.insert(PathBuf::new());

    for file_rel in &snap.files {
        if !allow_scope_path(file_rel, show_hidden) {
            continue;
        }
        if !is_markdown(file_rel) {
            continue;
        }
        let mut current = file_rel.parent().unwrap_or(Path::new("")).to_path_buf();
        loop {
            dirs_with_md.insert(current.clone());
            if current.as_os_str().is_empty() {
                break;
            }
            current = current.parent().unwrap_or(Path::new("")).to_path_buf();
        }
    }

    let mut dirs = BTreeMap::new();
    for dir_rel in &snap.dirs {
        if !allow_scope_dir(dir_rel, show_hidden) {
            continue;
        }
        if !dirs_with_md.contains(dir_rel) {
            continue;
        }

        let mut entries = Vec::new();
        for child_dir in snap
            .direct_dirs
            .get(dir_rel)
            .into_iter()
            .flatten()
            .filter(|child| allow_scope_path(child, show_hidden))
            .filter(|child| dirs_with_md.contains(*child))
        {
            entries.push(NavEntry {
                name: file_name(child_dir),
                href: dir_href(child_dir),
                is_dir: true,
            });
        }

        let mut readme = None;
        for file_rel in snap.direct_files.get(dir_rel).into_iter().flatten() {
            if !allow_scope_path(file_rel, show_hidden) {
                continue;
            }
            if !is_markdown(file_rel) {
                continue;
            }
            if is_readme(file_rel) {
                readme = Some(path_key(file_rel));
            }
            entries.push(NavEntry {
                name: file_name(file_rel),
                href: file_href(file_rel),
                is_dir: false,
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

fn build_files(snap: &Snapshot, show_hidden: bool) -> NavTree {
    let mut dirs = BTreeMap::new();
    for dir_rel in &snap.dirs {
        if !allow_scope_dir(dir_rel, show_hidden) {
            continue;
        }
        let mut entries = Vec::new();

        for child_dir in snap
            .direct_dirs
            .get(dir_rel)
            .into_iter()
            .flatten()
            .filter(|child| allow_scope_path(child, show_hidden))
        {
            entries.push(NavEntry {
                name: file_name(child_dir),
                href: dir_href(child_dir),
                is_dir: true,
            });
        }

        let mut readme = None;
        for file_rel in snap.direct_files.get(dir_rel).into_iter().flatten() {
            if !allow_scope_path(file_rel, show_hidden) {
                continue;
            }
            if is_readme(file_rel) {
                readme = Some(path_key(file_rel));
            }
            entries.push(NavEntry {
                name: file_name(file_rel),
                href: file_href(file_rel),
                is_dir: false,
            });
        }

        sort_entries(&mut entries);
        dirs.insert(path_key(dir_rel), NavDir { entries, readme });
    }

    NavTree { dirs }
}

fn allow_walk_name(name: &str) -> bool {
    if matches!(name, ".git" | "node_modules" | ".venv" | "__pycache__") {
        return false;
    }
    true
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

fn is_markdown(path: &Path) -> bool {
    matches!(path.extension().and_then(|s| s.to_str()), Some("md"))
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
