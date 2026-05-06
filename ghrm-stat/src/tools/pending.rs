use crate::{Context, Row, repo};
use anyhow::Result;

pub fn run(ctx: &Context) -> Result<Vec<Row>> {
    let statuses = repo(ctx)
        .status(gix::progress::Discard)?
        .dirwalk_options(|options| options.emit_untracked(gix::dir::walk::EmissionMode::Matching))
        .into_index_worktree_iter(Vec::new())?;

    let (added, deleted, modified) = statuses
        .take_while(Result::is_ok)
        .filter_map(Result::ok)
        .filter_map(|item| item.summary())
        .fold((0, 0, 0), |(added, deleted, modified), status| {
            use gix::status::index_worktree::iter::Summary;
            match status {
                Summary::Removed => (added, deleted + 1, modified),
                Summary::Added | Summary::Copied => (added + 1, deleted, modified),
                Summary::Modified | Summary::TypeChange => (added, deleted, modified + 1),
                Summary::Renamed => (added + 1, deleted + 1, modified),
                Summary::IntentToAdd | Summary::Conflict => (added, deleted, modified),
            }
        });

    if added == 0 && deleted == 0 && modified == 0 {
        return Ok(Vec::new());
    }

    Ok(vec![
        Row::new("added", added.to_string()),
        Row::new("deleted", deleted.to_string()),
        Row::new("modified", modified.to_string()),
    ])
}
