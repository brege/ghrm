use super::refs::CommitEntry;
use super::{CommitInfo, path_key};

use gix::bstr::BString;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

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
    let result = ghrm_git::walk(&repo, |commit| {
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
    ghrm_git::walk(&repo, |commit| {
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
