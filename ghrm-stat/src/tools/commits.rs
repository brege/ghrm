use crate::{Context, Row, history};
use anyhow::Result;

pub fn run(ctx: &Context) -> Result<Vec<Row>> {
    Ok(vec![Row::new("commits", history(ctx)?.commits.to_string())])
}
