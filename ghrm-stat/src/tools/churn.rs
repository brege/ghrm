use crate::tools::history::shorten_path;
use crate::{Context, Row, history};
use anyhow::Result;

const LIMIT: usize = 3;

pub fn run(ctx: &Context) -> Result<Vec<Row>> {
    let rows = history(ctx)?
        .churn
        .iter()
        .take(LIMIT)
        .map(|churn| Row::new(shorten_path(&churn.path, 2), churn.commits.to_string()))
        .collect();
    Ok(rows)
}
