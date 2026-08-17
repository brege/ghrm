use anyhow::{Context, Result};
use gix::bstr::{BString, ByteSlice};
use gix::traverse::commit::simple::CommitTimeOrder;
use std::collections::BTreeSet;
use std::ops::ControlFlow;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commit {
    pub id: gix::ObjectId,
    pub time: u64,
    pub author_name: String,
    pub author_email: String,
    pub subject: String,
    pub changed: BTreeSet<BString>,
}

pub fn walk(
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

#[cfg(test)]
mod tests {
    use super::*;
    use gix::objs::tree::EntryKind;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("ghrm-git-{name}-{}-{id}", std::process::id()));
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
