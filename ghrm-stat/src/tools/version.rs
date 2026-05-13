use crate::{Context, Row, metadata, repo};
use anyhow::Result;
use gix::Repository;

pub fn run(ctx: &Context) -> Result<Vec<Row>> {
    let version = latest_tag(repo(ctx))
        .unwrap_or_default()
        .or_else(|| {
            metadata(ctx)
                .ok()
                .and_then(|metadata| metadata.version().map(str::to_string))
        })
        .unwrap_or_default();

    Ok(vec![Row::new("version", version)])
}

fn latest_tag(repo: &Repository) -> Result<Option<String>> {
    let mut version = None;
    let mut most_recent = 0;

    for tag in repo.references()?.tags()?.peeled()?.filter_map(Result::ok) {
        if let Ok(commit) = tag.id().object()?.try_into_commit() {
            let current_time = commit.time()?.seconds;
            if current_time > most_recent {
                most_recent = current_time;
                version = Some(tag.name().shorten().to_string());
            }
        }
    }

    Ok(version)
}
