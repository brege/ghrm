use crate::{Context, Row, metadata};
use anyhow::Result;

pub fn run(ctx: &Context) -> Result<Vec<Row>> {
    let description = metadata(ctx)
        .ok()
        .and_then(|metadata| metadata.description().map(str::to_string))
        .unwrap_or_default();

    Ok(vec![Row::new("description", description)])
}
