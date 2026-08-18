use crate::stat::{Row, RowMetric};
use anyhow::{Context, Result};
use gix::bstr::ByteSlice;
use std::collections::HashMap;
use std::ops::ControlFlow;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Default)]
pub struct History {
    pub commits: usize,
    pub authors: Vec<Author>,
    pub churn: Vec<Churn>,
    pub first_commit: Option<u64>,
    pub last_commit: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct Author {
    pub name: String,
    pub commits: usize,
    pub contribution: usize,
}

#[derive(Clone, Debug)]
pub struct Churn {
    pub path: String,
    pub commits: usize,
}

#[derive(Hash, Eq, PartialEq)]
struct Signature {
    name: String,
    email: String,
}

// Every commit contributes to the aggregate metadata. Changed paths
// contribute only within the configured newest-commit churn window.
pub fn load(root: &Path, churn_limit: usize) -> Result<History> {
    let repo = gix::open(root).context("open repository")?;
    let mut commits = 0;
    let mut authors = HashMap::<Signature, usize>::new();
    let mut churn = HashMap::<String, usize>::new();
    let mut churn_commits = 0;
    let mut first_commit = None;
    let mut last_commit = None;

    crate::repo::history::walk(&repo, |commit| {
        commits += 1;
        last_commit.get_or_insert(commit.time);
        first_commit = Some(commit.time);
        *authors
            .entry(Signature {
                name: commit.author_name,
                email: commit.author_email,
            })
            .or_insert(0) += 1;
        if churn_limit == 0 || churn_commits < churn_limit {
            churn_commits += 1;
            for path in commit.changed {
                *churn.entry(path.to_str_lossy().into_owned()).or_insert(0) += 1;
            }
        }
        ControlFlow::Continue(())
    })?;

    Ok(History {
        commits,
        authors: authors_vec(authors, commits),
        churn: churn_vec(churn),
        first_commit,
        last_commit,
    })
}

pub fn time_row(key: &str, epoch: Option<u64>) -> Row {
    let mut row = Row::new(key, relative_time(epoch));
    if let Some(epoch) = epoch {
        row.metrics
            .push(RowMetric::new("timestamp", epoch.to_string()));
    }
    row
}

pub fn relative_time(epoch: Option<u64>) -> String {
    let Some(epoch) = epoch else {
        return String::new();
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    let seconds = now.saturating_sub(epoch);
    let (amount, unit) = match seconds {
        0..=59 => return "just now".to_string(),
        60..=3599 => (seconds / 60, "minute"),
        3600..=86399 => (seconds / 3600, "hour"),
        86400..=604799 => (seconds / 86400, "day"),
        604800..=2629745 => (seconds / 604800, "week"),
        2629746..=31556951 => (seconds / 2629746, "month"),
        _ => (seconds / 31556952, "year"),
    };
    plural(amount, unit)
}

fn authors_vec(authors: HashMap<Signature, usize>, total: usize) -> Vec<Author> {
    let mut out = authors
        .into_iter()
        .map(|(signature, commits)| Author {
            name: signature.name,
            commits,
            contribution: percent(commits, total),
        })
        .collect::<Vec<_>>();
    out.sort_by(|a, b| b.commits.cmp(&a.commits).then_with(|| a.name.cmp(&b.name)));
    out
}

fn churn_vec(churn: HashMap<String, usize>) -> Vec<Churn> {
    let mut out = churn
        .into_iter()
        .map(|(path, commits)| Churn { path, commits })
        .collect::<Vec<_>>();
    out.sort_by(|a, b| b.commits.cmp(&a.commits).then_with(|| a.path.cmp(&b.path)));
    out
}

fn percent(value: usize, total: usize) -> usize {
    if total == 0 {
        return 0;
    }
    ((value as f64 / total as f64) * 100.0).round() as usize
}

fn plural(amount: u64, unit: &str) -> String {
    if amount == 1 {
        format!("1 {unit} ago")
    } else {
        format!("{amount} {unit}s ago")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gix::objs::tree::EntryKind;
    use std::fs;

    fn init_repo(dir: &Path) -> gix::Repository {
        gix::init(dir).expect("init repository");
        let config = dir.join(".git/config");
        let mut text = fs::read_to_string(&config).expect("read repository config");
        text.push_str("[user]\n\tname = Test\n\temail = test@example.com\n");
        fs::write(&config, text).unwrap();
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
        author: &str,
        seconds: u64,
        message: &str,
    ) -> gix::ObjectId {
        let tree = write_tree(repo, files);
        let time = format!("{seconds} +0000");
        let sig = gix::actor::SignatureRef {
            name: author.into(),
            email: "dev@example.com".into(),
            time: &time,
        };
        repo.commit_as(sig, sig, "HEAD", message, tree, parents.iter().copied())
            .expect("commit")
            .detach()
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let base = std::env::temp_dir().join(format!("ghrm-history-stat-{name}"));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        base
    }

    #[test]
    fn load_accumulates_commits_authors_churn_and_bounds() {
        let dir = temp_dir("accumulate");
        let repo = init_repo(&dir);
        let c1 = commit(
            &repo,
            &[("src/lib.rs", b"a\n")],
            &[],
            "Ada",
            1000,
            "add lib",
        );
        commit(
            &repo,
            &[("src/lib.rs", b"b\n"), ("README.md", b"# x\n")],
            &[c1],
            "Ada",
            2000,
            "edit lib, add readme",
        );

        let history = load(&dir, 30).expect("history");

        assert_eq!(history.commits, 2);
        assert_eq!(history.authors.len(), 1);
        assert_eq!(history.authors[0].name, "Ada");
        assert_eq!(history.authors[0].commits, 2);
        assert_eq!(history.first_commit, Some(1000));
        assert_eq!(history.last_commit, Some(2000));
        // Both commits affect src/lib.rs; only the second affects README.md.
        assert_eq!(history.churn[0].path, "src/lib.rs");
        assert_eq!(history.churn[0].commits, 2);
        let readme = history
            .churn
            .iter()
            .find(|c| c.path == "README.md")
            .expect("readme churn");
        assert_eq!(readme.commits, 1);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_limits_churn_window_without_limiting_commits() {
        let dir = temp_dir("window");
        let repo = init_repo(&dir);
        let c1 = commit(
            &repo,
            &[("src/lib.rs", b"a\n")],
            &[],
            "Ada",
            1000,
            "add lib",
        );
        commit(
            &repo,
            &[("src/lib.rs", b"a\n"), ("README.md", b"# x\n")],
            &[c1],
            "Ada",
            2000,
            "add readme",
        );

        // churn window of one only counts the newest commit, which adds
        // only README.md relative to its parent
        let history = load(&dir, 1).expect("history");

        assert_eq!(history.commits, 2);
        assert_eq!(history.churn.len(), 1);
        assert_eq!(history.churn[0].path, "README.md");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_is_empty_for_an_unborn_head() {
        let dir = temp_dir("empty");
        init_repo(&dir);

        let history = load(&dir, 30).expect("history");

        assert_eq!(history.commits, 0);
        assert!(history.authors.is_empty());
        assert!(history.churn.is_empty());
        assert_eq!(history.first_commit, None);

        fs::remove_dir_all(&dir).ok();
    }
}
