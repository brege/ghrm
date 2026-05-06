use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use super::git;

#[derive(Clone, Debug, Default)]
pub struct History {
    pub commits: usize,
    pub authors: Vec<Author>,
    pub churn: Vec<Churn>,
    pub churn_limit: usize,
    pub first_commit: Option<u64>,
    pub last_commit: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct Author {
    pub name: String,
    pub email: String,
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

pub fn load(root: &Path, churn_limit: usize) -> Result<History> {
    let output = git::output(
        root,
        &["log", "--format=%x1e%H%x1f%an%x1f%ae%x1f%ct", "--name-only"],
    )?;
    Ok(parse(&output, churn_limit))
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

fn parse(output: &str, churn_limit: usize) -> History {
    let mut authors = HashMap::<Signature, usize>::new();
    let mut churn = HashMap::<String, usize>::new();
    let mut current_paths = HashSet::<String>::new();
    let mut commits = 0;
    let mut churn_commits = 0;
    let mut count_churn = false;
    let mut first_commit = None;
    let mut last_commit = None;

    for line in output.lines() {
        if let Some(header) = line.strip_prefix('\x1e') {
            current_paths.clear();
            count_churn = churn_limit == 0 || churn_commits < churn_limit;
            if count_churn {
                churn_commits += 1;
            }
            let mut fields = header.split('\x1f');
            let _hash = fields.next();
            let name = fields.next().unwrap_or_default().to_string();
            let email = fields.next().unwrap_or_default().to_string();
            let timestamp = fields.next().and_then(|value| value.parse::<u64>().ok());

            commits += 1;
            if let Some(timestamp) = timestamp {
                last_commit.get_or_insert(timestamp);
                first_commit = Some(timestamp);
            }
            *authors.entry(Signature { name, email }).or_insert(0) += 1;
            continue;
        }

        if !count_churn || line.is_empty() || !current_paths.insert(line.to_string()) {
            continue;
        }
        *churn.entry(line.to_string()).or_insert(0) += 1;
    }

    History {
        commits,
        authors: authors_vec(authors, commits),
        churn: churn_vec(churn),
        churn_limit: if churn_limit == 0 {
            churn_commits
        } else {
            churn_limit.min(commits)
        },
        first_commit,
        last_commit,
    }
}

fn authors_vec(authors: HashMap<Signature, usize>, total: usize) -> Vec<Author> {
    let mut out = authors
        .into_iter()
        .map(|(signature, commits)| Author {
            name: signature.name,
            email: signature.email,
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

    #[test]
    fn parses_authors_and_churn() {
        let history = parse(
            "\x1eabc\x1fA\x1fa@example.com\x1f10\nsrc/lib.rs\n\n\x1edef\x1fA\x1fa@example.com\x1f20\nsrc/lib.rs\nREADME.md\n",
            30,
        );

        assert_eq!(history.commits, 2);
        assert_eq!(history.authors[0].commits, 2);
        assert_eq!(history.churn[0].path, "src/lib.rs");
        assert_eq!(history.first_commit, Some(20));
        assert_eq!(history.last_commit, Some(10));
    }

    #[test]
    fn limits_churn_window_without_limiting_commits() {
        let history = parse(
            "\x1eabc\x1fA\x1fa@example.com\x1f10\nsrc/lib.rs\n\n\x1edef\x1fA\x1fa@example.com\x1f20\nREADME.md\n",
            1,
        );

        assert_eq!(history.commits, 2);
        assert_eq!(history.churn_limit, 1);
        assert_eq!(history.churn.len(), 1);
        assert_eq!(history.churn[0].path, "src/lib.rs");
    }
}
