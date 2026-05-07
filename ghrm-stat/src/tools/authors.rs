use crate::{Context, Row, RowMetric, config, history};
use anyhow::Result;

pub fn run(ctx: &Context) -> Result<Vec<Row>> {
    let rows = history(ctx)?
        .authors
        .iter()
        .take(config(ctx).max_authors)
        .map(|author| {
            Row::with_metrics(
                &author.name,
                vec![
                    RowMetric::new("contribution", author.contribution.to_string()),
                    RowMetric::new("commits", author.commits.to_string()),
                ],
            )
        })
        .collect();
    Ok(rows)
}
