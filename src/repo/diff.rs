use std::fmt::Write as _;
use std::io::Read;
use std::path::Path;

use gix::bstr::ByteSlice;
use gix::diff::blob::ResourceKind;
use gix::diff::blob::platform::resource::Data;
use gix::objs::tree::EntryKind;

pub(crate) const WORKTREE: &str = ":worktree";
pub(crate) const INDEX: &str = ":index";

const MAX_PATCH_BYTES: usize = 1024 * 1024;
const MAX_DIFF_INPUT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_TARGET_LEN: usize = 256;
const TRUNCATION_MARKER: &str = "[patch truncated at 1 MiB]\n";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DiffTarget {
    Worktree,
    Index,
    Rev(String),
}

impl DiffTarget {
    // A colon never appears in a valid ref name, so the pseudo-ref tokens
    // cannot collide with a real ref, and rejecting remaining colons also
    // rejects git's rev:path blob syntax. A leading hyphen and a leading
    // dot are rejected so a target can never read as an option or a
    // three-dot range that DiffSpec::parse would split into a dotted ref.
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

#[derive(Clone, Copy, Debug)]
struct Side {
    id: gix::ObjectId,
    mode: EntryKind,
    present: bool,
}

pub(crate) fn unified_diff(root: &Path, spec: &DiffSpec, rel: &str) -> DiffOutcome {
    if spec.base == spec.head {
        return DiffOutcome::Clean;
    }
    let Ok(repo) = gix::open(root) else {
        return DiffOutcome::Failed("repository is unavailable".to_string());
    };
    let base = match side(&repo, &spec.base, root, rel) {
        Ok(side) => side,
        Err(reason) => return DiffOutcome::Failed(reason),
    };
    let head = match side(&repo, &spec.head, root, rel) {
        Ok(side) => side,
        Err(reason) => return DiffOutcome::Failed(reason),
    };
    if !base.present && !head.present {
        return DiffOutcome::Clean;
    }

    let roots = gix::diff::blob::pipeline::WorktreeRoots {
        old_root: matches!(spec.base, DiffTarget::Worktree).then(|| root.to_owned()),
        new_root: matches!(spec.head, DiffTarget::Worktree).then(|| root.to_owned()),
    };
    let mut platform = match repo.diff_resource_cache(gix::diff::blob::pipeline::Mode::ToGit, roots)
    {
        Ok(platform) => platform,
        Err(_) => return DiffOutcome::Failed("cannot prepare diff resources".to_string()),
    };
    let threshold = &mut platform.filter.options.large_file_threshold_bytes;
    if *threshold == 0 || *threshold > MAX_DIFF_INPUT_BYTES {
        *threshold = MAX_DIFF_INPUT_BYTES;
    }
    platform
        .filter
        .worktree_filter
        .options_mut()
        .drivers
        .clear();

    let rel_bytes = rel.as_bytes().as_bstr();
    if platform
        .set_resource(
            base.id,
            base.mode,
            rel_bytes,
            ResourceKind::OldOrSource,
            &repo.objects,
        )
        .is_err()
        || platform
            .set_resource(
                head.id,
                head.mode,
                rel_bytes,
                ResourceKind::NewOrDestination,
                &repo.objects,
            )
            .is_err()
    {
        return DiffOutcome::Failed("cannot read diff resources".to_string());
    }

    let prepared = match platform.prepare_diff() {
        Ok(prepared) => prepared,
        Err(_) => return DiffOutcome::Failed("cannot prepare file diff".to_string()),
    };
    let data_equal = match (prepared.old.data, prepared.new.data) {
        (Data::Missing, Data::Missing) => true,
        (Data::Buffer { buf: old, .. }, Data::Buffer { buf: new, .. }) => old == new,
        (Data::Binary { .. }, Data::Binary { .. }) => {
            match binary_equal(&repo, &spec.base, base, &spec.head, head, root, rel) {
                Ok(equal) => equal,
                Err(reason) => return DiffOutcome::Failed(reason),
            }
        }
        _ => false,
    };
    if base.present == head.present && base.mode == head.mode && data_equal {
        return DiffOutcome::Clean;
    }

    let mut patch = patch_header(rel, base, head, !data_equal);
    match prepared.operation {
        gix::diff::blob::platform::prepare_diff::Operation::InternalDiff { algorithm } => {
            let old = prepared.old.data.as_slice().unwrap_or_default();
            let new = prepared.new.data.as_slice().unwrap_or_default();
            if old != new {
                patch = unified_patch(patch, &prepared, algorithm);
            }
        }
        gix::diff::blob::platform::prepare_diff::Operation::SourceOrDestinationIsBinary => {
            let old = patch_label(base.present, "a", rel);
            let new = patch_label(head.present, "b", rel);
            writeln!(patch, "Binary files {old} and {new} differ").unwrap();
        }
        gix::diff::blob::platform::prepare_diff::Operation::ExternalCommand { .. } => {
            return DiffOutcome::Failed("external diff drivers are disabled".to_string());
        }
    }
    DiffOutcome::Patch(finish_patch(patch))
}

fn side(
    repo: &gix::Repository,
    target: &DiffTarget,
    root: &Path,
    rel: &str,
) -> Result<Side, String> {
    match target {
        DiffTarget::Worktree => worktree_side(repo, root, rel),
        DiffTarget::Index => index_side(repo, rel),
        DiffTarget::Rev(rev) => rev_side(repo, rev, rel),
    }
}

fn missing_side(repo: &gix::Repository) -> Side {
    Side {
        id: gix::ObjectId::null(repo.object_hash()),
        mode: EntryKind::Blob,
        present: false,
    }
}

fn rev_side(repo: &gix::Repository, rev: &str, rel: &str) -> Result<Side, String> {
    let tree = repo
        .rev_parse_single(rev)
        .map_err(|_| format!("cannot resolve revision '{rev}'"))?
        .object()
        .map_err(|_| "cannot read revision".to_string())?
        .peel_to_tree()
        .map_err(|_| "revision has no tree".to_string())?;
    let Some(entry) = tree
        .lookup_entry_by_path(rel)
        .map_err(|_| "cannot read revision tree".to_string())?
    else {
        return Ok(missing_side(repo));
    };
    let mode = entry.mode().kind();
    regular_mode(mode)?;
    Ok(Side {
        id: entry.object_id(),
        mode,
        present: true,
    })
}

fn index_side(repo: &gix::Repository, rel: &str) -> Result<Side, String> {
    let index = repo
        .index_or_empty()
        .map_err(|_| "cannot read index".to_string())?;
    let Some(entry) = index.entry_by_path(rel.into()) else {
        return Ok(missing_side(repo));
    };
    let mode = entry
        .mode
        .to_tree_entry_mode()
        .ok_or_else(|| "staged file has an invalid mode".to_string())?
        .kind();
    regular_mode(mode)?;
    Ok(Side {
        id: entry.id,
        mode,
        present: true,
    })
}

fn worktree_side(repo: &gix::Repository, root: &Path, rel: &str) -> Result<Side, String> {
    let metadata = match gix::index::fs::Metadata::from_path_no_follow(&root.join(rel)) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(missing_side(repo)),
        Err(_) => return Err("cannot read worktree file metadata".to_string()),
    };
    if metadata.is_dir() {
        return Err("worktree target is not a file".to_string());
    }
    let fs = repo
        .filesystem_options()
        .map_err(|_| "cannot read repository filesystem options".to_string())?;
    let mode = if fs.symlink && metadata.is_symlink() {
        EntryKind::Link
    } else if fs.executable_bit && metadata.is_executable() {
        EntryKind::BlobExecutable
    } else {
        EntryKind::Blob
    };
    Ok(Side {
        id: gix::ObjectId::null(repo.object_hash()),
        mode,
        present: true,
    })
}

fn binary_equal(
    repo: &gix::Repository,
    old_target: &DiffTarget,
    old: Side,
    new_target: &DiffTarget,
    new: Side,
    root: &Path,
    rel: &str,
) -> Result<bool, String> {
    if old.mode != new.mode || old.present != new.present {
        return Ok(false);
    }
    if !old.id.is_null() && old.id == new.id {
        return Ok(true);
    }
    let old = raw_content(repo, old_target, old, root, rel)?;
    let new = raw_content(repo, new_target, new, root, rel)?;
    Ok(matches!((old, new), (Some(old), Some(new)) if old == new))
}

fn raw_content(
    repo: &gix::Repository,
    target: &DiffTarget,
    side: Side,
    root: &Path,
    rel: &str,
) -> Result<Option<Vec<u8>>, String> {
    if !side.present {
        return Ok(Some(Vec::new()));
    }
    if matches!(target, DiffTarget::Worktree) {
        let path = root.join(rel);
        let metadata = std::fs::symlink_metadata(&path)
            .map_err(|_| "cannot read worktree file metadata".to_string())?;
        if metadata.len() > MAX_DIFF_INPUT_BYTES {
            return Ok(None);
        }
        return if side.mode == EntryKind::Link {
            std::fs::read_link(path)
                .map(|target| Some(target.as_os_str().as_encoded_bytes().to_vec()))
                .map_err(|_| "cannot read worktree symbolic link".to_string())
        } else {
            canonical_worktree_content(repo, root, rel)
        };
    }
    let header = repo
        .find_header(side.id)
        .map_err(|_| "cannot read file object header".to_string())?;
    if header.size() > MAX_DIFF_INPUT_BYTES {
        return Ok(None);
    }
    repo.find_object(side.id)
        .map(|object| Some(object.data.clone()))
        .map_err(|_| "cannot read file object".to_string())
}

fn canonical_worktree_content(
    repo: &gix::Repository,
    root: &Path,
    rel: &str,
) -> Result<Option<Vec<u8>>, String> {
    let roots = gix::diff::blob::pipeline::WorktreeRoots {
        old_root: Some(root.to_owned()),
        new_root: None,
    };
    let mut platform = repo
        .diff_resource_cache(gix::diff::blob::pipeline::Mode::ToGit, roots)
        .map_err(|_| "cannot prepare worktree conversion".to_string())?;
    platform
        .filter
        .worktree_filter
        .options_mut()
        .drivers
        .clear();
    let attrs = platform
        .attr_stack
        .at_entry(rel.as_bytes().as_bstr(), None, &repo.objects)
        .map_err(|_| "cannot read file attributes".to_string())?;
    let file =
        std::fs::File::open(root.join(rel)).map_err(|_| "cannot read worktree file".to_string())?;
    let mut match_attrs = |_: &gix::bstr::BStr, out: &mut gix::attrs::search::Outcome| {
        attrs.matching_attributes(out);
    };
    let mut no_index_object = |_: &mut Vec<u8>| Ok(None);
    let mut converted = platform
        .filter
        .worktree_filter
        .convert_to_git(file, Path::new(rel), &mut match_attrs, &mut no_index_object)
        .map_err(|_| "cannot convert worktree file".to_string())?;
    let mut content = Vec::new();
    converted
        .read_to_end(&mut content)
        .map_err(|_| "cannot read converted worktree file".to_string())?;
    Ok(Some(content))
}

fn regular_mode(mode: EntryKind) -> Result<(), String> {
    match mode {
        EntryKind::Blob | EntryKind::BlobExecutable | EntryKind::Link => Ok(()),
        EntryKind::Tree | EntryKind::Commit => Err("target is not a regular file".to_string()),
    }
}

fn patch_header(rel: &str, old: Side, new: Side, content_changed: bool) -> String {
    let old_path = patch_path("a", rel);
    let new_path = patch_path("b", rel);
    let mut patch = format!("diff --git {old_path} {new_path}\n");
    match (old.present, new.present) {
        (false, true) => {
            writeln!(patch, "new file mode {}", mode_str(new.mode)).unwrap();
        }
        (true, false) => {
            writeln!(patch, "deleted file mode {}", mode_str(old.mode)).unwrap();
        }
        (true, true) if old.mode != new.mode => {
            writeln!(patch, "old mode {}", mode_str(old.mode)).unwrap();
            writeln!(patch, "new mode {}", mode_str(new.mode)).unwrap();
        }
        _ => {}
    }
    if content_changed || old.present != new.present {
        writeln!(patch, "--- {}", patch_label(old.present, "a", rel)).unwrap();
        writeln!(patch, "+++ {}", patch_label(new.present, "b", rel)).unwrap();
    }
    patch
}

fn mode_str(mode: EntryKind) -> &'static str {
    match mode {
        EntryKind::Blob => "100644",
        EntryKind::BlobExecutable => "100755",
        EntryKind::Link => "120000",
        EntryKind::Tree => "040000",
        EntryKind::Commit => "160000",
    }
}

fn patch_label(present: bool, prefix: &str, rel: &str) -> String {
    if present {
        patch_path(prefix, rel)
    } else {
        "/dev/null".to_string()
    }
}

// Git patch paths use double-quoted C escapes when whitespace, control
// characters, quotes, or backslashes would make the header ambiguous.
fn patch_path(prefix: &str, rel: &str) -> String {
    let path = format!("{prefix}/{rel}");
    if !path
        .chars()
        .any(|ch| ch.is_whitespace() || ch.is_control() || matches!(ch, '\\' | '"'))
    {
        return path;
    }
    let mut quoted = String::from("\"");
    for ch in path.chars() {
        match ch {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            ch if ch.is_control() => {
                for byte in ch.to_string().bytes() {
                    write!(quoted, "\\{byte:03o}").unwrap();
                }
            }
            ch => quoted.push(ch),
        }
    }
    quoted.push('"');
    quoted
}

fn unified_patch(
    patch: String,
    prepared: &gix::diff::blob::platform::prepare_diff::Outcome<'_>,
    algorithm: gix::diff::blob::Algorithm,
) -> String {
    use gix::diff::blob::unified_diff::ContextSize;
    use gix::diff::blob::{UnifiedDiff, diff_with_slider_heuristics};

    let input = prepared.interned_input();
    let diff = diff_with_slider_heuristics(algorithm, &input);
    let old = prepared.old.data.as_slice().unwrap_or_default();
    let new = prepared.new.data.as_slice().unwrap_or_default();
    let writer = HunkWriter::new(
        patch,
        input.before.len(),
        input.after.len(),
        old.ends_with(b"\n"),
        new.ends_with(b"\n"),
    );
    UnifiedDiff::new(&diff, &input, writer, ContextSize::symmetrical(3))
        .consume()
        .expect("writing diff hunks into memory cannot fail")
}

struct HunkWriter {
    patch: Vec<u8>,
    truncated: bool,
    old_lines: usize,
    new_lines: usize,
    old_ends_with_newline: bool,
    new_ends_with_newline: bool,
}

impl HunkWriter {
    fn new(
        patch: String,
        old_lines: usize,
        new_lines: usize,
        old_ends_with_newline: bool,
        new_ends_with_newline: bool,
    ) -> Self {
        Self {
            patch: patch.into_bytes(),
            truncated: false,
            old_lines,
            new_lines,
            old_ends_with_newline,
            new_ends_with_newline,
        }
    }

    fn push(&mut self, bytes: &[u8]) {
        let limit = MAX_PATCH_BYTES - TRUNCATION_MARKER.len();
        if self.truncated || self.patch.len().saturating_add(bytes.len()) > limit {
            self.truncated = true;
            return;
        }
        self.patch.extend_from_slice(bytes);
    }

    fn no_newline(&mut self) {
        self.push(b"\\ No newline at end of file\n");
    }

    fn push_line(&mut self, prefix: u8, content: &[u8]) {
        let limit = MAX_PATCH_BYTES - TRUNCATION_MARKER.len();
        let len = content.len().saturating_add(2);
        if self.truncated || self.patch.len().saturating_add(len) > limit {
            self.truncated = true;
            return;
        }
        self.patch.push(prefix);
        self.patch.extend_from_slice(content);
        self.patch.push(b'\n');
    }
}

impl gix::diff::blob::unified_diff::ConsumeHunk for HunkWriter {
    type Out = String;

    fn consume_hunk(
        &mut self,
        header: gix::diff::blob::unified_diff::HunkHeader,
        lines: &[(gix::diff::blob::unified_diff::DiffLineKind, &[u8])],
    ) -> std::io::Result<()> {
        self.push(format!("{header}\n").as_bytes());
        let mut old_line = header.before_hunk_start as usize;
        let mut new_line = header.after_hunk_start as usize;
        for &(kind, content) in lines {
            self.push_line(kind.to_prefix() as u8, content);
            match kind {
                gix::diff::blob::unified_diff::DiffLineKind::Remove => {
                    if old_line == self.old_lines && !self.old_ends_with_newline {
                        self.no_newline();
                    }
                    old_line += 1;
                }
                gix::diff::blob::unified_diff::DiffLineKind::Add => {
                    if new_line == self.new_lines && !self.new_ends_with_newline {
                        self.no_newline();
                    }
                    new_line += 1;
                }
                gix::diff::blob::unified_diff::DiffLineKind::Context => {
                    if (old_line == self.old_lines && !self.old_ends_with_newline)
                        || (new_line == self.new_lines && !self.new_ends_with_newline)
                    {
                        self.no_newline();
                    }
                    old_line += 1;
                    new_line += 1;
                }
            }
        }
        Ok(())
    }

    fn finish(mut self) -> Self::Out {
        if self.truncated {
            self.patch.extend_from_slice(TRUNCATION_MARKER.as_bytes());
        }
        String::from_utf8_lossy(&self.patch).into_owned()
    }
}

fn finish_patch(mut patch: String) -> String {
    if patch.len() > MAX_PATCH_BYTES {
        patch.truncate(MAX_PATCH_BYTES - TRUNCATION_MARKER.len());
        if let Some(end) = patch.rfind('\n') {
            patch.truncate(end + 1);
        }
        patch.push_str(TRUNCATION_MARKER);
    }
    patch
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;
    use gix::objs::tree::EntryKind;
    use std::fs;

    fn rev(name: &str) -> DiffTarget {
        DiffTarget::Rev(name.to_string())
    }

    fn spec(base: DiffTarget, head: DiffTarget) -> DiffSpec {
        DiffSpec { base, head }
    }

    fn init_repo(dir: &Path) -> gix::Repository {
        gix::init(dir).expect("init repository");
        let config = dir.join(".git/config");
        let mut text = fs::read_to_string(&config).expect("read repository config");
        text.push_str("[user]\n\tname = Test\n\temail = test@example.com\n");
        fs::write(&config, text).unwrap();
        gix::open(dir).expect("open repository")
    }

    fn commit_entries(
        repo: &gix::Repository,
        files: &[(&str, EntryKind, &[u8])],
        message: &str,
    ) -> gix::ObjectId {
        let parent = repo.head_id().ok().map(|id| id.detach());
        let base = parent
            .map(|id| {
                repo.find_commit(id)
                    .expect("find parent")
                    .tree_id()
                    .expect("find parent tree")
                    .detach()
            })
            .unwrap_or_else(|| gix::ObjectId::empty_tree(repo.object_hash()));
        let mut editor = repo.edit_tree(base).expect("edit tree");
        for (path, mode, bytes) in files {
            let blob = repo.write_blob(bytes).expect("write blob").detach();
            editor.upsert(*path, *mode, blob).expect("upsert");
        }
        let tree = editor.write().expect("write tree").detach();
        repo.commit("HEAD", message, tree, parent)
            .expect("commit")
            .detach()
    }

    fn commit(repo: &gix::Repository, files: &[(&str, &[u8])], message: &str) -> gix::ObjectId {
        let files = files
            .iter()
            .map(|(path, bytes)| (*path, EntryKind::Blob, *bytes))
            .collect::<Vec<_>>();
        commit_entries(repo, &files, message)
    }

    fn write_index(repo: &gix::Repository, files: &[(&str, &[u8])]) {
        let mut state = gix::index::State::new(repo.object_hash());
        for (path, bytes) in files {
            let id = repo.write_blob(bytes).expect("write staged blob").detach();
            state.dangerously_push_entry(
                Default::default(),
                id,
                gix::index::entry::Flags::empty(),
                gix::index::entry::Mode::FILE,
                path.as_bytes().as_bstr(),
            );
        }
        state.sort_entries();
        let mut index = gix::index::File::from_state(state, repo.index_path());
        index.write(Default::default()).expect("write index");
    }

    fn patch(outcome: DiffOutcome) -> String {
        let DiffOutcome::Patch(patch) = outcome else {
            panic!("expected patch, got {outcome:?}");
        };
        patch
    }

    fn assert_change(patch: &str, old: &str, new: &str) {
        assert!(patch.contains(&format!("\n-{old}\n")), "{patch}");
        assert!(patch.contains(&format!("\n+{new}\n")), "{patch}");
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
    fn equal_targets_are_clean_without_opening_a_repo() {
        let outcome = unified_diff(
            Path::new("/nonexistent"),
            &spec(DiffTarget::Worktree, DiffTarget::Worktree),
            "README.md",
        );
        assert_eq!(outcome, DiffOutcome::Clean);
    }

    #[test]
    fn modifies_a_tracked_file() {
        let td = TempDir::new("ghrm-diff-modify");
        let repo = init_repo(td.path());
        commit(&repo, &[("a.txt", b"a\nb\nc\n")], "add a");
        fs::write(td.path().join("a.txt"), b"a\nB\nc\n").unwrap();

        let patch = patch(unified_diff(
            td.path(),
            &spec(rev("HEAD"), DiffTarget::Worktree),
            "a.txt",
        ));

        assert!(patch.starts_with("diff --git a/a.txt b/a.txt\n"), "{patch}");
        assert!(patch.contains("--- a/a.txt\n+++ b/a.txt\n"), "{patch}");
        assert!(patch.contains("@@"), "{patch}");
        assert_change(&patch, "b", "B");
    }

    #[test]
    fn adds_an_untracked_file() {
        let td = TempDir::new("ghrm-diff-add");
        let repo = init_repo(td.path());
        commit(&repo, &[("keep.txt", b"keep\n")], "keep");
        fs::write(td.path().join("new.txt"), b"one\ntwo\n").unwrap();

        let patch = patch(unified_diff(
            td.path(),
            &spec(rev("HEAD"), DiffTarget::Worktree),
            "new.txt",
        ));

        assert!(patch.contains("new file mode 100644"), "{patch}");
        assert!(patch.contains("--- /dev/null\n+++ b/new.txt\n"), "{patch}");
        assert!(patch.contains("+one"), "{patch}");
        assert!(patch.contains("+two"), "{patch}");
    }

    #[test]
    fn deletes_a_removed_file() {
        let td = TempDir::new("ghrm-diff-delete");
        let repo = init_repo(td.path());
        commit(&repo, &[("gone.txt", b"one\ntwo\n")], "add gone");
        // no worktree file, so the worktree side is absent

        let patch = patch(unified_diff(
            td.path(),
            &spec(rev("HEAD"), DiffTarget::Worktree),
            "gone.txt",
        ));

        assert!(patch.contains("deleted file mode 100644"), "{patch}");
        assert!(patch.contains("--- a/gone.txt\n+++ /dev/null\n"), "{patch}");
        assert!(patch.contains("-one"), "{patch}");
        assert!(patch.contains("-two"), "{patch}");
    }

    #[test]
    fn clean_when_the_worktree_matches_the_revision() {
        let td = TempDir::new("ghrm-diff-clean");
        let repo = init_repo(td.path());
        commit(&repo, &[("a.txt", b"same\n")], "add a");
        fs::write(td.path().join("a.txt"), b"same\n").unwrap();

        let outcome = unified_diff(td.path(), &spec(rev("HEAD"), DiffTarget::Worktree), "a.txt");

        assert_eq!(outcome, DiffOutcome::Clean);
    }

    #[test]
    fn binary_content_reports_a_notice() {
        let td = TempDir::new("ghrm-diff-binary");
        let repo = init_repo(td.path());
        commit(&repo, &[("blob.bin", b"\x00\x01\x02old")], "add blob");
        fs::write(td.path().join("blob.bin"), b"\x00\x01\x02new").unwrap();

        let patch = patch(unified_diff(
            td.path(),
            &spec(rev("HEAD"), DiffTarget::Worktree),
            "blob.bin",
        ));

        assert!(patch.contains("Binary files"), "{patch}");
    }

    #[test]
    fn unchanged_binary_content_is_clean() {
        let td = TempDir::new("ghrm-diff-binary-clean");
        let repo = init_repo(td.path());
        let content = b"\x00\x01\x02same";
        commit(&repo, &[("blob.bin", content)], "add blob");
        fs::write(td.path().join("blob.bin"), content).unwrap();

        let outcome = unified_diff(
            td.path(),
            &spec(rev("HEAD"), DiffTarget::Worktree),
            "blob.bin",
        );

        assert_eq!(outcome, DiffOutcome::Clean);
    }

    #[test]
    fn compares_every_distinct_target_pair() {
        let td = TempDir::new("ghrm-diff-matrix");
        let repo = init_repo(td.path());
        let old = commit(&repo, &[("a.txt", b"old\n")], "old");
        let head = commit(&repo, &[("a.txt", b"head\n")], "head");
        write_index(&repo, &[("a.txt", b"staged\n")]);
        fs::write(td.path().join("a.txt"), b"worktree\n").unwrap();

        let old = rev(&old.to_string());
        let head = rev(&head.to_string());
        let cases = [
            (spec(old.clone(), head.clone()), "old", "head"),
            (spec(head.clone(), DiffTarget::Index), "head", "staged"),
            (spec(head.clone(), DiffTarget::Worktree), "head", "worktree"),
            (
                spec(DiffTarget::Index, DiffTarget::Worktree),
                "staged",
                "worktree",
            ),
            (spec(DiffTarget::Index, head.clone()), "staged", "head"),
            (spec(DiffTarget::Worktree, head), "worktree", "head"),
            (
                spec(DiffTarget::Worktree, DiffTarget::Index),
                "worktree",
                "staged",
            ),
        ];
        for (spec, old, new) in cases {
            let patch = patch(unified_diff(td.path(), &spec, "a.txt"));
            assert_change(&patch, old, new);
        }
    }

    #[test]
    fn normalizes_worktree_line_endings_from_attributes() {
        let td = TempDir::new("ghrm-diff-eol");
        let repo = init_repo(td.path());
        commit(
            &repo,
            &[
                (".gitattributes", b"a.txt text eol=lf\n"),
                ("a.txt", b"one\ntwo\n"),
            ],
            "add text",
        );
        fs::write(td.path().join(".gitattributes"), b"a.txt text eol=lf\n").unwrap();
        fs::write(td.path().join("a.txt"), b"one\r\ntwo\r\n").unwrap();

        let outcome = unified_diff(td.path(), &spec(rev("HEAD"), DiffTarget::Worktree), "a.txt");

        assert_eq!(outcome, DiffOutcome::Clean);
    }

    #[test]
    fn honors_the_binary_diff_attribute() {
        let td = TempDir::new("ghrm-diff-attribute-binary");
        let repo = init_repo(td.path());
        commit(
            &repo,
            &[(".gitattributes", b"a.txt -diff\n"), ("a.txt", b"old\n")],
            "add binary attribute",
        );
        fs::write(td.path().join(".gitattributes"), b"a.txt -diff\n").unwrap();
        fs::write(td.path().join("a.txt"), b"new\n").unwrap();

        let patch = patch(unified_diff(
            td.path(),
            &spec(rev("HEAD"), DiffTarget::Worktree),
            "a.txt",
        ));

        assert!(patch.contains("Binary files"), "{patch}");
        assert!(!patch.contains("-old"), "{patch}");
    }

    #[test]
    fn binary_attributes_still_normalize_clean_line_endings() {
        let td = TempDir::new("ghrm-diff-attribute-binary-clean");
        let repo = init_repo(td.path());
        let attrs = b"a.txt -diff text eol=lf\n";
        commit(
            &repo,
            &[(".gitattributes", attrs), ("a.txt", b"one\ntwo\n")],
            "add binary text attribute",
        );
        fs::write(td.path().join(".gitattributes"), attrs).unwrap();
        fs::write(td.path().join("a.txt"), b"one\r\ntwo\r\n").unwrap();

        let outcome = unified_diff(td.path(), &spec(rev("HEAD"), DiffTarget::Worktree), "a.txt");

        assert_eq!(outcome, DiffOutcome::Clean);
    }

    #[test]
    fn repository_filters_are_not_executed() {
        let td = TempDir::new("ghrm-diff-no-filter-process");
        let repo = init_repo(td.path());
        let config = td.path().join(".git/config");
        let mut text = fs::read_to_string(&config).unwrap();
        text.push_str(
            "[filter \"explode\"]\n\tclean = /ghrm-filter-must-not-run\n\trequired = true\n[diff \"explode\"]\n\ttextconv = /ghrm-textconv-must-not-run\n",
        );
        fs::write(config, text).unwrap();
        commit(
            &repo,
            &[
                (".gitattributes", b"a.txt filter=explode diff=explode\n"),
                ("a.txt", b"old\n"),
            ],
            "add filters",
        );
        fs::write(
            td.path().join(".gitattributes"),
            b"a.txt filter=explode diff=explode\n",
        )
        .unwrap();
        fs::write(td.path().join("a.txt"), b"new\n").unwrap();

        let patch = patch(unified_diff(
            td.path(),
            &spec(rev("HEAD"), DiffTarget::Worktree),
            "a.txt",
        ));

        assert_change(&patch, "old", "new");
    }

    #[test]
    fn empty_file_addition_and_deletion_keep_direction() {
        let add = TempDir::new("ghrm-diff-empty-add");
        let add_repo = init_repo(add.path());
        commit(&add_repo, &[("keep.txt", b"keep\n")], "keep");
        fs::write(add.path().join("empty.txt"), b"").unwrap();
        let addition = patch(unified_diff(
            add.path(),
            &spec(rev("HEAD"), DiffTarget::Worktree),
            "empty.txt",
        ));
        assert!(addition.contains("new file mode 100644"), "{addition}");
        assert!(
            addition.contains("--- /dev/null\n+++ b/empty.txt\n"),
            "{addition}"
        );

        let delete = TempDir::new("ghrm-diff-empty-delete");
        let delete_repo = init_repo(delete.path());
        commit(&delete_repo, &[("empty.txt", b"")], "empty");
        let deletion = patch(unified_diff(
            delete.path(),
            &spec(rev("HEAD"), DiffTarget::Worktree),
            "empty.txt",
        ));
        assert!(deletion.contains("deleted file mode 100644"), "{deletion}");
        assert!(
            deletion.contains("--- a/empty.txt\n+++ /dev/null\n"),
            "{deletion}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn worktree_symlink_uses_the_link_target() {
        use std::os::unix::fs::symlink;

        let td = TempDir::new("ghrm-diff-symlink");
        let repo = init_repo(td.path());
        commit_entries(
            &repo,
            &[("link.txt", EntryKind::Link, b"target.txt")],
            "add link",
        );
        fs::write(td.path().join("target.txt"), b"external content\n").unwrap();
        symlink("target.txt", td.path().join("link.txt")).unwrap();

        let outcome = unified_diff(
            td.path(),
            &spec(rev("HEAD"), DiffTarget::Worktree),
            "link.txt",
        );

        assert_eq!(outcome, DiffOutcome::Clean);
    }

    #[cfg(unix)]
    #[test]
    fn reports_executable_mode_changes() {
        use std::os::unix::fs::PermissionsExt;

        let td = TempDir::new("ghrm-diff-mode");
        let repo = init_repo(td.path());
        commit(&repo, &[("run.sh", b"exit 0\n")], "add script");
        fs::write(td.path().join("run.sh"), b"exit 0\n").unwrap();
        let mut permissions = fs::metadata(td.path().join("run.sh"))
            .unwrap()
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(td.path().join("run.sh"), permissions).unwrap();

        let patch = patch(unified_diff(
            td.path(),
            &spec(rev("HEAD"), DiffTarget::Worktree),
            "run.sh",
        ));

        assert!(patch.contains("old mode 100644"), "{patch}");
        assert!(patch.contains("new mode 100755"), "{patch}");
        assert!(!patch.contains("@@"), "{patch}");
    }

    #[test]
    fn worktree_read_failures_are_not_deletions() {
        let td = TempDir::new("ghrm-diff-read-failure");
        let repo = init_repo(td.path());
        commit(&repo, &[("a.txt", b"file\n")], "add file");
        fs::create_dir(td.path().join("a.txt")).unwrap();

        let outcome = unified_diff(td.path(), &spec(rev("HEAD"), DiffTarget::Worktree), "a.txt");

        assert!(matches!(outcome, DiffOutcome::Failed(_)), "{outcome:?}");
    }

    #[test]
    fn patch_paths_are_quoted_when_ambiguous() {
        assert_eq!(patch_path("a", "odd name.txt"), "\"a/odd name.txt\"");
        assert_eq!(patch_path("b", "tab\tname.txt"), "\"b/tab\\tname.txt\"");
    }

    #[test]
    fn reports_missing_final_newlines() {
        let td = TempDir::new("ghrm-diff-no-newline");
        let repo = init_repo(td.path());
        commit(&repo, &[("a.txt", b"old")], "old");
        fs::write(td.path().join("a.txt"), b"new").unwrap();

        let patch = patch(unified_diff(
            td.path(),
            &spec(rev("HEAD"), DiffTarget::Worktree),
            "a.txt",
        ));

        assert_eq!(patch.matches("\\ No newline at end of file").count(), 2);
    }

    #[test]
    fn unknown_revision_fails() {
        let td = TempDir::new("ghrm-diff-badrev");
        let repo = init_repo(td.path());
        commit(&repo, &[("a.txt", b"a\n")], "add a");

        let outcome = unified_diff(
            td.path(),
            &spec(rev("does-not-exist"), DiffTarget::Worktree),
            "a.txt",
        );

        assert!(matches!(outcome, DiffOutcome::Failed(_)), "{outcome:?}");
    }

    #[test]
    fn hunk_writer_caps_complete_output_lines() {
        let mut writer = HunkWriter::new("header\n".to_string(), 0, 0, true, true);
        writer.push(&vec![b'x'; MAX_PATCH_BYTES]);
        let patch = gix::diff::blob::unified_diff::ConsumeHunk::finish(writer);

        assert!(patch.starts_with("header\n"));
        assert!(patch.ends_with(TRUNCATION_MARKER));
        assert!(patch.len() <= MAX_PATCH_BYTES);
    }
}
