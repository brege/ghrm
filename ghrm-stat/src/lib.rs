pub mod tools;

use anyhow::{Context as AnyhowContext, Result, anyhow};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
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
    #[serde(skip_serializing_if = "String::is_empty")]
    pub value: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub metrics: Vec<RowMetric>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RowMetric {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ValueEnum)]
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
    Authors,
    LastChange,
    Url,
    Commits,
    Churn,
    Loc,
    Size,
    License,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct Config {
    pub enabled: bool,
    pub tools: Vec<Tool>,
    pub max_languages: usize,
    pub max_authors: usize,
    pub max_churn: usize,
    pub churn_limit: usize,
    pub include_hidden: bool,
}

pub struct Context {
    pub root: PathBuf,
    config: Config,
    repo: gix::Repository,
    history: OnceLock<Result<tools::history::History, String>>,
    language_summary: OnceLock<Result<tools::languages::Summary, String>>,
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

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            tools: Vec::new(),
            max_languages: 6,
            max_authors: 3,
            max_churn: 3,
            churn_limit: 30,
            include_hidden: false,
        }
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
            metrics: Vec::new(),
        }
    }

    pub fn with_metrics(key: impl Into<String>, metrics: Vec<RowMetric>) -> Self {
        Self {
            key: key.into(),
            value: String::new(),
            metrics,
        }
    }

    fn has_data(&self) -> bool {
        !self.value.is_empty() || self.metrics.iter().any(|metric| !metric.value.is_empty())
    }
}

impl RowMetric {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

pub fn resolve(input: &Path, tools: &[Tool]) -> Result<Report> {
    resolve_with_config(
        input,
        Config {
            tools: tools.to_vec(),
            ..Config::default()
        },
    )
}

pub fn resolve_with_config(input: &Path, config: Config) -> Result<Report> {
    let repo = gix::discover(input).context("failed to discover git repository")?;
    let workdir = repo
        .workdir()
        .context("repository is bare or has no working directory")?;
    let root = fs::canonicalize(workdir).context("failed to resolve repository root")?;
    let ctx = Context {
        root,
        config,
        repo,
        history: OnceLock::new(),
        language_summary: OnceLock::new(),
        manifest: OnceLock::new(),
    };
    if !ctx.config.enabled {
        return Ok(Report {
            root: ctx.root,
            sections: Vec::new(),
        });
    }

    let requested = if ctx.config.tools.is_empty() {
        Tool::default_set()
    } else {
        &ctx.config.tools
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
            Tool::Authors => tools::authors::run(&ctx)?,
            Tool::LastChange => tools::last_change::run(&ctx)?,
            Tool::Url => tools::url::run(&ctx)?,
            Tool::Commits => tools::commits::run(&ctx)?,
            Tool::Churn => tools::churn::run(&ctx)?,
            Tool::Loc => tools::loc::run(&ctx)?,
            Tool::Size => tools::size::run(&ctx)?,
            Tool::License => tools::license::run(&ctx)?,
        };
        let rows = rows.into_iter().filter(Row::has_data).collect::<Vec<_>>();
        if !rows.is_empty() {
            sections.push(Section::new(*tool, rows));
        }
    }

    Ok(Report {
        root: ctx.root,
        sections,
    })
}

pub fn repo(ctx: &Context) -> &gix::Repository {
    &ctx.repo
}

pub fn config(ctx: &Context) -> &Config {
    &ctx.config
}

pub fn history(ctx: &Context) -> Result<&tools::history::History> {
    let result = ctx.history.get_or_init(|| {
        tools::history::load(&ctx.root, ctx.config.churn_limit).map_err(|err| err.to_string())
    });
    result.as_ref().map_err(|message| anyhow!(message.clone()))
}

pub fn language_summary(ctx: &Context) -> Result<&tools::languages::Summary> {
    let result = ctx
        .language_summary
        .get_or_init(|| tools::languages::load(ctx).map_err(|err| err.to_string()));
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
