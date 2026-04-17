use crate::walk::{self, NavTree};
use notify::{RecursiveMode, Watcher};
use notify_debouncer_full::{new_debouncer, DebouncedEvent};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::info;

pub fn spawn_dir(
    root: PathBuf,
    nav: Arc<RwLock<NavTree>>,
    reload_tx: broadcast::Sender<()>,
) -> anyhow::Result<()> {
    let (tx, rx) = std::sync::mpsc::channel::<Result<Vec<DebouncedEvent>, Vec<notify::Error>>>();
    let mut debouncer = new_debouncer(Duration::from_millis(150), None, tx)?;
    debouncer
        .watcher()
        .watch(&root, RecursiveMode::Recursive)?;

    std::thread::spawn(move || {
        let _debouncer = debouncer;
        for res in rx {
            let events = match res {
                Ok(ev) => ev,
                Err(_) => continue,
            };
            if events.is_empty() {
                continue;
            }
            let nav_dirty = events.iter().any(|e| is_nav_event(&root, e));
            if nav_dirty {
                let fresh = walk::build(&root);
                if let Ok(mut guard) = nav.write() {
                    *guard = fresh;
                }
            }
            for p in changed_paths(&root, &events) {
                let rel = p.strip_prefix(&root).unwrap_or(&p).display();
                info!(
                    kind = if nav_dirty { "nav+reload" } else { "reload" },
                    path = %rel,
                    "change"
                );
            }
            let _ = reload_tx.send(());
        }
    });
    Ok(())
}

pub fn spawn_file(
    file: PathBuf,
    reload_tx: broadcast::Sender<()>,
) -> anyhow::Result<()> {
    let (tx, rx) = std::sync::mpsc::channel::<Result<Vec<DebouncedEvent>, Vec<notify::Error>>>();
    let mut debouncer = new_debouncer(Duration::from_millis(120), None, tx)?;
    let parent = file.parent().unwrap_or(Path::new(".")).to_path_buf();
    debouncer
        .watcher()
        .watch(&parent, RecursiveMode::NonRecursive)?;

    std::thread::spawn(move || {
        let _debouncer = debouncer;
        for res in rx {
            let Ok(events) = res else { continue };
            if events.iter().any(|e| e.event.paths.iter().any(|p| p == &file)) {
                info!(path = %file.display(), "change");
                let _ = reload_tx.send(());
            }
        }
    });
    Ok(())
}

fn changed_paths(root: &Path, events: &[DebouncedEvent]) -> Vec<PathBuf> {
    let mut seen: Vec<PathBuf> = Vec::new();
    for ev in events {
        for p in &ev.event.paths {
            let rel = p.strip_prefix(root).unwrap_or(p);
            let rel_s = rel.to_string_lossy();
            if rel_s.contains("/.git/") || rel_s.starts_with(".git") {
                continue;
            }
            if rel_s.contains("node_modules") || rel_s.contains(".venv") {
                continue;
            }
            if !seen.contains(p) {
                seen.push(p.clone());
            }
        }
    }
    seen
}

fn is_nav_event(root: &Path, ev: &DebouncedEvent) -> bool {
    use notify::event::{EventKind, ModifyKind};
    let kind_nav = matches!(
        ev.event.kind,
        EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(_))
    );
    if !kind_nav {
        return false;
    }
    ev.event.paths.iter().any(|p| {
        let Ok(rel) = p.strip_prefix(root) else { return false };
        let rel_s = rel.to_string_lossy();
        if rel_s.contains("/.git/") || rel_s.starts_with(".git") {
            return false;
        }
        if rel_s.contains("node_modules") || rel_s.contains(".venv") {
            return false;
        }
        if p.extension().and_then(|s| s.to_str()) == Some("md") {
            return true;
        }
        // directory events (no extension) are candidates too
        p.extension().is_none()
    })
}
