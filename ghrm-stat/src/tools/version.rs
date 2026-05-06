use crate::{Context, Row, manifest};
use anyhow::Result;

pub fn run(ctx: &Context) -> Result<Vec<Row>> {
    let version = manifest(ctx)?
        .version
        .as_deref()
        .map(|value| {
            if value.starts_with('v') {
                value.to_string()
            } else {
                format!("v{value}")
            }
        })
        .unwrap_or_default();

    Ok(vec![Row::new("version", version)])
}
