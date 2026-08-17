pub(crate) mod diff;
mod history;
pub(crate) mod refs;
mod remote;
mod root;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default)]
pub struct RepoSet {
    entries: Vec<RepoEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitInfo {
    pub subject: String,
    pub author: String,
    pub email: String,
    pub timestamp: u64,
}

#[derive(Clone, Debug)]
struct RepoEntry {
    root: PathBuf,
    source: SourceState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceState {
    Web {
        url: String,
        raw: String,
        forge: Forge,
    },
    Transport {
        raw: String,
    },
    NoRemote,
    NoRepo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Forge {
    GitHub,
    Bitbucket,
    GitLab,
    Codeberg,
    SourceHut,
    Generic,
}

impl RepoSet {
    pub fn discover(root: &Path, exclude_names: &[String]) -> Self {
        Self {
            entries: root::discover(root, exclude_names),
        }
    }

    pub fn source_for(&self, path: &Path) -> SourceState {
        self.entries
            .iter()
            .find(|entry| path.starts_with(&entry.root))
            .map(|entry| entry.source.clone())
            .unwrap_or(SourceState::NoRepo)
    }

    pub fn repo_for(&self, path: &Path) -> Option<(&Path, String)> {
        let entry = self
            .entries
            .iter()
            .find(|entry| path.starts_with(&entry.root))?;
        let rel = path.strip_prefix(&entry.root).ok()?;
        if rel.as_os_str().is_empty() {
            return None;
        }
        Some((entry.root.as_path(), path_key(rel)))
    }

    pub fn commit_info(&self, paths: &[PathBuf]) -> BTreeMap<PathBuf, CommitInfo> {
        let mut out = BTreeMap::new();
        for entry in &self.entries {
            let pending = paths
                .iter()
                .filter(|path| !out.contains_key(*path) && path.starts_with(&entry.root))
                .cloned()
                .collect::<Vec<_>>();
            out.extend(history::commit_info(&entry.root, &pending));
        }
        out
    }
}

fn path_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn commit_time(commit: &gix::Commit<'_>) -> Option<u64> {
    u64::try_from(commit.time().ok()?.seconds).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;
    use std::fs;

    fn write_git_config(root: &Path) {
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(
            root.join(".git/config"),
            "[core]\nrepositoryformatversion = 0\n",
        )
        .unwrap();
    }

    #[test]
    fn repo_for_resolves_deepest_repo_and_relative_path() {
        let td = TempDir::new("ghrm-repo-for");
        let outer = td.path().join("outer");
        let inner = outer.join("vendor/inner");
        write_git_config(&outer);
        write_git_config(&inner);
        fs::create_dir_all(inner.join("src")).unwrap();
        fs::write(inner.join("src/lib.rs"), "").unwrap();
        fs::write(outer.join("README.md"), "").unwrap();

        let repos = RepoSet::discover(&outer, &[]);

        let (root, rel) = repos.repo_for(&inner.join("src/lib.rs")).unwrap();
        assert_eq!(root, inner.as_path());
        assert_eq!(rel, "src/lib.rs");

        let (root, rel) = repos.repo_for(&outer.join("README.md")).unwrap();
        assert_eq!(root, outer.as_path());
        assert_eq!(rel, "README.md");
    }

    #[test]
    fn repo_for_rejects_repo_roots_and_foreign_paths() {
        let td = TempDir::new("ghrm-repo-for-none");
        let repo = td.path().join("repo");
        write_git_config(&repo);

        let repos = RepoSet::discover(&repo, &[]);

        assert!(repos.repo_for(&repo).is_none());
        assert!(
            repos
                .repo_for(Path::new("/nonexistent/elsewhere.txt"))
                .is_none()
        );
    }
}
