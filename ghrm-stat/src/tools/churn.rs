use crate::{Context, Row, config, history};
use anyhow::Result;

pub fn run(ctx: &Context) -> Result<Vec<Row>> {
    let rows = history(ctx)?
        .churn
        .iter()
        .take(config(ctx).max_churn)
        .map(|churn| Row::new(churn.path.clone(), churn.commits.to_string()))
        .collect();
    Ok(rows)
}
