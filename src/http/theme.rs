use anyhow::Result;
use include_dir::{Dir, DirEntry, include_dir};
use notify::RecursiveMode;
use notify_debouncer_full::{DebouncedEvent, new_debouncer};
use std::{
    env, fs,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::sync::broadcast;
use tracing::info;

const THEME_VERSION: &str = include_str!(concat!(env!("OUT_DIR"), "/theme_version.txt"));
const THEME_DIRS: &[&str] = &["css", "img", "js"];

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
    let (tx, rx) = std::sync::mpsc::channel::<Result<Vec<DebouncedEvent>, Vec<notify::Error>>>();
    let mut debouncer = new_debouncer(Duration::from_millis(120), None, tx)?;
    for dir in THEME_DIRS {
        debouncer.watch(root.join(dir), RecursiveMode::Recursive)?;
    }

    std::thread::spawn(move || {
        let _debouncer = debouncer;
        for res in rx {
            let Ok(events) = res else {
                continue;
            };
            if events
                .iter()
                .all(|event| matches!(event.event.kind, notify::event::EventKind::Access(_)))
            {
                continue;
            }
            info!("theme asset change");
            let _ = reload_tx.send("reload".to_string());
        }
    });
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
        assert!(td.path().join("js/main.js").is_file());
        assert!(td.path().join("js/gist.js").is_file());
        assert!(td.path().join("img/favicon.svg").is_file());
        assert!(td.path().join("VERSION").is_file());
        assert!(!td.path().join("vendor").exists());
        assert!(!td.path().join("templates").exists());
        assert!(!td.path().join("config.json").exists());
    }

    #[test]
    fn current_rejects_missing_asset() {
        let td = TempDir::new("ghrm-theme-current");

        install(td.path()).unwrap();
        assert!(current(td.path()));

        fs::remove_file(td.path().join("js/gist.js")).unwrap();
        assert!(!current(td.path()));
    }
}
