use anyhow::Result;
use include_dir::{Dir, DirEntry, include_dir};
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    thread,
    time::{Duration, SystemTime},
};
use tokio::sync::broadcast;
use tracing::info;

const THEME_VERSION: &str = include_str!(concat!(env!("OUT_DIR"), "/theme_version.txt"));
const THEME_DIRS: &[&str] = &["css", "img", "js"];
const DEV_WATCH_INTERVAL: Duration = Duration::from_millis(300);

static CSS: Dir = include_dir!("$CARGO_MANIFEST_DIR/assets/css");
static IMG: Dir = include_dir!("$CARGO_MANIFEST_DIR/assets/img");
static JS: Dir = include_dir!("$CARGO_MANIFEST_DIR/assets/js");

pub fn dir() -> Result<PathBuf> {
    if let Some(path) = env::var_os("GHRM_THEME_DIR") {
        return Ok(PathBuf::from(path));
    }
    if let Some(path) = dev_dir() {
        return Ok(path);
    }
    Ok(crate::dirs::data()?.join("theme"))
}

pub fn ensure() -> Result<()> {
    if env::var_os("GHRM_THEME_DIR").is_some() || dev_dir().is_some() {
        return Ok(());
    }
    let d = dir()?;
    if current(&d) {
        return Ok(());
    }
    install(&d)?;
    Ok(())
}

fn current(dest: &Path) -> bool {
    fs::read_to_string(dest.join("VERSION")).ok().as_deref() == Some(THEME_VERSION)
        && dir_matches(&CSS, dest.join("css").as_path())
        && dir_matches(&IMG, dest.join("img").as_path())
        && dir_matches(&JS, dest.join("js").as_path())
}

fn dir_matches(dir: &Dir<'_>, dest: &Path) -> bool {
    for entry in dir.entries() {
        match entry {
            DirEntry::Dir(dir) => {
                if !dest.join(dir.path()).is_dir() || !dir_matches(dir, dest) {
                    return false;
                }
            }
            DirEntry::File(file) => {
                if fs::read(dest.join(file.path())).ok().as_deref() != Some(file.contents()) {
                    return false;
                }
            }
        }
    }
    true
}

fn install(dest: &std::path::Path) -> Result<()> {
    if dest.is_dir() {
        fs::remove_dir_all(dest)?;
    }
    fs::create_dir_all(dest)?;
    install_dir(&CSS, &dest.join("css"))?;
    install_dir(&IMG, &dest.join("img"))?;
    install_dir(&JS, &dest.join("js"))?;
    fs::write(dest.join("VERSION"), THEME_VERSION)?;
    Ok(())
}

fn install_dir(dir: &Dir<'_>, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;
    for entry in dir.entries() {
        match entry {
            DirEntry::Dir(dir) => {
                fs::create_dir_all(dest.join(dir.path()))?;
                install_dir(dir, dest)?;
            }
            DirEntry::File(file) => {
                let path = dest.join(file.path());
                fs::create_dir_all(path.parent().unwrap())?;
                fs::write(path, file.contents())?;
            }
        }
    }
    Ok(())
}

pub fn clean() -> Result<()> {
    if env::var_os("GHRM_THEME_DIR").is_some() || dev_dir().is_some() {
        return Ok(());
    }
    let d = dir()?;
    if d.is_dir() {
        fs::remove_dir_all(&d)?;
    }
    Ok(())
}

pub fn spawn_dev_watch(reload_tx: broadcast::Sender<String>) -> Result<()> {
    let Some(root) = dev_dir() else {
        return Ok(());
    };
    let mut snapshot = dev_snapshot(&root)?;

    thread::spawn(move || {
        loop {
            thread::sleep(DEV_WATCH_INTERVAL);
            let Ok(next) = dev_snapshot(&root) else {
                continue;
            };
            if next != snapshot {
                snapshot = next;
                info!("theme asset change");
                let _ = reload_tx.send("reload".to_string());
            }
        }
    });
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct AssetStamp {
    len: u64,
    modified: Option<SystemTime>,
}

fn dev_snapshot(root: &Path) -> Result<BTreeMap<PathBuf, AssetStamp>> {
    let mut snapshot = BTreeMap::new();
    for dir in THEME_DIRS {
        collect_dev_snapshot(root, &root.join(dir), &mut snapshot)?;
    }
    Ok(snapshot)
}

fn collect_dev_snapshot(
    root: &Path,
    dir: &Path,
    snapshot: &mut BTreeMap<PathBuf, AssetStamp>,
) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let path = entry.path();
        if file_type.is_dir() {
            collect_dev_snapshot(root, &path, snapshot)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let meta = entry.metadata()?;
        snapshot.insert(
            path.strip_prefix(root).unwrap().to_path_buf(),
            AssetStamp {
                len: meta.len(),
                modified: meta.modified().ok(),
            },
        );
    }
    Ok(())
}

fn dev_dir() -> Option<PathBuf> {
    if !cfg!(debug_assertions) {
        return None;
    }
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    if path.join("css").is_dir() && path.join("img").is_dir() && path.join("js").is_dir() {
        Some(path)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    #[test]
    fn install_writes_only_runtime_theme_assets() {
        let td = TempDir::new("ghrm-theme-install");

        install(td.path()).unwrap();

        assert!(td.path().join("css/theme.css").is_file());
        assert!(td.path().join("css/explorer.css").is_file());
        assert!(td.path().join("css/gist.css").is_file());
        assert!(td.path().join("js/preview.js").is_file());
        assert!(td.path().join("js/main.js").is_file());
        assert!(td.path().join("js/gist.js").is_file());
        assert!(td.path().join("img/favicon.svg").is_file());
        assert!(td.path().join("VERSION").is_file());
        assert!(!td.path().join("vendor").exists());
        assert!(!td.path().join("templates").exists());
        assert!(!td.path().join("config.json").exists());
    }

    #[test]
    fn install_includes_nested_js_chunks() {
        let td = TempDir::new("ghrm-theme-chunks");

        install(td.path()).unwrap();

        assert!(
            td.path().join("js/chunks").is_dir(),
            "theme install must include js/chunks directory"
        );
        assert!(
            first_installed_chunk(td.path()).is_file(),
            "js/chunks must contain generated JavaScript files"
        );
    }

    #[test]
    fn current_rejects_missing_asset() {
        let td = TempDir::new("ghrm-theme-current");

        install(td.path()).unwrap();
        assert!(current(td.path()));

        fs::remove_file(td.path().join("js/gist.js")).unwrap();
        assert!(!current(td.path()));
    }

    #[test]
    fn current_rejects_missing_entry_script() {
        let td = TempDir::new("ghrm-theme-entry");

        install(td.path()).unwrap();
        assert!(current(td.path()));

        fs::remove_file(td.path().join("js/preview.js")).unwrap();
        assert!(
            !current(td.path()),
            "theme should be stale when preview.js is missing"
        );
    }

    #[test]
    fn current_rejects_missing_chunk() {
        let td = TempDir::new("ghrm-theme-chunk");

        install(td.path()).unwrap();
        assert!(current(td.path()));

        fs::remove_file(first_installed_chunk(td.path())).unwrap();
        assert!(
            !current(td.path()),
            "theme should be stale when a js/chunks file is missing"
        );
    }

    #[test]
    fn current_rejects_stale_chunk_content() {
        let td = TempDir::new("ghrm-theme-chunk-stale");

        install(td.path()).unwrap();
        assert!(current(td.path()));

        fs::write(first_installed_chunk(td.path()), b"stale content").unwrap();
        assert!(
            !current(td.path()),
            "theme should be stale when a js/chunks file has wrong content"
        );
    }

    fn first_installed_chunk(root: &Path) -> PathBuf {
        fs::read_dir(root.join("js/chunks"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| path.extension().is_some_and(|ext| ext == "js"))
            .expect("js/chunks must contain generated JavaScript files")
    }
}
