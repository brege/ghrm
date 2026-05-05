use crate::{Context, Row, repo};
use anyhow::Result;

pub fn run(ctx: &Context) -> Result<Vec<Row>> {
    let repo = repo(ctx);
    let name = ctx
        .root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tags = repo.references()?.tags()?.count();
    let remote_branches = repo.references()?.remote_branches()?.count();
    let branches = remote_branches.saturating_sub(1);

    Ok(vec![
        Row::new("name", name),
        Row::new("branches", branches.to_string()),
        Row::new("tags", tags.to_string()),
    ])
}
