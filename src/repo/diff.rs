#![allow(dead_code)]

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};

pub(crate) const WORKTREE: &str = ":worktree";
pub(crate) const INDEX: &str = ":index";

const MAX_PATCH_BYTES: usize = 1024 * 1024;
const MAX_STDERR_BYTES: u64 = 4096;
const MAX_TARGET_LEN: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DiffTarget {
    Worktree,
    Index,
    Rev(String),
}

impl DiffTarget {
    // A colon never appears in a valid ref name, so the pseudo-ref tokens
    // cannot collide with a real ref, and rejecting remaining colons also
    // rejects git's rev:path blob syntax. A leading hyphen would read as a
    // git option, and a leading dot covers the three-dot range syntax that
    // DiffSpec::parse would otherwise split into a dotted ref.
    pub(crate) fn parse(raw: &str) -> Option<Self> {
        match raw {
            WORKTREE => return Some(Self::Worktree),
            INDEX => return Some(Self::Index),
            _ => {}
        }
        let valid = !raw.is_empty()
            && raw.len() <= MAX_TARGET_LEN
            && !raw.starts_with('-')
            && !raw.starts_with('.')
            && !raw.contains("..")
            && !raw.contains(':')
            && !raw.chars().any(|c| c.is_whitespace() || c.is_control());
        valid.then(|| Self::Rev(raw.to_string()))
    }

    pub(crate) fn token(&self) -> &str {
        match self {
            Self::Worktree => WORKTREE,
            Self::Index => INDEX,
            Self::Rev(rev) => rev,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DiffSpec {
    pub(crate) base: DiffTarget,
    pub(crate) head: DiffTarget,
}

impl DiffSpec {
    pub(crate) fn parse(raw: &str) -> Option<Self> {
        let (base, head) = raw.split_once("..")?;
        Some(Self {
            base: DiffTarget::parse(base)?,
            head: DiffTarget::parse(head)?,
        })
    }

    pub(crate) fn token(&self) -> String {
        format!("{}..{}", self.base.token(), self.head.token())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DiffOutcome {
    Patch(String),
    Clean,
    Failed(String),
}

// git diff has three fixed comparison forms: no selector is index vs
// worktree, "--cached <rev>" is rev vs index, and "<rev>" alone is rev vs
// worktree. -R swaps sides for the pairings that reverse a fixed form, and
// equal sides never spawn because their diff is empty by definition.
fn selector_args(spec: &DiffSpec) -> Option<Vec<String>> {
    use DiffTarget::{Index, Rev, Worktree};
    let owned = |args: &[&str]| Some(args.iter().map(|arg| arg.to_string()).collect());
    match (&spec.base, &spec.head) {
        (Worktree, Worktree) | (Index, Index) => None,
        (Rev(a), Rev(b)) if a == b => None,
        (Rev(a), Rev(b)) => owned(&[a, b]),
        (Rev(a), Worktree) => owned(&[a]),
        (Rev(a), Index) => owned(&["--cached", a]),
        (Index, Worktree) => owned(&[]),
        (Worktree, Rev(a)) => owned(&["-R", a]),
        (Index, Rev(a)) => owned(&["--cached", "-R", a]),
        (Worktree, Index) => owned(&["-R"]),
    }
}

pub(crate) fn unified_diff(root: &Path, spec: &DiffSpec, rel: &str) -> DiffOutcome {
    let Some(selector) = selector_args(spec) else {
        return DiffOutcome::Clean;
    };

    let mut cmd = Command::new("git");
    cmd.arg("--no-pager")
        .arg("-C")
        .arg(root)
        .arg("-c")
        .arg("core.quotepath=false")
        .arg("diff")
        .arg("--no-color")
        .arg("--no-ext-diff")
        .arg("--no-textconv")
        .args(&selector)
        .arg("--")
        .arg(rel)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let Ok(mut child) = cmd.spawn() else {
        return DiffOutcome::Failed("git is unavailable".to_string());
    };

    let mut raw = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        let _ = stdout
            .take((MAX_PATCH_BYTES + 1) as u64)
            .read_to_end(&mut raw);
    }
    let truncated = raw.len() > MAX_PATCH_BYTES;
    if truncated {
        raw.truncate(MAX_PATCH_BYTES);
        let _ = child.kill();
    }

    let mut stderr_raw = Vec::new();
    if let Some(stderr) = child.stderr.take() {
        let _ = stderr.take(MAX_STDERR_BYTES).read_to_end(&mut stderr_raw);
    }

    let Ok(status) = child.wait() else {
        return DiffOutcome::Failed("git is unavailable".to_string());
    };
    if !truncated && !status.success() {
        return DiffOutcome::Failed(failure_line(&stderr_raw));
    }

    let mut patch = String::from_utf8_lossy(&raw).into_owned();
    if truncated {
        drop_torn_line(&mut patch);
        patch.push_str("\n[patch truncated at 1 MiB]\n");
    }
    if patch.trim().is_empty() {
        return DiffOutcome::Clean;
    }
    DiffOutcome::Patch(patch)
}

// A byte-capped cut can split a line and a UTF-8 sequence; dropping back to
// the last newline removes both.
fn drop_torn_line(patch: &mut String) {
    match patch.rfind('\n') {
        Some(idx) => patch.truncate(idx),
        None => patch.clear(),
    }
}

fn failure_line(stderr: &[u8]) -> String {
    String::from_utf8_lossy(stderr)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.trim_start_matches("fatal:").trim().to_string())
        .unwrap_or_else(|| "git diff failed".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rev(name: &str) -> DiffTarget {
        DiffTarget::Rev(name.to_string())
    }

    fn spec(base: DiffTarget, head: DiffTarget) -> DiffSpec {
        DiffSpec { base, head }
    }

    #[test]
    fn parse_accepts_pseudo_tokens_and_revs() {
        assert_eq!(DiffTarget::parse(":worktree"), Some(DiffTarget::Worktree));
        assert_eq!(DiffTarget::parse(":index"), Some(DiffTarget::Index));
        for raw in [
            "HEAD",
            "main",
            "d888e48",
            "HEAD~2",
            "v0.5.3",
            "feature/x",
            "@",
        ] {
            assert_eq!(DiffTarget::parse(raw), Some(rev(raw)), "{raw}");
        }
    }

    #[test]
    fn parse_rejects_unsafe_targets() {
        let long = "a".repeat(300);
        for raw in [
            "",
            "-R",
            "--cached",
            "a..b",
            ".hidden",
            "HEAD:file",
            ":other",
            "a b",
            "a\tb",
            "a\u{7}b",
            long.as_str(),
        ] {
            assert_eq!(DiffTarget::parse(raw), None, "{raw:?}");
        }
    }

    #[test]
    fn spec_parse_splits_on_first_dot_pair() {
        assert_eq!(
            DiffSpec::parse("HEAD..:worktree"),
            Some(spec(rev("HEAD"), DiffTarget::Worktree))
        );
        assert_eq!(
            DiffSpec::parse(":index..:worktree"),
            Some(spec(DiffTarget::Index, DiffTarget::Worktree))
        );
        for raw in ["HEAD", "a..", "..b", "a...b", "a..b..c"] {
            assert_eq!(DiffSpec::parse(raw), None, "{raw:?}");
        }
    }

    #[test]
    fn spec_token_round_trips() {
        for raw in ["HEAD..:worktree", ":index..:worktree", "main..d888e48"] {
            assert_eq!(DiffSpec::parse(raw).unwrap().token(), raw);
        }
    }

    #[test]
    fn selector_args_map_fixed_forms() {
        let cases = [
            (spec(rev("a"), rev("b")), vec!["a", "b"]),
            (spec(rev("a"), DiffTarget::Worktree), vec!["a"]),
            (spec(rev("a"), DiffTarget::Index), vec!["--cached", "a"]),
            (spec(DiffTarget::Index, DiffTarget::Worktree), vec![]),
            (spec(DiffTarget::Worktree, rev("a")), vec!["-R", "a"]),
            (
                spec(DiffTarget::Index, rev("a")),
                vec!["--cached", "-R", "a"],
            ),
            (spec(DiffTarget::Worktree, DiffTarget::Index), vec!["-R"]),
        ];
        for (spec, expected) in cases {
            assert_eq!(
                selector_args(&spec),
                Some(expected.iter().map(|arg| arg.to_string()).collect()),
                "{spec:?}"
            );
        }
    }

    #[test]
    fn selector_args_skip_equal_sides() {
        for spec in [
            spec(DiffTarget::Worktree, DiffTarget::Worktree),
            spec(DiffTarget::Index, DiffTarget::Index),
            spec(rev("HEAD"), rev("HEAD")),
        ] {
            assert_eq!(selector_args(&spec), None, "{spec:?}");
        }
    }

    #[test]
    fn unified_diff_is_clean_for_equal_sides_without_spawning() {
        let outcome = unified_diff(
            Path::new("/nonexistent"),
            &spec(DiffTarget::Worktree, DiffTarget::Worktree),
            "README.md",
        );
        assert_eq!(outcome, DiffOutcome::Clean);
    }

    #[test]
    fn failure_line_reports_first_meaningful_line() {
        assert_eq!(
            failure_line(b"\nfatal: ambiguous argument 'nope'\nhint: more\n"),
            "ambiguous argument 'nope'"
        );
        assert_eq!(failure_line(b""), "git diff failed");
    }
}
