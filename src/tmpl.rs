use crate::filter::GroupMeta;

use anyhow::Result;
use askama::Template;

pub(crate) const FAVICON_SVG_URL: &str = "%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%20512%20512%22%20fill%3D%22%232ea043%22%3E%3Cpath%20d%3D%22M240%20216V32H92a12%2012%200%200%200-12%2012v424a12%2012%200%200%200%2012%2012h328a12%2012%200%200%200%2012-12V224H248a8%208%200%200%201-8-8z%22%2F%3E%3Cpath%20d%3D%22M272%2041.69V188a4%204%200%200%200%204%204h146.31a2%202%200%200%200%201.42-3.41L275.41%2040.27a2%202%200%200%200-3.41%201.42z%22%2F%3E%3C%2Fsvg%3E";

#[derive(Template)]
#[template(path = "base.html")]
pub struct PageShell<'a> {
    pub title: &'a str,
    pub body: &'a str,
    pub source: &'a str,
    pub favicon: &'static str,
    pub show_logout: bool,
    pub default_show_hidden: bool,
    pub default_show_excludes: bool,
    pub default_use_ignore: bool,
    pub default_filter_ext: bool,
    pub default_filter_group: Option<&'a str>,
    pub default_sort: &'a str,
    pub default_show_date: bool,
    pub default_show_commit: bool,
    pub default_show_commit_date: bool,
    pub can_toggle_excludes: bool,
    pub has_mermaid: bool,
    pub has_math: bool,
    pub has_map: bool,
}

#[derive(Template)]
#[template(path = "page.html")]
pub struct PageCtx<'a> {
    pub crumbs: &'a str,
    pub preview_html: &'a str,
    pub raw_html: &'a str,
    pub view_attrs: &'a str,
    pub preview_hidden: bool,
    pub raw_hidden: bool,
}

#[derive(Template)]
#[template(path = "explorer.html")]
pub struct ExplorerCtx<'a> {
    pub crumbs: &'a str,
    pub current_path: &'a str,
    pub has_parent: bool,
    pub parent_href: &'a str,
    pub show_excludes: bool,
    pub show_date: bool,
    pub show_commit: bool,
    pub show_commit_date: bool,
    pub filter_groups: &'a [GroupMeta],
    pub entries: &'a [ExplorerEntry],
    pub readme: Option<ExplorerReadme<'a>>,
}

pub struct ExplorerEntry {
    pub name: String,
    pub href: String,
    pub is_dir: bool,
    pub modified: Option<u64>,
    pub commit_message: Option<String>,
    pub commit_date: Option<u64>,
}

pub struct ExplorerReadme<'a> {
    pub name: &'a str,
    pub html: &'a str,
}

pub fn base(p: PageShell) -> Result<String> {
    Ok(p.render()?)
}

pub fn page(ctx: PageCtx<'_>) -> Result<String> {
    Ok(ctx.render()?)
}

pub fn explorer(ctx: ExplorerCtx) -> Result<String> {
    Ok(ctx.render()?)
}
