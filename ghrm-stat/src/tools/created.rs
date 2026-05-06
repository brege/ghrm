use crate::tools::history::relative_time;
use crate::{Context, Row, history};
use anyhow::Result;

pub fn run(ctx: &Context) -> Result<Vec<Row>> {
    Ok(vec![Row::new(
        "created",
        relative_time(history(ctx)?.first_commit),
    )])
}
