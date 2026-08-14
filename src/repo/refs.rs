use std::io::Read;
use std::path::Path;
use std::process::Stdio;

const MAX_OUTPUT_BYTES: usize = 256 * 1024;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct RefList {
    pub(crate) branches: Vec<RefEntry>,
    pub(crate) tags: Vec<RefEntry>,
    pub(crate) commits: Vec<CommitEntry>,
}

// value is the unambiguous revision a picker submits (a full ref name or a
// full object id); label is the short display name, which can collide
// across branches and tags.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RefEntry {
    pub(crate) value: String,
    pub(crate) label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CommitEntry {
    pub(crate) value: String,
    pub(crate) label: String,
    pub(crate) timestamp: u64,
    pub(crate) subject: String,
}

enum GitOut {
    Unavailable,
    Broken,
    Text(String),
}

// Entry counts are bounded at the git command (--count and -n) and bytes
// are bounded at the read. for-each-ref hex escapes are %xx while log
// escapes are %x..; both emit the 0x1f unit separator the parsers split
// on. Only a failed spawn or a nonzero git status degrades to an empty
// list; a broken pipe, a read failure, a wait failure, or malformed
// successful output fails the whole listing.
pub(crate) fn refs_for(root: &Path, rel: &str) -> Option<RefList> {
    let branches = match run_git(
        root,
        &[
            "for-each-ref",
            "--count=100",
            "--format=%(refname)%1f%(refname:short)",
            "refs/heads",
        ],
    ) {
        GitOut::Unavailable => Vec::new(),
        GitOut::Broken => return None,
        GitOut::Text(text) => parse_ref_lines(&text, "refs/heads/")?,
    };
    let tags = match run_git(
        root,
        &[
            "for-each-ref",
            "--count=100",
            "--format=%(refname)%1f%(refname:short)",
            "refs/tags",
        ],
    ) {
        GitOut::Unavailable => Vec::new(),
        GitOut::Broken => return None,
        GitOut::Text(text) => parse_ref_lines(&text, "refs/tags/")?,
    };
    let commits = match run_git(
        root,
        &[
            "log",
            "-n",
            "30",
            "--format=%H%x1f%h%x1f%ct%x1f%s",
            "--",
            rel,
        ],
    ) {
        GitOut::Unavailable => Vec::new(),
        GitOut::Broken => return None,
        GitOut::Text(text) => parse_commit_lines(&text)?,
    };
    Some(RefList {
        branches,
        tags,
        commits,
    })
}

fn run_git(root: &Path, args: &[&str]) -> GitOut {
    let mut cmd = super::git_command(root);
    cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::null());
    let Ok(mut child) = cmd.spawn() else {
        return GitOut::Unavailable;
    };
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return GitOut::Broken;
    };
    let mut raw = Vec::new();
    let read_ok = stdout
        .take(MAX_OUTPUT_BYTES as u64)
        .read_to_end(&mut raw)
        .is_ok();
    let capped = raw.len() >= MAX_OUTPUT_BYTES;
    if capped {
        let _ = child.kill();
    }
    let Ok(status) = child.wait() else {
        return GitOut::Broken;
    };
    if !read_ok {
        return GitOut::Broken;
    }
    if !status.success() && !capped {
        return GitOut::Unavailable;
    }
    let mut text = String::from_utf8_lossy(&raw).into_owned();
    if capped {
        // A capped read can end mid-record; the torn tail is dropped so
        // the parsers only see complete lines.
        match text.rfind('\n') {
            Some(idx) => text.truncate(idx + 1),
            None => text.clear(),
        }
    }
    GitOut::Text(text)
}

fn parse_ref_lines(text: &str, prefix: &str) -> Option<Vec<RefEntry>> {
    text.lines()
        .map(|line| {
            let (full, short) = line.split_once('\x1f')?;
            if !full.starts_with(prefix) || short.is_empty() {
                return None;
            }
            Some(RefEntry {
                value: full.to_string(),
                label: short.to_string(),
            })
        })
        .collect()
}

fn parse_commit_lines(text: &str) -> Option<Vec<CommitEntry>> {
    text.lines()
        .map(|line| {
            let (value, rest) = line.split_once('\x1f')?;
            let (label, rest) = rest.split_once('\x1f')?;
            let (timestamp, subject) = rest.split_once('\x1f')?;
            let hex = |raw: &str| !raw.is_empty() && raw.chars().all(|c| c.is_ascii_hexdigit());
            if !hex(value) || !hex(label) {
                return None;
            }
            Some(CommitEntry {
                value: value.to_string(),
                label: label.to_string(),
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
    fn parse_ref_lines_keeps_full_value_and_short_label() {
        let text = "refs/heads/main\x1fmain\nrefs/heads/feature/x\x1ffeature/x\n";

        let branches = parse_ref_lines(text, "refs/heads/").unwrap();

        assert_eq!(
            branches,
            vec![
                RefEntry {
                    value: "refs/heads/main".to_string(),
                    label: "main".to_string(),
                },
                RefEntry {
                    value: "refs/heads/feature/x".to_string(),
                    label: "feature/x".to_string(),
                },
            ]
        );
    }

    #[test]
    fn parse_ref_lines_rejects_malformed_output() {
        assert!(parse_ref_lines("no separator line\n", "refs/heads/").is_none());
        assert!(parse_ref_lines("refs/heads/empty\x1f\n", "refs/heads/").is_none());
        assert!(parse_ref_lines("refs/remotes/origin/main\x1fmain\n", "refs/heads/").is_none());
    }

    #[test]
    fn parse_commit_lines_keeps_full_and_short_ids_in_order() {
        let text = "d888e48aaaa\x1fd888e48\x1f1723600000\x1fchore: upgrade benchmark tooling\n\
                    1ae8e74bbbb\x1f1ae8e74\x1f1723500000\x1ffeat: save icon\x1fstray separator\n";

        let commits = parse_commit_lines(text).unwrap();

        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].value, "d888e48aaaa");
        assert_eq!(commits[0].label, "d888e48");
        assert_eq!(commits[0].timestamp, 1723600000);
        assert_eq!(commits[0].subject, "chore: upgrade benchmark tooling");
        assert_eq!(commits[1].subject, "feat: save icon\x1fstray separator");
    }

    #[test]
    fn parse_commit_lines_rejects_malformed_output() {
        assert!(parse_commit_lines("junk without separators\n").is_none());
        assert!(parse_commit_lines("d888e48\x1fd888e48\x1fnot-a-number\x1fsubject\n").is_none());
        assert!(parse_commit_lines("nothex!\x1fd888e48\x1f1723600000\x1fsubject\n").is_none());
        assert!(parse_commit_lines("\x1fd888e48\x1f1723600000\x1fsubject\n").is_none());
    }
}
