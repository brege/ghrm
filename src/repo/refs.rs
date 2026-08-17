use super::diff::DiffTarget;

use gix::bstr::ByteSlice;
use std::io::Read;
use std::path::Path;
use std::process::Stdio;

const MAX_OUTPUT_BYTES: usize = 256 * 1024;
const MAX_REFS: usize = 100;

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

// Branches, tags, and the HEAD commit time come from the reference
// store, bounded to MAX_REFS per class; peel_to_commit resolves both
// lightweight and annotated tags to their commit timestamp. Recent
// commits come from git log. A reference-store read failure fails the
// listing; for commits a broken pipe fails while a failed spawn or
// nonzero status yields an empty list.
pub(crate) fn refs_for(root: &Path, rel: &str) -> Option<RefList> {
    let repo = gix::open(root).ok()?;
    let branches = collect_refs(&repo, RefKind::Branches)?;
    let tags = collect_refs(&repo, RefKind::Tags)?;
    let head_timestamp = repo
        .head_commit()
        .ok()
        .and_then(|commit| commit_time(&commit));
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
    Some(RefList {
        branches,
        tags,
        commits,
        head_timestamp,
    })
}

enum RefKind {
    Branches,
    Tags,
}

impl RefKind {
    fn prefix(&self) -> &'static str {
        match self {
            RefKind::Branches => "refs/heads/",
            RefKind::Tags => "refs/tags/",
        }
    }
}

// Enumerate one ref class, bounded to MAX_REFS. A reference whose full
// name is not a submittable revision or lacks a short label is dropped so
// every emitted value round-trips through the compare entry point. A
// reference-store read error propagates as None rather than a short list.
fn collect_refs(repo: &gix::Repository, kind: RefKind) -> Option<Vec<RefEntry>> {
    let prefix = kind.prefix();
    let platform = repo.references().ok()?;
    let iter = match kind {
        RefKind::Branches => platform.local_branches(),
        RefKind::Tags => platform.tags(),
    }
    .ok()?;
    let mut refs = Vec::new();
    for reference in iter.take(MAX_REFS) {
        if let Some(entry) = ref_entry(reference.ok()?, prefix) {
            refs.push(entry);
        }
    }
    Some(refs)
}

fn ref_entry(mut reference: gix::Reference<'_>, prefix: &str) -> Option<RefEntry> {
    let full = reference.name().as_bstr().to_str().ok()?.to_string();
    let label = full.strip_prefix(prefix)?.to_string();
    if label.is_empty() {
        return None;
    }
    let DiffTarget::Rev(value) = DiffTarget::parse(&full)? else {
        return None;
    };
    let timestamp = reference
        .peel_to_commit()
        .ok()
        .and_then(|commit| commit_time(&commit));
    Some(RefEntry {
        value,
        label,
        timestamp,
    })
}

fn commit_time(commit: &gix::Commit<'_>) -> Option<u64> {
    u64::try_from(commit.time().ok()?.seconds).ok()
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
        // the parser only sees complete lines.
        match raw.iter().rposition(|byte| *byte == b'\n') {
            Some(idx) => raw.truncate(idx + 1),
            None => raw.clear(),
        }
    }
    GitOut::Bytes(raw)
}

fn parse_commit_output(raw: &[u8]) -> Option<Vec<CommitEntry>> {
    parse_commit_lines(&String::from_utf8_lossy(raw))
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

    use crate::testutil::TempDir;
    use gix::refs::transaction::PreviousValue;
    use std::fs;

    fn init_repo(dir: &Path) -> gix::Repository {
        gix::init(dir).expect("init repository");
        let config = dir.join(".git/config");
        let mut text = fs::read_to_string(&config).expect("read repository config");
        text.push_str("[user]\n\tname = Test\n\temail = test@example.com\n");
        fs::write(&config, text).unwrap();
        gix::open(dir).expect("open repository")
    }

    fn empty_commit(
        repo: &gix::Repository,
        parents: &[gix::ObjectId],
        message: &str,
    ) -> gix::ObjectId {
        let tree = gix::ObjectId::empty_tree(repo.object_hash());
        repo.commit("HEAD", message, tree, parents.iter().copied())
            .expect("commit")
            .detach()
    }

    #[test]
    fn collects_branches_with_full_ref_values() {
        let td = TempDir::new("ghrm-refs-branches");
        let repo = init_repo(td.path());
        let c1 = empty_commit(&repo, &[], "c1");
        empty_commit(&repo, &[c1], "c2");
        repo.reference(
            "refs/heads/feature",
            c1,
            PreviousValue::MustNotExist,
            "create",
        )
        .expect("create branch");

        let branches = collect_refs(&repo, RefKind::Branches).expect("branch listing");
        let feature = branches
            .iter()
            .find(|entry| entry.label == "feature")
            .expect("feature branch present");

        assert_eq!(feature.value, "refs/heads/feature");
        assert!(feature.timestamp.is_some());
    }

    #[test]
    fn collects_tags_with_full_ref_values_and_short_labels() {
        let td = TempDir::new("ghrm-refs-tags");
        let repo = init_repo(td.path());
        let c1 = empty_commit(&repo, &[], "c1");
        repo.tag_reference("v1", c1, PreviousValue::MustNotExist)
            .expect("create tag");

        let tags = collect_refs(&repo, RefKind::Tags).expect("tag listing");

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].value, "refs/tags/v1");
        assert_eq!(tags[0].label, "v1");
        assert!(tags[0].timestamp.is_some());
    }

    #[test]
    fn collects_annotated_tag_commit_timestamp() {
        let td = TempDir::new("ghrm-refs-annotated");
        let repo = init_repo(td.path());
        let c1 = empty_commit(&repo, &[], "c1");
        let commit_seconds =
            commit_time(&repo.find_commit(c1).expect("find commit")).expect("commit time");
        let tagger = gix::actor::SignatureRef {
            name: "Tagger".into(),
            email: "tag@example.com".into(),
            // A tagger timestamp far from the commit's, so a returned commit
            // time proves peel_to_commit followed the tag object to the commit.
            time: "1000000000 +0000",
        };
        repo.tag(
            "release",
            c1,
            gix::objs::Kind::Commit,
            Some(tagger),
            "annotated",
            PreviousValue::MustNotExist,
        )
        .expect("create annotated tag");

        let tags = collect_refs(&repo, RefKind::Tags).expect("tag listing");
        let release = tags
            .iter()
            .find(|entry| entry.label == "release")
            .expect("annotated tag present");

        assert_eq!(release.value, "refs/tags/release");
        assert_eq!(release.timestamp, Some(commit_seconds));
        assert_ne!(release.timestamp, Some(1_000_000_000));
    }

    #[test]
    fn bounds_ref_enumeration_to_the_limit() {
        let td = TempDir::new("ghrm-refs-limit");
        let repo = init_repo(td.path());
        let c1 = empty_commit(&repo, &[], "c1");
        for index in 0..(MAX_REFS + 20) {
            repo.tag_reference(format!("t{index:04}"), c1, PreviousValue::MustNotExist)
                .expect("create tag");
        }

        let tags = collect_refs(&repo, RefKind::Tags).expect("tag listing");

        assert_eq!(tags.len(), MAX_REFS);
    }

    #[test]
    fn head_timestamp_reads_the_head_commit() {
        let td = TempDir::new("ghrm-refs-head");
        let repo = init_repo(td.path());
        empty_commit(&repo, &[], "c1");

        let head_timestamp = repo
            .head_commit()
            .ok()
            .and_then(|commit| commit_time(&commit));

        assert!(head_timestamp.is_some());
    }

    #[test]
    fn empty_repository_yields_no_branches_or_tags() {
        let td = TempDir::new("ghrm-refs-empty");
        let repo = init_repo(td.path());

        assert!(collect_refs(&repo, RefKind::Branches).unwrap().is_empty());
        assert!(collect_refs(&repo, RefKind::Tags).unwrap().is_empty());
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
