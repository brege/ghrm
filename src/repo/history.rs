use super::refs::CommitEntry;
use super::{CommitInfo, path_key};

use anyhow::{Context, Result};
use gix::bstr::{BString, ByteSlice};
use gix::traverse::commit::simple::CommitTimeOrder;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Commit {
    pub id: gix::ObjectId,
    pub time: u64,
    pub author_name: String,
    pub author_email: String,
    pub subject: String,
    pub changed: BTreeSet<BString>,
}

pub(crate) fn walk(
    repo: &gix::Repository,
    mut visit: impl FnMut(Commit) -> ControlFlow<()>,
) -> Result<()> {
    let mut head = repo.head().context("read HEAD")?;
    let Some(head) = head.try_peel_to_id().context("resolve HEAD")? else {
        return Ok(());
    };
    let mut cache = repo
        .diff_resource_cache(gix::diff::blob::pipeline::Mode::ToGit, Default::default())
        .context("create diff resource cache")?;
    let commits = head
        .ancestors()
        .sorting(gix::revision::walk::Sorting::ByCommitTime(
            CommitTimeOrder::NewestFirst,
        ))
        .all()
        .context("start history walk")?;

    for info in commits {
        let commit = read_commit(repo, info.context("walk history")?, &mut cache)?;
        if visit(commit).is_break() {
            break;
        }
    }
    Ok(())
}

fn read_commit(
    repo: &gix::Repository,
    info: gix::revision::walk::Info<'_>,
    cache: &mut gix::diff::blob::Platform,
) -> Result<Commit> {
    let id = info.id;
    let commit = info.object().context("read commit")?;
    let author = commit.author().context("read commit author")?;
    let time = u64::try_from(commit.time().context("read commit time")?.seconds)
        .context("commit time predates the Unix epoch")?;
    let subject = commit.message().context("read commit message")?.summary();

    Ok(Commit {
        id,
        time,
        author_name: author.name.to_str_lossy().into_owned(),
        author_email: author.email.to_str_lossy().into_owned(),
        subject: subject.to_string(),
        changed: changed_paths(repo, &commit, cache)?,
    })
}

// A root commit is compared with the empty tree. A single-parent commit is
// compared with its parent. Merge commits have no changed paths, matching
// `git log --name-only` without a merge diff format.
fn changed_paths(
    repo: &gix::Repository,
    commit: &gix::Commit<'_>,
    cache: &mut gix::diff::blob::Platform,
) -> Result<BTreeSet<BString>> {
    let mut parents = commit.parent_ids();
    let parent = parents.next();
    if parents.next().is_some() {
        return Ok(BTreeSet::new());
    }

    let commit_tree = commit.tree().context("read commit tree")?;
    let parent_tree = match parent {
        Some(parent) => repo
            .find_commit(parent.detach())
            .context("read parent commit")?
            .tree()
            .context("read parent tree")?,
        None => repo.empty_tree(),
    };
    let mut paths = BTreeSet::new();
    let mut changes = parent_tree.changes().context("prepare tree diff")?;
    cache.clear_resource_cache();
    changes
        .for_each_to_obtain_tree_with_cache(&commit_tree, cache, |change| {
            use gix::object::tree::diff::Change;
            let (location, entry_mode) = match change {
                Change::Addition {
                    location,
                    entry_mode,
                    ..
                }
                | Change::Deletion {
                    location,
                    entry_mode,
                    ..
                }
                | Change::Modification {
                    location,
                    entry_mode,
                    ..
                }
                | Change::Rewrite {
                    location,
                    entry_mode,
                    ..
                } => (location, entry_mode),
            };
            if !entry_mode.is_tree() {
                paths.insert(location.to_owned());
            }
            Ok::<_, std::convert::Infallible>(ControlFlow::Continue(()))
        })
        .context("diff commit tree")?;
    Ok(paths)
}

struct Request {
    abs: PathBuf,
    rel: BString,
    is_dir: bool,
}

// A changed path resolves a request when it equals the request path or,
// for a directory request, lies beneath it.
fn path_matches(request: &Request, changed: &[u8]) -> bool {
    if changed == request.rel.as_slice() {
        return true;
    }
    request.is_dir
        && changed
            .strip_prefix(request.rel.as_slice())
            .is_some_and(|rest| rest.starts_with(b"/"))
}

// A pathspec matches a changed path when it equals the spec or lies
// beneath it, mirroring `git log -- <spec>`.
fn pathspec_matches(spec: &[u8], changed: &BTreeSet<BString>) -> bool {
    changed.iter().any(|path| {
        path.as_slice() == spec
            || path
                .as_slice()
                .strip_prefix(spec)
                .is_some_and(|rest| rest.starts_with(b"/"))
    })
}

// The newest commit that touched each requested path. The walk stops once
// every request resolves; unresolved paths are absent from the map.
pub(super) fn commit_info(root: &Path, paths: &[PathBuf]) -> BTreeMap<PathBuf, CommitInfo> {
    let mut out = BTreeMap::new();
    let Ok(repo) = gix::open(root) else {
        return out;
    };
    let requests = paths
        .iter()
        .filter_map(|path| {
            let rel = path.strip_prefix(root).ok()?;
            if rel.as_os_str().is_empty() {
                return None;
            }
            Some(Request {
                abs: path.clone(),
                rel: path_key(rel).into(),
                is_dir: path.is_dir(),
            })
        })
        .collect::<Vec<_>>();
    if requests.is_empty() {
        return out;
    }
    let result = walk(&repo, |commit| {
        for request in &requests {
            if out.contains_key(&request.abs) {
                continue;
            }
            if commit
                .changed
                .iter()
                .any(|changed| path_matches(request, changed.as_slice()))
            {
                out.insert(
                    request.abs.clone(),
                    CommitInfo {
                        subject: commit.subject.clone(),
                        author: commit.author_name.clone(),
                        email: commit.author_email.clone(),
                        timestamp: commit.time,
                    },
                );
            }
        }
        if out.len() == requests.len() {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    });
    if result.is_err() {
        out.clear();
    }
    out
}

// The most recent commits, up to limit, that touched rel. A reference or
// object read failure returns None; an unborn HEAD returns an empty list.
pub(super) fn recent_commits(root: &Path, rel: &str, limit: usize) -> Option<Vec<CommitEntry>> {
    let repo = gix::open(root).ok()?;
    let mut commits = Vec::new();
    walk(&repo, |commit| {
        if pathspec_matches(rel.as_bytes(), &commit.changed) {
            let id = commit.id.to_string();
            let label = id.get(..7).unwrap_or(&id).to_string();
            commits.push(CommitEntry {
                value: id,
                label,
                timestamp: commit.time,
                subject: commit.subject.clone(),
            });
            if commits.len() >= limit {
                return ControlFlow::Break(());
            }
        }
        ControlFlow::Continue(())
    })
    .ok()?;
    Some(commits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;
    use gix::objs::tree::EntryKind;
    use std::fs;

    fn init_repo(dir: &Path) -> gix::Repository {
        gix::init(dir).expect("init repository");
        gix::open(dir).expect("open repository")
    }

    fn write_tree(repo: &gix::Repository, files: &[(&str, &[u8])]) -> gix::ObjectId {
        let empty = gix::ObjectId::empty_tree(repo.object_hash());
        let mut editor = repo.edit_tree(empty).expect("edit tree");
        for (path, bytes) in files {
            let blob = repo.write_blob(bytes).expect("write blob").detach();
            editor.upsert(*path, EntryKind::Blob, blob).expect("upsert");
        }
        editor.write().expect("write tree").detach()
    }

    fn commit(
        repo: &gix::Repository,
        files: &[(&str, &[u8])],
        parents: &[gix::ObjectId],
        message: &str,
    ) -> gix::ObjectId {
        let tree = write_tree(repo, files);
        let sig = gix::actor::SignatureRef {
            name: "Test".into(),
            email: "test@example.com".into(),
            time: "1000000000 +0000",
        };
        repo.commit_as(sig, sig, "HEAD", message, tree, parents.iter().copied())
            .expect("commit")
            .detach()
    }

    #[test]
    fn commit_info_reports_the_newest_commit_per_path() {
        let td = TempDir::new("ghrm-history-info");
        let repo = init_repo(td.path());
        let c1 = commit(&repo, &[("a.txt", b"a1\n")], &[], "add a");
        let c2 = commit(
            &repo,
            &[("a.txt", b"a2\n"), ("b.txt", b"b\n")],
            &[c1],
            "edit a, add b",
        );
        commit(
            &repo,
            &[("a.txt", b"a2\n"), ("b.txt", b"b\n"), ("c.txt", b"c\n")],
            &[c2],
            "add c",
        );

        let root = td.path();
        let info = commit_info(root, &[root.join("a.txt"), root.join("b.txt")]);

        assert_eq!(info[&root.join("a.txt")].subject, "edit a, add b");
        assert_eq!(info[&root.join("a.txt")].author, "Test");
        assert_eq!(info[&root.join("b.txt")].subject, "edit a, add b");
        assert!(info[&root.join("a.txt")].timestamp > 0);
    }

    #[test]
    fn commit_info_matches_a_directory_request_to_files_beneath_it() {
        let td = TempDir::new("ghrm-history-dir");
        let repo = init_repo(td.path());
        let c1 = commit(&repo, &[("src/main.rs", b"fn main() {}\n")], &[], "add src");
        commit(
            &repo,
            &[("src/main.rs", b"fn main() {}\n"), ("README.md", b"# x\n")],
            &[c1],
            "add readme",
        );

        let root = td.path();
        // commit_info reads is_dir from the worktree, which gix commits do
        // not populate, so mirror the directory on disk
        fs::create_dir_all(root.join("src")).unwrap();
        let info = commit_info(root, &[root.join("src")]);

        assert_eq!(info[&root.join("src")].subject, "add src");
    }

    #[test]
    fn recent_commits_lists_only_commits_touching_the_path() {
        let td = TempDir::new("ghrm-history-recent");
        let repo = init_repo(td.path());
        let c1 = commit(&repo, &[("a.txt", b"a1\n")], &[], "first");
        let c2 = commit(
            &repo,
            &[("a.txt", b"a1\n"), ("b.txt", b"b\n")],
            &[c1],
            "second",
        );
        commit(
            &repo,
            &[("a.txt", b"a3\n"), ("b.txt", b"b\n")],
            &[c2],
            "third",
        );

        let commits = recent_commits(td.path(), "a.txt", 30).expect("recent commits");

        let subjects = commits
            .iter()
            .map(|c| c.subject.as_str())
            .collect::<Vec<_>>();
        assert_eq!(subjects, vec!["third", "first"]);
        assert_eq!(commits[0].label.len(), 7);
        assert!(commits[0].value.len() > 7);
        assert!(commits[0].value.starts_with(&commits[0].label));
    }

    #[test]
    fn recent_commits_honors_the_limit() {
        let td = TempDir::new("ghrm-history-limit");
        let repo = init_repo(td.path());
        let mut parent: Vec<gix::ObjectId> = Vec::new();
        for index in 0..5 {
            let id = commit(
                &repo,
                &[("a.txt", format!("v{index}\n").as_bytes())],
                &parent,
                &format!("edit {index}"),
            );
            parent = vec![id];
        }

        let commits = recent_commits(td.path(), "a.txt", 2).expect("recent commits");

        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].subject, "edit 4");
        assert_eq!(commits[1].subject, "edit 3");
    }

    #[test]
    fn recent_commits_is_empty_for_an_unborn_head() {
        let td = TempDir::new("ghrm-history-empty");
        init_repo(td.path());

        let commits = recent_commits(td.path(), "a.txt", 30).expect("recent commits");

        assert!(commits.is_empty());
    }

    #[test]
    fn path_matches_files_and_directory_children() {
        let file = Request {
            abs: PathBuf::from("/repo/src/view.rs"),
            rel: "src/view.rs".into(),
            is_dir: false,
        };
        assert!(path_matches(&file, b"src/view.rs"));
        assert!(!path_matches(&file, b"src/view.rs.bak"));

        let dir = Request {
            abs: PathBuf::from("/repo/src"),
            rel: "src".into(),
            is_dir: true,
        };
        assert!(path_matches(&dir, b"src/view.rs"));
        assert!(!path_matches(&dir, b"src-old/view.rs"));
    }
}

#[cfg(test)]
mod walk_tests {
    use super::*;
    use gix::objs::tree::EntryKind;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("ghrm-history-{name}-{}-{id}", std::process::id()));
            fs::create_dir_all(&path).expect("create temp directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).ok();
        }
    }

    fn init_repo(dir: &Path) -> gix::Repository {
        gix::init(dir).expect("init repository");
        gix::open(dir).expect("open repository")
    }

    fn write_tree(repo: &gix::Repository, files: &[(BString, &[u8])]) -> gix::ObjectId {
        let empty = gix::ObjectId::empty_tree(repo.object_hash());
        let mut editor = repo.edit_tree(empty).expect("edit tree");
        for (path, bytes) in files {
            let blob = repo.write_blob(bytes).expect("write blob").detach();
            editor
                .upsert(path.as_bstr(), EntryKind::Blob, blob)
                .expect("upsert");
        }
        editor.write().expect("write tree").detach()
    }

    fn commit(
        repo: &gix::Repository,
        reference: &str,
        files: &[(BString, &[u8])],
        parents: &[gix::ObjectId],
        seconds: u64,
        message: &str,
    ) -> gix::ObjectId {
        let tree = write_tree(repo, files);
        let time = format!("{seconds} +0000");
        let sig = gix::actor::SignatureRef {
            name: "Test".into(),
            email: "test@example.com".into(),
            time: &time,
        };
        repo.commit_as(sig, sig, reference, message, tree, parents.iter().copied())
            .expect("commit")
            .detach()
    }

    fn file(path: &str, contents: &'static [u8]) -> (BString, &'static [u8]) {
        (path.into(), contents)
    }

    #[test]
    fn walks_branches_by_commit_time_and_omits_merge_paths() {
        let dir = TempDir::new("ordering");
        let repo = init_repo(dir.path());
        let root = commit(
            &repo,
            "HEAD",
            &[file("root.txt", b"root\n")],
            &[],
            100,
            "root",
        );
        let main = commit(
            &repo,
            "HEAD",
            &[file("root.txt", b"root\n"), file("main.txt", b"main\n")],
            &[root],
            200,
            "main",
        );
        let side = commit(
            &repo,
            "refs/heads/side",
            &[file("root.txt", b"root\n"), file("side.txt", b"side\n")],
            &[root],
            300,
            "side",
        );
        commit(
            &repo,
            "HEAD",
            &[
                file("root.txt", b"root\n"),
                file("main.txt", b"main\n"),
                file("side.txt", b"side\n"),
            ],
            &[main, side],
            400,
            "merge",
        );

        let mut commits = Vec::new();
        walk(&repo, |commit| {
            commits.push(commit);
            ControlFlow::Continue(())
        })
        .expect("walk");

        let subjects = commits
            .iter()
            .map(|commit| commit.subject.as_str())
            .collect::<Vec<_>>();
        assert_eq!(subjects, vec!["merge", "side", "main", "root"]);
        assert!(commits[0].changed.is_empty());
        assert!(commits[1].changed.contains("side.txt".as_bytes().as_bstr()));
    }

    #[test]
    fn reports_only_the_destination_of_a_rename() {
        let dir = TempDir::new("rename");
        let repo = init_repo(dir.path());
        let root = commit(
            &repo,
            "HEAD",
            &[file("old.txt", b"same\n")],
            &[],
            100,
            "old",
        );
        commit(
            &repo,
            "HEAD",
            &[file("new.txt", b"same\n")],
            &[root],
            200,
            "rename",
        );

        let mut changed = BTreeSet::new();
        walk(&repo, |commit| {
            if commit.subject == "rename" {
                changed = commit.changed;
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        })
        .expect("walk");

        assert!(changed.contains("new.txt".as_bytes().as_bstr()));
        assert!(!changed.contains("old.txt".as_bytes().as_bstr()));
    }

    #[cfg(unix)]
    #[test]
    fn retains_non_utf8_paths() {
        let dir = TempDir::new("bytes");
        let repo = init_repo(dir.path());
        let path = BString::from(vec![b'f', 0xff]);
        commit(
            &repo,
            "HEAD",
            &[(path.clone(), b"bytes\n")],
            &[],
            100,
            "bytes",
        );

        let mut changed = BTreeSet::new();
        walk(&repo, |commit| {
            changed = commit.changed;
            ControlFlow::Break(())
        })
        .expect("walk");

        assert!(changed.contains(path.as_bstr()));
    }

    #[test]
    fn propagates_missing_parent_objects() {
        let dir = TempDir::new("missing-parent");
        let repo = init_repo(dir.path());
        let parent = commit(
            &repo,
            "HEAD",
            &[file("file.txt", b"one\n")],
            &[],
            100,
            "parent",
        );
        commit(
            &repo,
            "HEAD",
            &[file("file.txt", b"two\n")],
            &[parent],
            200,
            "child",
        );
        drop(repo);
        let hex = parent.to_string();
        fs::remove_file(
            dir.path()
                .join(".git/objects")
                .join(&hex[..2])
                .join(&hex[2..]),
        )
        .expect("remove parent object");
        let repo = gix::open(dir.path()).expect("reopen repository");

        assert!(walk(&repo, |_| ControlFlow::Continue(())).is_err());
    }

    #[test]
    fn stops_when_the_consumer_breaks() {
        let dir = TempDir::new("break");
        let repo = init_repo(dir.path());
        let root = commit(&repo, "HEAD", &[file("a.txt", b"a\n")], &[], 100, "root");
        commit(
            &repo,
            "HEAD",
            &[file("a.txt", b"b\n")],
            &[root],
            200,
            "head",
        );
        let mut visited = 0;

        walk(&repo, |_| {
            visited += 1;
            ControlFlow::Break(())
        })
        .expect("walk");

        assert_eq!(visited, 1);
    }

    #[test]
    fn unborn_head_walks_no_commits() {
        let dir = TempDir::new("unborn");
        let repo = init_repo(dir.path());
        let mut visited = false;

        walk(&repo, |_| {
            visited = true;
            ControlFlow::Continue(())
        })
        .expect("walk");

        assert!(!visited);
    }
}
