use super::diff::DiffTarget;

use std::io::Read;
use std::path::Path;
use std::process::Stdio;

const MAX_OUTPUT_BYTES: usize = 256 * 1024;
// Annotated tags use the peeled commit date; branches and lightweight tags
// use the direct commit date.
const REF_FORMAT: &str = "--format=%(refname)%1f%(refname:short)%1f%(if)%(*committerdate)%(then)%(*committerdate:unix)%(else)%(committerdate:unix)%(end)";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct RefList {
    pub(crate) branches: Vec<RefEntry>,
    pub(crate) tags: Vec<RefEntry>,
    pub(crate) commits: Vec<CommitEntry>,
    pub(crate) head_timestamp: Option<u64>,
}

// value is the unambiguous revision a picker submits (a full ref name or a
// full object id); label is the short display name, which can collide
// across branches and tags.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RefEntry {
    pub(crate) value: String,
    pub(crate) label: String,
    pub(crate) timestamp: Option<u64>,
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
    Bytes(Vec<u8>),
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
        &["for-each-ref", "--count=100", REF_FORMAT, "refs/heads"],
    ) {
        GitOut::Unavailable => Vec::new(),
        GitOut::Broken => return None,
        GitOut::Bytes(raw) => parse_ref_output(&raw, "refs/heads/")?,
    };
    let tags = match run_git(
        root,
        &["for-each-ref", "--count=100", REF_FORMAT, "refs/tags"],
    ) {
        GitOut::Unavailable => Vec::new(),
        GitOut::Broken => return None,
        GitOut::Bytes(raw) => parse_ref_output(&raw, "refs/tags/")?,
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
        GitOut::Bytes(raw) => parse_commit_output(&raw)?,
    };
    let head_timestamp = match run_git(root, &["show", "--no-patch", "--format=%ct", "HEAD"]) {
        GitOut::Unavailable => None,
        GitOut::Broken => return None,
        GitOut::Bytes(raw) => Some(parse_timestamp_output(&raw)?),
    };
    Some(RefList {
        branches,
        tags,
        commits,
        head_timestamp,
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
    if capped {
        // A capped read can end mid-record; the torn tail is dropped so
        // the parsers only see complete lines.
        match raw.iter().rposition(|byte| *byte == b'\n') {
            Some(idx) => raw.truncate(idx + 1),
            None => raw.clear(),
        }
    }
    GitOut::Bytes(raw)
}

fn parse_ref_output(raw: &[u8], prefix: &str) -> Option<Vec<RefEntry>> {
    parse_ref_lines(std::str::from_utf8(raw).ok()?, prefix)
}

fn parse_commit_output(raw: &[u8]) -> Option<Vec<CommitEntry>> {
    parse_commit_lines(&String::from_utf8_lossy(raw))
}

fn parse_timestamp_output(raw: &[u8]) -> Option<u64> {
    let text = std::str::from_utf8(raw).ok()?;
    let mut lines = text.lines();
    let timestamp = lines.next()?.parse().ok()?;
    (lines.next().is_none()).then_some(timestamp)
}

fn parse_ref_lines(text: &str, prefix: &str) -> Option<Vec<RefEntry>> {
    text.lines()
        .map(|line| {
            let (full, rest) = line.split_once('\x1f')?;
            let (short, timestamp) = rest.split_once('\x1f')?;
            if !full.starts_with(prefix) || short.is_empty() {
                return None;
            }
            let DiffTarget::Rev(value) = DiffTarget::parse(full)? else {
                return None;
            };
            let timestamp = if timestamp.is_empty() {
                None
            } else {
                Some(timestamp.parse().ok()?)
            };
            Some(RefEntry {
                value,
                label: short.to_string(),
                timestamp,
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
            let DiffTarget::Rev(value) = DiffTarget::parse(value)? else {
                return None;
            };
            Some(CommitEntry {
                value,
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
        let text = "refs/heads/main\x1fmain\x1f1723600000\nrefs/heads/feature/x\x1ffeature/x\x1f\n";

        let branches = parse_ref_lines(text, "refs/heads/").unwrap();

        assert_eq!(
            branches,
            vec![
                RefEntry {
                    value: "refs/heads/main".to_string(),
                    label: "main".to_string(),
                    timestamp: Some(1723600000),
                },
                RefEntry {
                    value: "refs/heads/feature/x".to_string(),
                    label: "feature/x".to_string(),
                    timestamp: None,
                },
            ]
        );
    }

    #[test]
    fn parse_ref_lines_rejects_malformed_output() {
        assert!(parse_ref_lines("no separator line\n", "refs/heads/").is_none());
        assert!(parse_ref_lines("refs/heads/empty\x1f\x1f1\n", "refs/heads/").is_none());
        assert!(
            parse_ref_lines("refs/remotes/origin/main\x1fmain\x1f1\n", "refs/heads/").is_none()
        );
        assert!(parse_ref_lines("refs/heads/main\x1fmain\x1fbad\n", "refs/heads/").is_none());

        let long = format!("refs/heads/{}\x1flong\x1f1\n", "a".repeat(256));
        assert!(parse_ref_lines(&long, "refs/heads/").is_none());
    }

    #[test]
    fn parse_ref_output_rejects_non_utf8_identity() {
        assert!(parse_ref_output(b"refs/heads/bad\xff\x1fbad\x1f1\n", "refs/heads/").is_none());
    }

    #[test]
    fn parse_timestamp_output_accepts_one_unix_timestamp() {
        assert_eq!(parse_timestamp_output(b"1723600000\n"), Some(1723600000));
        assert!(parse_timestamp_output(b"not-a-time\n").is_none());
        assert!(parse_timestamp_output(b"1\n2\n").is_none());
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

        let long = format!("{}\x1fd888e48\x1f1723600000\x1fsubject\n", "a".repeat(257));
        assert!(parse_commit_lines(&long).is_none());
    }

    #[test]
    fn parse_commit_output_replaces_non_utf8_subject_bytes() {
        let commits =
            parse_commit_output(b"d888e48aaaa\x1fd888e48\x1f1723600000\x1fsubject \xff\n").unwrap();

        assert_eq!(commits[0].value, "d888e48aaaa");
        assert_eq!(commits[0].subject, "subject \u{fffd}");
    }
}
