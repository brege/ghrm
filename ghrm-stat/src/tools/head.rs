use crate::{Context, Row, repo};
use anyhow::{Context as _, Result};

pub fn run(ctx: &Context) -> Result<Vec<Row>> {
    let repo = repo(ctx);
    let head_id = repo
        .head_id()
        .context("failed to retrieve head commit")?
        .shorten()?
        .to_string();
    let mut refs = Vec::new();

    if let Some(head_ref) = repo.head_ref()? {
        refs.push(head_ref.name().shorten().to_string());
        if let Some(Ok(remote_ref)) =
            repo.branch_remote_tracking_ref_name(head_ref.name(), gix::remote::Direction::Push)
        {
            refs.push(remote_ref.shorten().to_string());
        }
    }

    Ok(vec![
        Row::new("commit", head_id),
        Row::new("refs", refs.join(", ")),
    ])
}
