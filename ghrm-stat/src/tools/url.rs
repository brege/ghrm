use crate::tools::git;
use crate::{Context, Row};
use anyhow::Result;

pub fn run(ctx: &Context) -> Result<Vec<Row>> {
    let url = git::output(&ctx.root, &["config", "--get", "remote.origin.url"]).unwrap_or_default();
    Ok(vec![Row::new("url", url)])
}
