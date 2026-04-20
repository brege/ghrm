use crate::walk::{self, NavSet};
use notify::{RecursiveMode, Watcher};
use notify_debouncer_full::{DebouncedEvent, new_debouncer};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::{error, info};

pub fn spawn_dir(
    root: PathBuf,
    nav: Arc<RwLock<NavSet>>,
    reload_tx: broadcast::Sender<()>,
    use_ignore: bool,
) -> anyhow::Result<()> {
    let (tx, rx) = std::sync::mpsc::channel::<Result<Vec<DebouncedEvent>, Vec<notify::Error>>>();
    let mut debouncer = new_debouncer(Duration::from_millis(150), None, tx)?;

    // Propagate the watch result through a channel. Block briefly to catch
    // fast failures (bad path, permission denied). For large trees, watch()
    // takes O(n_dirs) inotify calls; after the timeout we proceed so the
    // server starts immediately and the watcher finishes in the background.
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel::<anyhow::Result<()>>(1);

    std::thread::spawn(move || {
        let result = debouncer
            .watcher()
            .watch(&root, RecursiveMode::Recursive)
            .map_err(anyhow::Error::from);
        let failed = result.is_err();
        if let Err(ref e) = result {
            error!("watcher failed: {e}");
        }
        let _ = ready_tx.send(result);
        if failed {
            return;
        }
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
                let fresh = walk::build_all(&root, use_ignore);
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

    if let Ok(Err(e)) = ready_rx.recv_timeout(Duration::from_millis(50)) {
        return Err(e);
    }
    Ok(())
}

pub fn spawn_file(file: PathBuf, reload_tx: broadcast::Sender<()>) -> anyhow::Result<()> {
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
            if events
                .iter()
                .any(|e| e.event.paths.iter().any(|p| p == &file))
            {
                info!(path = %file.display(), "change");
                let _ = reload_tx.send(());
            }
        }
    });
    Ok(())
}

// Returns true if any component of rel is in the walk skip list,
// keeping event filtering consistent with walk::allow_walk_name.
fn skip_watch_path(rel: &Path) -> bool {
    rel.components().any(|c| match c {
        Component::Normal(name) => matches!(
            name.to_string_lossy().as_ref(),
            ".git"
                | "node_modules"
                | "__pycache__"
                | "target"
                | ".venv"
                | ".env"
                | ".pytest_cache"
                | ".ruff_cache"
                | ".uv-cache"
                | ".ipynb_checkpoints"
        ),
        _ => false,
    })
}

fn changed_paths(root: &Path, events: &[DebouncedEvent]) -> Vec<PathBuf> {
    let mut seen: Vec<PathBuf> = Vec::new();
    for ev in events {
        for p in &ev.event.paths {
            let rel = p.strip_prefix(root).unwrap_or(p);
            if skip_watch_path(rel) {
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
        let Ok(rel) = p.strip_prefix(root) else {
            return false;
        };
        !skip_watch_path(rel)
    })
}
