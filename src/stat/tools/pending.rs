use crate::stat::{Context, Row, repo};
use anyhow::{Context as _, Result};

pub fn run(ctx: &Context) -> Result<Vec<Row>> {
    // Status constructs its resource cache internally. Exclude configuration
    // sections from trusted lookups so repository filter drivers cannot run.
    let workdir = repo(ctx)
        .workdir()
        .context("repository has no working directory")?;
    let repo = gix::open_opts(
        workdir,
        gix::open::Options::default().filter_config_section(|_| false),
    )?;

    let statuses = repo
        .status(gix::progress::Discard)?
        .dirwalk_options(|options| options.emit_untracked(gix::dir::walk::EmissionMode::Matching))
        .into_index_worktree_iter(Vec::new())?;

    use gix::status::index_worktree::iter::Summary;
    let (mut added, mut deleted, mut modified) = (0u32, 0u32, 0u32);
    for item in statuses {
        let Some(status) = item?.summary() else {
            continue;
        };
        match status {
            Summary::Removed => deleted += 1,
            Summary::Added | Summary::Copied => added += 1,
            Summary::Modified | Summary::TypeChange => modified += 1,
            Summary::Renamed => {
                added += 1;
                deleted += 1;
            }
            Summary::IntentToAdd | Summary::Conflict => {}
        }
    }

    if added == 0 && deleted == 0 && modified == 0 {
        return Ok(Vec::new());
    }

    Ok(vec![
        Row::new("added", added.to_string()),
        Row::new("deleted", deleted.to_string()),
        Row::new("modified", modified.to_string()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stat::Config;
    use crate::testutil::TempDir;
    use gix::bstr::ByteSlice;
    use std::fs;

    fn init_repo(dir: &std::path::Path) -> gix::Repository {
        gix::init(dir).expect("init repository")
    }

    fn stage(repo: &gix::Repository, path: &str, bytes: &[u8]) {
        let mut state = gix::index::State::new(repo.object_hash());
        let id = repo.write_blob(bytes).expect("write staged blob").detach();
        state.dangerously_push_entry(
            Default::default(),
            id,
            gix::index::entry::Flags::empty(),
            gix::index::entry::Mode::FILE,
            path.as_bytes().as_bstr(),
        );
        state.sort_entries();
        let mut index = gix::index::File::from_state(state, repo.index_path());
        index.write(Default::default()).expect("write index");
    }

    fn modified_count(rows: &[Row]) -> &str {
        rows.iter()
            .find(|row| row.key == "modified")
            .map(|row| row.value.as_str())
            .unwrap_or("0")
    }

    #[test]
    fn counts_modified_tracked_file() {
        let dir = TempDir::new("ghrm-pending-stat-modified");
        let repo = init_repo(dir.path());
        stage(&repo, "a.txt", b"old\n");
        fs::write(dir.path().join("a.txt"), b"new\n").unwrap();

        let rows = run(&context(dir.path())).expect("status collection");

        assert_eq!(modified_count(&rows), "1");
    }

    #[test]
    fn status_does_not_execute_repository_filters() {
        let dir = TempDir::new("ghrm-pending-stat-no-filter");
        let repo = init_repo(dir.path());
        stage(&repo, "a.txt", b"old\n");

        // A required clean driver pointing at a missing command would abort
        // status with an error if it were ever executed. Same-length contents
        // force a content comparison rather than a size-only shortcut.
        let config = dir.path().join(".git/config");
        let mut text = fs::read_to_string(&config).unwrap();
        text.push_str("[filter \"explode\"]\n\tclean = /ghrm-pending-filter-must-not-run\n\trequired = true\n");
        fs::write(&config, text).unwrap();
        fs::write(dir.path().join(".gitattributes"), b"a.txt filter=explode\n").unwrap();
        fs::write(dir.path().join("a.txt"), b"new\n").unwrap();

        let rows = run(&context(dir.path())).expect("status must not execute filter drivers");

        assert_eq!(modified_count(&rows), "1");
    }

    fn context(dir: &std::path::Path) -> Context {
        Context {
            root: dir.to_path_buf(),
            config: Config::default(),
            repo: gix::open(dir).expect("open repository"),
            history: std::sync::OnceLock::new(),
            language_summary: std::sync::OnceLock::new(),
            metadata: std::sync::OnceLock::new(),
        }
    }
}
