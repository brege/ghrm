use crate::{Context, Row, config, history};
use anyhow::Result;

pub fn run(ctx: &Context) -> Result<Vec<Row>> {
    let rows = history(ctx)?
        .authors
        .iter()
        .take(config(ctx).max_authors)
        .map(|author| {
            Row::new(
                &author.name,
                format!("{}% {}", author.contribution, author.commits),
            )
        })
        .collect();
    Ok(rows)
}
