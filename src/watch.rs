use crate::paths;
use crate::walk::{self, NavSet};
use ignore::gitignore::GitignoreBuilder;
use notify::RecursiveMode;
use notify_debouncer_full::{DebouncedEvent, new_debouncer};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::{info, warn};

pub struct NavCache {
    pub current: Arc<RwLock<NavSet>>,
    pub alternate: Arc<RwLock<Option<NavSet>>>,
}

pub fn spawn_dir(
    root: PathBuf,
    nav: NavCache,
    reload_tx: broadcast::Sender<&'static str>,
    use_ignore: bool,
    exclude_names: Vec<String>,
    extensions: Vec<String>,
    no_excludes: bool,
) -> anyhow::Result<()> {
    let (tx, rx) = std::sync::mpsc::channel::<Result<Vec<DebouncedEvent>, Vec<notify::Error>>>();
    let mut debouncer = new_debouncer(Duration::from_millis(150), None, tx)?;

    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel::<anyhow::Result<()>>(1);

    std::thread::spawn(move || {
        if let Err(e) = debouncer.watch(&root, RecursiveMode::Recursive) {
            warn!("watcher setup failed, live reload disabled: {e}");
            let _ = ready_tx.send(Ok(()));
            return;
        }
        let _ = ready_tx.send(Ok(()));
        let _debouncer = debouncer;
        for res in rx {
            let events = match res {
                Ok(ev) => ev,
                Err(_) => continue,
            };
            if events.is_empty() {
                continue;
            }
            let changed = changed_paths(&root, &events, use_ignore, &exclude_names, None);
            if changed.is_empty() {
                continue;
            }
            let nav_dirty = events
                .iter()
                .any(|e| is_nav_event(&root, e, use_ignore, &exclude_names, None));
            if nav_dirty {
                if let Ok(mut guard) = nav.alternate.write() {
                    *guard = None;
                }
                let fresh =
                    walk::build_all(&root, use_ignore, &exclude_names, &extensions, no_excludes);
                if let Ok(mut guard) = nav.current.write() {
                    *guard = fresh;
                }
            }
            for p in changed {
                let rel = p.strip_prefix(&root).unwrap_or(&p).display();
                info!(
                    kind = if nav_dirty { "nav+reload" } else { "reload" },
                    path = %rel,
                    "change"
                );
            }
            let _ = reload_tx.send("reload");
        }
    });

    if let Ok(Err(e)) = ready_rx.recv_timeout(Duration::from_millis(50)) {
        return Err(e);
    }
    Ok(())
}

pub fn spawn_file(file: PathBuf, reload_tx: broadcast::Sender<&'static str>) -> anyhow::Result<()> {
    let (tx, rx) = std::sync::mpsc::channel::<Result<Vec<DebouncedEvent>, Vec<notify::Error>>>();
    let mut debouncer = new_debouncer(Duration::from_millis(120), None, tx)?;
    let parent = file.parent().unwrap_or(Path::new(".")).to_path_buf();
    debouncer.watch(&parent, RecursiveMode::NonRecursive)?;

    std::thread::spawn(move || {
        let _debouncer = debouncer;
        for res in rx {
            let Ok(events) = res else { continue };
            if events
                .iter()
                .any(|e| e.event.paths.iter().any(|p| p == &file))
            {
                info!(path = %file.display(), "change");
                let _ = reload_tx.send("reload");
            }
        }
    });
    Ok(())
}

fn changed_paths(
    root: &Path,
    events: &[DebouncedEvent],
    use_ignore: bool,
    exclude_names: &[String],
    global_ignore: Option<&Path>,
) -> Vec<PathBuf> {
    use notify::event::EventKind;
    let mut seen: Vec<PathBuf> = Vec::new();
    for ev in events {
        if matches!(ev.event.kind, EventKind::Access(_)) {
            continue;
        }
        for p in &ev.event.paths {
            if !is_relevant_watch_path(root, p, use_ignore, exclude_names, global_ignore) {
                continue;
            }
            if !seen.contains(p) {
                seen.push(p.clone());
            }
        }
    }
    seen
}

fn is_nav_event(
    root: &Path,
    ev: &DebouncedEvent,
    use_ignore: bool,
    exclude_names: &[String],
    global_ignore: Option<&Path>,
) -> bool {
    use notify::event::{EventKind, ModifyKind};
    let kind_nav = matches!(
        ev.event.kind,
        EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(_))
    );
    if !kind_nav {
        return false;
    }
    ev.event
        .paths
        .iter()
        .any(|p| is_relevant_watch_path(root, p, use_ignore, exclude_names, global_ignore))
}

fn is_relevant_watch_path(
    root: &Path,
    path: &Path,
    use_ignore: bool,
    exclude_names: &[String],
    global_ignore: Option<&Path>,
) -> bool {
    let Ok(rel) = path.strip_prefix(root) else {
        return false;
    };
    if paths::has_excluded_part(rel, exclude_names) {
        return false;
    }
    if !use_ignore {
        return true;
    }
    !matches_ignore(root, rel, path_kind(path), global_ignore)
}

fn matches_ignore(
    root: &Path,
    rel: &Path,
    is_dir: Option<bool>,
    global_ignore: Option<&Path>,
) -> bool {
    match is_dir {
        Some(is_dir) => ignore_state(root, rel, is_dir, global_ignore).is_ignore(),
        None => {
            let file_match = ignore_state(root, rel, false, global_ignore);
            if file_match.is_whitelist() {
                return false;
            }
            let dir_match = ignore_state(root, rel, true, global_ignore);
            if dir_match.is_whitelist() {
                return false;
            }
            file_match.is_ignore() || dir_match.is_ignore()
        }
    }
}

fn path_kind(path: &Path) -> Option<bool> {
    std::fs::metadata(path).ok().map(|meta| meta.is_dir())
}

fn ignore_state(root: &Path, rel: &Path, is_dir: bool, global_ignore: Option<&Path>) -> MatchState {
    match ignore_match(root, rel, is_dir, ".ignore") {
        MatchState::None => {}
        state => return state,
    }
    match ignore_match(root, rel, is_dir, ".gitignore") {
        MatchState::None => {}
        state => return state,
    }
    match ignore_match(root, rel, is_dir, ".git/info/exclude") {
        MatchState::None => {}
        state => return state,
    }
    global_ignore_match(root, rel, is_dir, global_ignore)
}

fn ignore_match(root: &Path, rel: &Path, is_dir: bool, name: &str) -> MatchState {
    let Some(parent_dirs) = path_ancestors(rel) else {
        return MatchState::None;
    };
    let mut builder = GitignoreBuilder::new(root);
    let mut found = false;
    for dir in parent_dirs {
        let path = root.join(dir).join(name);
        if !path.is_file() {
            continue;
        }
        let _ = builder.add(path);
        found = true;
    }
    if !found {
        return MatchState::None;
    }
    let Ok(matcher) = builder.build() else {
        return MatchState::None;
    };
    MatchState::from_match(matcher.matched_path_or_any_parents(rel, is_dir))
}

fn global_ignore_match(
    root: &Path,
    rel: &Path,
    is_dir: bool,
    explicit: Option<&Path>,
) -> MatchState {
    let mut builder = GitignoreBuilder::new(root);
    let matcher = match explicit {
        Some(path) => {
            let _ = builder.add(path);
            match builder.build() {
                Ok(m) => m,
                Err(_) => return MatchState::None,
            }
        }
        None => builder.build_global().0,
    };
    MatchState::from_match(matcher.matched_path_or_any_parents(rel, is_dir))
}

fn path_ancestors(rel: &Path) -> Option<Vec<PathBuf>> {
    let parent = rel.parent()?;
    let mut dirs = vec![PathBuf::new()];
    let mut current = PathBuf::new();
    for part in parent.iter() {
        current.push(part);
        dirs.push(current.clone());
    }
    Some(dirs)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MatchState {
    None,
    Ignore,
    Whitelist,
}

impl MatchState {
    fn from_match<T>(matched: ignore::Match<T>) -> Self {
        match matched {
            ignore::Match::None => Self::None,
            ignore::Match::Ignore(_) => Self::Ignore,
            ignore::Match::Whitelist(_) => Self::Whitelist,
        }
    }

    fn is_ignore(self) -> bool {
        matches!(self, Self::Ignore)
    }

    fn is_whitelist(self) -> bool {
        matches!(self, Self::Whitelist)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;
    use notify::Event;
    use std::fs;
    use std::time::Instant;

    #[test]
    fn changed_paths_skip_excluded_names() {
        let td = TempDir::new("ghrm-watch-test");
        let path = td.path().join(".venv/bin/python");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "").unwrap();

        let changed = changed_paths(
            td.path(),
            &[DebouncedEvent::new(
                Event::default().add_path(path),
                Instant::now(),
            )],
            true,
            &[".venv".to_string()],
            None,
        );

        assert!(changed.is_empty());
    }

    #[test]
    fn changed_paths_honor_gitignore() {
        let td = TempDir::new("ghrm-watch-test");
        fs::write(td.path().join(".gitignore"), ".venv/\n").unwrap();
        let path = td.path().join(".venv/bin/python");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "").unwrap();

        let changed = changed_paths(
            td.path(),
            &[DebouncedEvent::new(
                Event::default().add_path(path),
                Instant::now(),
            )],
            true,
            &[],
            None,
        );

        assert!(changed.is_empty());
    }

    #[test]
    fn changed_paths_honor_global_gitignore() {
        let td = TempDir::new("ghrm-watch-test");
        let ignore_file = td.path().join("global-ignore");
        fs::write(&ignore_file, ".venv/\n").unwrap();

        let path = td.path().join(".venv/bin/python");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "").unwrap();

        let changed = changed_paths(
            td.path(),
            &[DebouncedEvent::new(
                Event::default().add_path(path),
                Instant::now(),
            )],
            true,
            &[],
            Some(&ignore_file),
        );

        assert!(changed.is_empty());
    }

    #[test]
    fn changed_paths_keep_whitelisted_paths() {
        let td = TempDir::new("ghrm-watch-test");
        fs::write(td.path().join(".gitignore"), "*\n!notes.md\n").unwrap();
        let path = td.path().join("notes.md");
        fs::write(&path, "# notes\n").unwrap();

        let changed = changed_paths(
            td.path(),
            &[DebouncedEvent::new(
                Event::default().add_path(path.clone()),
                Instant::now(),
            )],
            true,
            &[],
            None,
        );

        assert_eq!(changed, vec![path]);
    }
}
