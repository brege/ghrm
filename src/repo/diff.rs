use std::io::Read;
use std::path::Path;
use std::process::Stdio;

pub(crate) const WORKTREE: &str = ":worktree";
pub(crate) const INDEX: &str = ":index";

const MAX_PATCH_BYTES: usize = 1024 * 1024;
const MAX_STDERR_BYTES: u64 = 4096;
const MAX_TARGET_LEN: usize = 256;

// --no-ext-diff and --no-textconv disable the diff drivers a repository
// config could point at external programs; the shared repo::git_command
// prefix disables fsmonitor for every spawned command, including the
// index-reading ls-files probe.
const DIFF_FLAGS: &[&str] = &["--no-color", "--no-ext-diff", "--no-textconv"];

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

fn diff_flags() -> Vec<String> {
    let mut argv = vec!["diff".to_string()];
    argv.extend(DIFF_FLAGS.iter().map(|flag| flag.to_string()));
    argv
}

fn diff_argv(spec: &DiffSpec, rel: &str) -> Option<Vec<String>> {
    let selector = selector_args(spec)?;
    let mut argv = diff_flags();
    argv.extend(selector);
    argv.push("--".to_string());
    argv.push(rel.to_string());
    Some(argv)
}

// --no-index reads both sides from the filesystem, so /dev/null supplies
// the empty side of an untracked file's patch.
fn untracked_argv(rel: &str, reversed: bool) -> Vec<String> {
    let mut argv = diff_flags();
    argv.push("--no-index".to_string());
    if reversed {
        argv.push("-R".to_string());
    }
    argv.push("--".to_string());
    argv.push("/dev/null".to_string());
    argv.push(rel.to_string());
    argv
}

fn tracked_argv(rel: &str) -> Vec<String> {
    vec![
        "ls-files".to_string(),
        "--error-unmatch".to_string(),
        "--".to_string(),
        rel.to_string(),
    ]
}

// Exactly one worktree side makes an untracked file comparable: the file
// is wholly an addition (or a deletion when the worktree is the base).
fn worktree_direction(spec: &DiffSpec) -> Option<bool> {
    match (&spec.base, &spec.head) {
        (DiffTarget::Worktree, DiffTarget::Worktree) => None,
        (_, DiffTarget::Worktree) => Some(false),
        (DiffTarget::Worktree, _) => Some(true),
        _ => None,
    }
}

pub(crate) fn unified_diff(root: &Path, spec: &DiffSpec, rel: &str) -> DiffOutcome {
    let Some(argv) = diff_argv(spec, rel) else {
        return DiffOutcome::Clean;
    };
    let outcome = spawn_diff(root, &argv, false);
    if !matches!(outcome, DiffOutcome::Clean) {
        return outcome;
    }
    let Some(reversed) = worktree_direction(spec) else {
        return DiffOutcome::Clean;
    };
    match tracked_state(root, rel) {
        TrackedState::Tracked => DiffOutcome::Clean,
        TrackedState::Untracked => spawn_diff(root, &untracked_argv(rel, reversed), true),
        TrackedState::Failed(reason) => DiffOutcome::Failed(reason),
    }
}

enum TrackedState {
    Tracked,
    Untracked,
    Failed(String),
}

// ls-files --error-unmatch exits 0 for a tracked path and 1 for an
// unmatched one; every other exit is a repository failure, not an
// untracked file.
fn tracked_state(root: &Path, rel: &str) -> TrackedState {
    let output = super::git_command(root)
        .args(tracked_argv(rel))
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output();
    let Ok(output) = output else {
        return TrackedState::Failed("git is unavailable".to_string());
    };
    match output.status.code() {
        Some(0) => TrackedState::Tracked,
        Some(1) => TrackedState::Untracked,
        _ => {
            let end = output.stderr.len().min(MAX_STDERR_BYTES as usize);
            TrackedState::Failed(failure_line(&output.stderr[..end]))
        }
    }
}

fn spawn_diff(root: &Path, argv: &[String], allow_exit_one: bool) -> DiffOutcome {
    let mut cmd = super::git_command(root);
    cmd.args(argv).stdout(Stdio::piped()).stderr(Stdio::piped());

    let Ok(mut child) = cmd.spawn() else {
        return DiffOutcome::Failed("git is unavailable".to_string());
    };

    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return DiffOutcome::Failed("git stdout pipe missing".to_string());
    };
    let Some(stderr) = child.stderr.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return DiffOutcome::Failed("git stderr pipe missing".to_string());
    };

    // stderr drains on its own thread so a chatty child can never block on
    // a full stderr pipe while stdout is still streaming.
    let stderr_thread = std::thread::spawn(move || -> std::io::Result<Vec<u8>> {
        let mut limited = stderr.take(MAX_STDERR_BYTES);
        let mut buf = Vec::new();
        limited.read_to_end(&mut buf)?;
        std::io::copy(&mut limited.into_inner(), &mut std::io::sink())?;
        Ok(buf)
    });

    let stdout_read = read_capped(stdout, MAX_PATCH_BYTES);
    if !matches!(stdout_read, Ok((_, false))) {
        let _ = child.kill();
    }

    let stderr_read = stderr_thread.join();

    let Ok(status) = child.wait() else {
        return DiffOutcome::Failed("git did not exit cleanly".to_string());
    };
    let Ok((raw, truncated)) = stdout_read else {
        return DiffOutcome::Failed("failed to read git diff output".to_string());
    };
    let stderr_raw = match stderr_read {
        Ok(Ok(buf)) => buf,
        Ok(Err(_)) => {
            return DiffOutcome::Failed("failed to read git diagnostics".to_string());
        }
        Err(_) => {
            return DiffOutcome::Failed("git diagnostics thread panicked".to_string());
        }
    };
    if truncated {
        return DiffOutcome::Patch(shape_truncated_patch(&raw));
    }
    if !status.success() && !(allow_exit_one && status.code() == Some(1)) {
        return DiffOutcome::Failed(failure_line(&stderr_raw));
    }
    let patch = String::from_utf8_lossy(&raw).into_owned();
    if patch.trim().is_empty() {
        return DiffOutcome::Clean;
    }
    DiffOutcome::Patch(patch)
}

fn read_capped(reader: impl Read, cap: usize) -> std::io::Result<(Vec<u8>, bool)> {
    let mut raw = Vec::new();
    reader.take((cap + 1) as u64).read_to_end(&mut raw)?;
    let truncated = raw.len() > cap;
    if truncated {
        raw.truncate(cap);
    }
    Ok((raw, truncated))
}

// A byte cap can split a line and a UTF-8 sequence; cutting back to the
// last full line removes both before the truncation marker is appended.
fn shape_truncated_patch(raw: &[u8]) -> String {
    let mut patch = String::from_utf8_lossy(raw).into_owned();
    match patch.rfind('\n') {
        Some(idx) => patch.truncate(idx),
        None => patch.clear(),
    }
    patch.push_str("\n[patch truncated at 1 MiB]\n");
    patch
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
            "refs/heads/main",
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
    fn diff_argv_disables_external_diff_programs() {
        let spec = spec(rev("HEAD"), DiffTarget::Worktree);
        let argv = diff_argv(&spec, "src/main.rs").unwrap();

        assert_eq!(argv[0], "diff");
        assert!(argv.contains(&"--no-ext-diff".to_string()));
        assert!(argv.contains(&"--no-textconv".to_string()));
        assert!(argv.contains(&"--no-color".to_string()));

        let sep = argv.iter().position(|arg| arg == "--").unwrap();
        assert_eq!(argv[sep + 1], "src/main.rs");
    }

    #[test]
    fn untracked_argv_synthesizes_against_dev_null() {
        let forward = untracked_argv("notes.md", false);
        assert_eq!(forward[0], "diff");
        assert!(forward.contains(&"--no-ext-diff".to_string()));
        assert!(forward.contains(&"--no-index".to_string()));
        assert!(!forward.contains(&"-R".to_string()));
        let sep = forward.iter().position(|arg| arg == "--").unwrap();
        assert_eq!(forward[sep + 1], "/dev/null");
        assert_eq!(forward[sep + 2], "notes.md");

        let reversed = untracked_argv("notes.md", true);
        assert!(reversed.contains(&"-R".to_string()));
    }

    #[test]
    fn tracked_argv_probes_one_literal_pathspec() {
        assert_eq!(
            tracked_argv("notes.md"),
            vec!["ls-files", "--error-unmatch", "--", "notes.md"]
        );
    }

    #[test]
    fn worktree_direction_identifies_single_worktree_side() {
        assert_eq!(
            worktree_direction(&spec(rev("HEAD"), DiffTarget::Worktree)),
            Some(false)
        );
        assert_eq!(
            worktree_direction(&spec(DiffTarget::Index, DiffTarget::Worktree)),
            Some(false)
        );
        assert_eq!(
            worktree_direction(&spec(DiffTarget::Worktree, rev("HEAD"))),
            Some(true)
        );
        assert_eq!(
            worktree_direction(&spec(DiffTarget::Worktree, DiffTarget::Index)),
            Some(true)
        );
        assert_eq!(worktree_direction(&spec(rev("a"), rev("b"))), None);
        assert_eq!(
            worktree_direction(&spec(rev("HEAD"), DiffTarget::Index)),
            None
        );
        assert_eq!(
            worktree_direction(&spec(DiffTarget::Worktree, DiffTarget::Worktree)),
            None
        );
    }

    #[test]
    fn read_capped_reports_truncation_and_errors() {
        struct FailingReader;
        impl Read for FailingReader {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("broken pipe"))
            }
        }

        let (raw, truncated) = read_capped(&b"short"[..], 16).unwrap();
        assert_eq!(raw, b"short");
        assert!(!truncated);

        let (raw, truncated) = read_capped(&b"0123456789"[..], 4).unwrap();
        assert_eq!(raw, b"0123");
        assert!(truncated);

        assert!(read_capped(FailingReader, 4).is_err());
    }

    #[test]
    fn shape_truncated_patch_drops_torn_lines_and_bytes() {
        let complete = shape_truncated_patch(b"line one\nline two\n");
        assert!(complete.starts_with("line one\nline two"));
        assert!(complete.ends_with("[patch truncated at 1 MiB]\n"));

        let torn = shape_truncated_patch(b"line one\npartial li");
        assert!(torn.starts_with("line one\n["));
        assert!(!torn.contains("partial"));

        let invalid = shape_truncated_patch(b"line one\ntail\xff\xfe");
        assert!(!invalid.contains('\u{fffd}'));
        assert!(invalid.starts_with("line one\n["));
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
