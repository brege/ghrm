use crate::tools::git;
use crate::{Context, Row};
use anyhow::Result;

pub fn run(ctx: &Context) -> Result<Vec<Row>> {
    let user = git::output(&ctx.root, &["config", "user.name"]).unwrap_or_default();
    let version = git::global_output(&["--version"]).unwrap_or_default();

    Ok(vec![Row::new("user", user), Row::new("git", version)])
}
