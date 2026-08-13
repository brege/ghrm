#![allow(dead_code)]

use std::path::Path;
use std::process::{Command, Stdio};

const REF_LIMIT: usize = 100;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RefList {
    pub(crate) branches: Vec<String>,
    pub(crate) tags: Vec<String>,
    pub(crate) commits: Vec<CommitEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CommitEntry {
    pub(crate) hash: String,
    pub(crate) timestamp: u64,
    pub(crate) subject: String,
}

pub(crate) fn refs_for(root: &Path, rel: &str) -> RefList {
    // for-each-ref hex escapes are %xx while log escapes are %x..; both
    // emit the 0x1f unit separator the parsers below split on.
    let refs_out = run_git(
        root,
        &[
            "for-each-ref",
            "refs/heads",
            "refs/tags",
            "--format=%(refname:short)%1f%(refname)",
        ],
    );
    let log_out = run_git(
        root,
        &["log", "-n", "30", "--format=%h%x1f%ct%x1f%s", "--", rel],
    );
    let (branches, tags) = parse_ref_lines(&refs_out);
    RefList {
        branches,
        tags,
        commits: parse_commit_lines(&log_out),
    }
}

fn run_git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("--no-pager")
        .arg("-C")
        .arg(root)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();
    match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).into_owned(),
        _ => String::new(),
    }
}

fn parse_ref_lines(text: &str) -> (Vec<String>, Vec<String>) {
    let mut branches = Vec::new();
    let mut tags = Vec::new();
    for line in text.lines() {
        let Some((short, full)) = line.split_once('\x1f') else {
            continue;
        };
        if short.is_empty() {
            continue;
        }
        if full.starts_with("refs/heads/") && branches.len() < REF_LIMIT {
            branches.push(short.to_string());
        } else if full.starts_with("refs/tags/") && tags.len() < REF_LIMIT {
            tags.push(short.to_string());
        }
    }
    (branches, tags)
}

fn parse_commit_lines(text: &str) -> Vec<CommitEntry> {
    text.lines()
        .filter_map(|line| {
            let (hash, rest) = line.split_once('\x1f')?;
            let (timestamp, subject) = rest.split_once('\x1f')?;
            if hash.is_empty() {
                return None;
            }
            Some(CommitEntry {
                hash: hash.to_string(),
                timestamp: timestamp.parse().ok()?,
                subject: subject.to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ref_lines_classifies_by_full_ref() {
        let text = "main\x1frefs/heads/main\n\
                    feature/x\x1frefs/heads/feature/x\n\
                    v0.5.3\x1frefs/tags/v0.5.3\n\
                    noise without separator\n\
                    origin/main\x1frefs/remotes/origin/main\n\
                    \x1frefs/heads/empty\n";

        let (branches, tags) = parse_ref_lines(text);

        assert_eq!(branches, vec!["main", "feature/x"]);
        assert_eq!(tags, vec!["v0.5.3"]);
    }

    #[test]
    fn parse_ref_lines_caps_each_kind() {
        let text = (0..110)
            .map(|idx| format!("b{idx}\x1frefs/heads/b{idx}\n"))
            .collect::<String>();

        let (branches, tags) = parse_ref_lines(&text);

        assert_eq!(branches.len(), REF_LIMIT);
        assert!(tags.is_empty());
    }

    #[test]
    fn parse_commit_lines_keeps_order_and_skips_malformed() {
        let text = "d888e48\x1f1723600000\x1fchore: upgrade benchmark Python tooling\n\
                    line without separators\n\
                    abc1234\x1fnot-a-number\x1fsubject\n\
                    1ae8e74\x1f1723500000\x1ffeat: add save icon\x1fwith stray separator\n";

        let commits = parse_commit_lines(text);

        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].hash, "d888e48");
        assert_eq!(commits[0].timestamp, 1723600000);
        assert_eq!(
            commits[0].subject,
            "chore: upgrade benchmark Python tooling"
        );
        assert_eq!(commits[1].hash, "1ae8e74");
        assert_eq!(
            commits[1].subject,
            "feat: add save icon\x1fwith stray separator"
        );
    }
}
