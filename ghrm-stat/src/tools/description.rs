use crate::{Context, Row, manifest};
use anyhow::Result;

pub fn run(ctx: &Context) -> Result<Vec<Row>> {
    Ok(vec![Row::new(
        "description",
        manifest(ctx)?.description.clone().unwrap_or_default(),
    )])
}
