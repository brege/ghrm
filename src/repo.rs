mod log;
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

    pub fn commit_info(&self, paths: &[PathBuf]) -> BTreeMap<PathBuf, CommitInfo> {
        let mut out = BTreeMap::new();
        for entry in &self.entries {
            let pending = paths
                .iter()
                .filter(|path| !out.contains_key(*path) && path.starts_with(&entry.root))
                .cloned()
                .collect::<Vec<_>>();
            out.extend(log::commit_info(&entry.root, &pending));
        }
        out
    }
}
