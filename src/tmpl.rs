use crate::explorer::column;
use crate::runtime;

use anyhow::Result;
use askama::Template;

#[derive(Template)]
#[template(path = "base.html")]
pub struct PageShell<'a> {
    pub title: &'a str,
    pub body: &'a str,
    pub source: &'a str,
    pub about: &'a str,
    pub show_logout: bool,
    pub asset_json: &'a str,
    pub vendor_styles: &'a [String],
    pub vendor_scripts: &'a [String],
}

#[derive(Template)]
#[template(path = "fragments/about.html")]
pub struct AboutPeek<'a> {
    pub oob: bool,
    pub runtime_paths: &'a [runtime::PathRow],
    pub stats_loaded: bool,
    pub stats: &'a [AboutStatRow],
    pub project_href: &'a str,
    pub project_release_href: &'a str,
    pub project_version: &'static str,
}

pub struct AboutStatRow {
    pub label: String,
    pub value: String,
}

#[derive(Template)]
#[template(path = "page.html")]
pub struct PageCtx<'a> {
    pub features: &'a str,
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
    pub article_class: &'a str,
    pub features: &'a str,
    pub crumbs: &'a str,
    pub current_path: &'a str,
    pub has_parent: bool,
    pub parent_href: &'a str,
    pub filter_menu_active: bool,
    pub filter_controls: &'a [FilterControl],
    pub sort_menu_active: bool,
    pub sort_controls: &'a [SortControl],
    pub sort_dir_control: &'a SortDirControl,
    pub column_menu_active: bool,
    pub column_controls: &'a [ColumnControl],
    pub headers_control: &'a ColumnControl,
    pub column_defs: &'a [column::Def],
    pub show_headers: bool,
    pub empty_cells: &'a [column::Cell],
    pub entries: &'a [ExplorerEntry],
    pub readme: Option<ExplorerReadme<'a>>,
}

pub struct FilterControl {
    pub href: String,
    pub label: String,
    pub title: String,
    pub active: bool,
    pub hidden: bool,
    pub group: bool,
}

pub struct SortControl {
    pub href: String,
    pub label: &'static str,
    pub title: &'static str,
    pub active: bool,
    pub hidden: bool,
}

pub struct SortDirControl {
    pub href: String,
    pub label: &'static str,
    pub icon: &'static str,
    pub active: bool,
}

pub struct ColumnControl {
    pub href: String,
    pub key: &'static str,
    pub label: &'static str,
    pub title: &'static str,
    pub active: bool,
    pub edge: bool,
}

pub struct ExplorerEntry {
    pub name: String,
    pub href: String,
    pub is_dir: bool,
    pub cells: Vec<column::Cell>,
}

pub struct ExplorerReadme<'a> {
    pub name: &'a str,
    pub html: &'a str,
}

#[derive(Template)]
#[template(path = "fragments/search/path.html")]
pub struct PathSearchCtx<'a> {
    pub pending: bool,
    pub rows: &'a [PathSearchRow<'a>],
    pub empty_colspan: usize,
}

pub struct PathSearchRow<'a> {
    pub href: String,
    pub html: String,
    pub is_dir: bool,
    pub cells: &'a [column::Cell],
}

#[derive(Template)]
#[template(path = "fragments/search/content.html")]
pub struct ContentSearchCtx<'a> {
    pub rows: &'a [ContentSearchRow],
    pub truncated: bool,
    pub max_rows: usize,
    pub empty_colspan: usize,
    pub content_colspan: usize,
    pub summary_colspan: usize,
}

pub struct ContentSearchRow {
    pub href: String,
    pub path: String,
    pub line: u64,
    pub html: String,
    pub modified: Option<u64>,
}

pub fn base(p: PageShell) -> Result<String> {
    Ok(p.render()?)
}

pub fn about(p: AboutPeek) -> Result<String> {
    Ok(p.render()?)
}

pub fn page(ctx: PageCtx<'_>) -> Result<String> {
    Ok(ctx.render()?)
}

pub fn explorer(ctx: ExplorerCtx) -> Result<String> {
    Ok(ctx.render()?)
}

pub fn path_search(ctx: PathSearchCtx<'_>) -> Result<String> {
    Ok(ctx.render()?)
}

pub fn content_search(ctx: ContentSearchCtx<'_>) -> Result<String> {
    Ok(ctx.render()?)
}
