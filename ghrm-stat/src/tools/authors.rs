use crate::{Context, Row, history};
use anyhow::Result;

const LIMIT: usize = 3;

pub fn run(ctx: &Context) -> Result<Vec<Row>> {
    let rows = history(ctx)?
        .authors
        .iter()
        .take(LIMIT)
        .map(|author| {
            Row::new(
                &author.name,
                format!("{}% {}", author.contribution, author.commits),
            )
        })
        .collect();
    Ok(rows)
}
