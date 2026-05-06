use crate::{Context, Row, manifest};
use anyhow::Result;

pub fn run(ctx: &Context) -> Result<Vec<Row>> {
    let manifest = manifest(ctx)?;
    let value = manifest
        .kind
        .as_deref()
        .map(|kind| format!("{} ({kind})", manifest.dependencies))
        .unwrap_or_else(|| manifest.dependencies.to_string());

    Ok(vec![Row::new("dependencies", value)])
}
