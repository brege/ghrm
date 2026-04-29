use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{config, filter, walk};

pub(crate) struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub(crate) fn new(prefix: &str) -> Self {
        let unique = format!(
            "{prefix}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let path = std::env::temp_dir().join(unique);
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub(crate) fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub(crate) fn group_filters() -> filter::Set {
    let groups = [
        (
            "docs".to_string(),
            config::FilterGroupConfig {
                label: Some("Docs".to_string()),
                globs: vec!["*.md".to_string()],
            },
        ),
        (
            "web".to_string(),
            config::FilterGroupConfig {
                label: Some("Web".to_string()),
                globs: vec!["*.html".to_string()],
            },
        ),
    ]
    .into_iter()
    .collect();
    filter::Set::resolve(&config::FilterConfig {
        enabled: Some(false),
        default_group: Some("docs".to_string()),
        groups,
    })
    .unwrap()
}

pub(crate) fn nav_entry(name: &str, is_dir: bool, modified: Option<u64>) -> walk::NavEntry {
    walk::NavEntry {
        name: name.to_string(),
        href: String::new(),
        is_dir,
        modified,
        size: None,
    }
}
