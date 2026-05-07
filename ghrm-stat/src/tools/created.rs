use crate::tools::history::time_row;
use crate::{Context, Row, history};
use anyhow::Result;

pub fn run(ctx: &Context) -> Result<Vec<Row>> {
    let history = history(ctx)?;
    Ok(vec![time_row("created", history.first_commit)])
}
