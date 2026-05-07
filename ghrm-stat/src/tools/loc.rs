use crate::{Context, Row};
use anyhow::Result;

pub fn run(ctx: &Context) -> Result<Vec<Row>> {
    Ok(vec![Row::new(
        "linesOfCode",
        crate::language_summary(ctx)?.total.to_string(),
    )])
}
