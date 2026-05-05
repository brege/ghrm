pub mod tools;

use anyhow::{Context as AnyhowContext, Result};
use clap::ValueEnum;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub root: PathBuf,
    pub sections: Vec<Section>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub tool: Tool,
    pub rows: Vec<Row>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Tool {
    Project,
    Head,
    Pending,
    Languages,
    Size,
}

pub struct Context {
    pub root: PathBuf,
    repo: gix::Repository,
}

impl Tool {
    pub fn default_set() -> &'static [Self] {
        &[
            Self::Project,
            Self::Head,
            Self::Pending,
            Self::Languages,
            Self::Size,
        ]
    }
}

impl Section {
    pub fn new(tool: Tool, rows: Vec<Row>) -> Self {
        Self { tool, rows }
    }
}

impl Row {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

pub fn resolve(input: &Path, tools: &[Tool]) -> Result<Report> {
    let repo = gix::discover(input).context("failed to discover git repository")?;
    let workdir = repo
        .workdir()
        .context("repository is bare or has no working directory")?;
    let root = fs::canonicalize(workdir).context("failed to resolve repository root")?;
    let ctx = Context { root, repo };
    let requested = if tools.is_empty() {
        Tool::default_set()
    } else {
        tools
    };

    let mut sections = Vec::with_capacity(requested.len());
    for tool in requested {
        let rows = match tool {
            Tool::Project => tools::project::run(&ctx)?,
            Tool::Head => tools::head::run(&ctx)?,
            Tool::Pending => tools::pending::run(&ctx)?,
            Tool::Languages => tools::languages::run(&ctx)?,
            Tool::Size => tools::size::run(&ctx)?,
        };
        sections.push(Section::new(*tool, rows));
    }

    Ok(Report {
        root: ctx.root,
        sections,
    })
}

pub fn repo(ctx: &Context) -> &gix::Repository {
    &ctx.repo
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_tool_set_is_non_empty() {
        assert_eq!(Tool::default_set()[0], Tool::Project);
    }
}
