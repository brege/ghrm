use ignore::WalkBuilder;
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

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

pub fn build(root: &Path, use_ignore: bool) -> NavTree {
    let mut dirs_seen: HashSet<PathBuf> = HashSet::new();
    dirs_seen.insert(root.to_path_buf());

    let walker = WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(use_ignore)
        .git_exclude(use_ignore)
        .git_global(use_ignore)
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !matches!(
                name.as_ref(),
                ".git" | "node_modules" | ".venv" | "__pycache__"
            )
        })
        .build();

    for entry in walker.flatten() {
        let path = entry.path();
        if path == root {
            continue;
        }
        let file_type = match entry.file_type() {
            Some(ft) => ft,
            None => continue,
        };
        if file_type.is_dir() {
            dirs_seen.insert(path.to_path_buf());
        }
    }

    let mut dirs: BTreeMap<String, NavDir> = BTreeMap::new();

    for dir in &dirs_seen {
        let rel = dir.strip_prefix(root).unwrap().to_path_buf();
        let key = rel.to_string_lossy().replace('\\', "/");

        let mut entries: Vec<NavEntry> = Vec::new();
        let mut readme: Option<String> = None;

        let read = match std::fs::read_dir(dir) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for child in read.flatten() {
            let cp = child.path();
            let name = cp.file_name().unwrap().to_string_lossy().to_string();
            if matches!(
                name.as_str(),
                ".git" | "node_modules" | ".venv" | "__pycache__"
            ) {
                continue;
            }
            if name.starts_with('.') {
                continue;
            }
            let ft = match child.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            let child_rel = cp.strip_prefix(root).unwrap();
            let href = format!("/{}", child_rel.to_string_lossy().replace('\\', "/"));
            if ft.is_dir() {
                if dirs_seen.contains(&cp) {
                    entries.push(NavEntry {
                        name,
                        href: format!("{}/", href.trim_end_matches('/')),
                        is_dir: true,
                    });
                }
            } else if ft.is_file() && cp.extension().and_then(|s| s.to_str()) == Some("md") {
                if name.eq_ignore_ascii_case("README.md") {
                    readme = Some(child_rel.to_string_lossy().to_string());
                }
                entries.push(NavEntry {
                    name,
                    href,
                    is_dir: false,
                });
            }
        }

        entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        });

        dirs.insert(key, NavDir { entries, readme });
    }

    NavTree { dirs }
}
