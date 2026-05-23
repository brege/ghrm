use super::{RepoEntry, remote};
use crate::paths;

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn discover(root: &Path, exclude_names: &[String]) -> Vec<RepoEntry> {
    let scan_root = if root.is_dir() {
        root.to_path_buf()
    } else {
        root.parent().unwrap_or(root).to_path_buf()
    };

    let mut roots = Vec::new();
    let mut seen = HashSet::new();
    if let Some(ancestor) = nearest_repo_root(root) {
        push_root(&mut roots, &mut seen, ancestor);
    }
    collect_repo_roots(&scan_root, exclude_names, &mut roots, &mut seen);

    let mut entries = roots
        .into_iter()
        .map(|root| RepoEntry {
            source: remote::source_for_repo(&root),
            root,
        })
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| {
        b.root
            .components()
            .count()
            .cmp(&a.root.components().count())
    });
    entries
}

pub(super) fn git_config_path(dot_git: &Path) -> Option<PathBuf> {
    if dot_git.is_dir() {
        let config = dot_git.join("config");
        return config.is_file().then_some(config);
    }
    if !dot_git.is_file() {
        return None;
    }

    let text = fs::read_to_string(dot_git).ok()?;
    for line in text.lines() {
        let Some(path) = line.trim().strip_prefix("gitdir:") else {
            continue;
        };
        let path = path.trim();
        let gitdir = if Path::new(path).is_absolute() {
            PathBuf::from(path)
        } else {
            dot_git.parent()?.join(path)
        };
        let config = gitdir.join("config");
        return config.is_file().then_some(config);
    }
    None
}

fn push_root(roots: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, root: PathBuf) {
    if seen.insert(root.clone()) {
        roots.push(root);
    }
}

fn nearest_repo_root(path: &Path) -> Option<PathBuf> {
    let mut dir = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()?.to_path_buf()
    };
    loop {
        if git_config_path(&dir.join(".git")).is_some() {
            return Some(dir);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

fn collect_repo_roots(
    dir: &Path,
    exclude_names: &[String],
    roots: &mut Vec<PathBuf>,
    seen: &mut HashSet<PathBuf>,
) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };
        let name = entry.file_name();
        let name = name.to_string_lossy();

        if name == ".git" {
            if (file_type.is_dir() || file_type.is_file())
                && git_config_path(&entry.path()).is_some()
            {
                push_root(roots, seen, dir.to_path_buf());
            }
            continue;
        }

        if !file_type.is_dir()
            || file_type.is_symlink()
            || !paths::allowed_name(&name, exclude_names)
        {
            continue;
        }

        collect_repo_roots(&entry.path(), exclude_names, roots, seen);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("ghrm-repo-root-{name}-{nanos}"))
    }

    fn write_git_config(root: &Path) {
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(
            root.join(".git/config"),
            "[core]\nrepositoryformatversion = 0\n",
        )
        .unwrap();
    }

    #[test]
    fn git_config_path_ignores_empty_git_dir() {
        let root = temp_root("empty");
        fs::create_dir_all(root.join(".git")).unwrap();

        assert_eq!(git_config_path(&root.join(".git")), None);

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn discover_ignores_empty_git_dirs() {
        let root = temp_root("discover");
        let child = root.join("child");
        fs::create_dir_all(root.join(".git")).unwrap();
        write_git_config(&child);

        let entries = discover(&root, &[]);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].root, child);

        fs::remove_dir_all(root).ok();
    }
}
