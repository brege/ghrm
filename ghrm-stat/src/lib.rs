pub mod tools;

use anyhow::{Context as AnyhowContext, Result, anyhow};
use clap::ValueEnum;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

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
    Title,
    Project,
    Description,
    Head,
    Pending,
    Version,
    Created,
    Languages,
    Dependencies,
    Authors,
    LastChange,
    Url,
    Commits,
    Churn,
    Loc,
    Size,
    License,
}

pub struct Context {
    pub root: PathBuf,
    repo: gix::Repository,
    history: OnceLock<Result<tools::history::History, String>>,
    manifest: OnceLock<Result<tools::manifest::Manifest, String>>,
}

impl Tool {
    pub fn default_set() -> &'static [Self] {
        &[
            Self::Title,
            Self::Project,
            Self::Description,
            Self::Head,
            Self::Pending,
            Self::Version,
            Self::Created,
            Self::Languages,
            Self::Dependencies,
            Self::Authors,
            Self::LastChange,
            Self::Url,
            Self::Commits,
            Self::Churn,
            Self::Loc,
            Self::Size,
            Self::License,
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
    let ctx = Context {
        root,
        repo,
        history: OnceLock::new(),
        manifest: OnceLock::new(),
    };
    let requested = if tools.is_empty() {
        Tool::default_set()
    } else {
        tools
    };

    let mut sections = Vec::with_capacity(requested.len());
    for tool in requested {
        let rows = match tool {
            Tool::Title => tools::title::run(&ctx)?,
            Tool::Project => tools::project::run(&ctx)?,
            Tool::Description => tools::description::run(&ctx)?,
            Tool::Head => tools::head::run(&ctx)?,
            Tool::Pending => tools::pending::run(&ctx)?,
            Tool::Version => tools::version::run(&ctx)?,
            Tool::Created => tools::created::run(&ctx)?,
            Tool::Languages => tools::languages::run(&ctx)?,
            Tool::Dependencies => tools::dependencies::run(&ctx)?,
            Tool::Authors => tools::authors::run(&ctx)?,
            Tool::LastChange => tools::last_change::run(&ctx)?,
            Tool::Url => tools::url::run(&ctx)?,
            Tool::Commits => tools::commits::run(&ctx)?,
            Tool::Churn => tools::churn::run(&ctx)?,
            Tool::Loc => tools::loc::run(&ctx)?,
            Tool::Size => tools::size::run(&ctx)?,
            Tool::License => tools::license::run(&ctx)?,
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

pub fn history(ctx: &Context) -> Result<&tools::history::History> {
    let result = ctx
        .history
        .get_or_init(|| tools::history::load(&ctx.root).map_err(|err| err.to_string()));
    result.as_ref().map_err(|message| anyhow!(message.clone()))
}

pub fn manifest(ctx: &Context) -> Result<&tools::manifest::Manifest> {
    let result = ctx
        .manifest
        .get_or_init(|| tools::manifest::load(&ctx.root).map_err(|err| err.to_string()));
    result.as_ref().map_err(|message| anyhow!(message.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_tool_set_is_non_empty() {
        assert_eq!(Tool::default_set()[0], Tool::Title);
    }
}
