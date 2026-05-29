use crate::explorer::view::ViewQuery;
use crate::explorer::{view, walk};
use crate::http::server::{AppState, Mode};
use crate::paths;
use crate::repo::SourceState;
use crate::runtime;
use crate::tmpl::{
    self, AboutDetailRow, AboutDetails, AboutFilterRow, AboutLanguage, AboutPeek, AboutStatItem,
    AboutStatMetric, AboutStatPart, AboutStatRow, AboutStats,
};

use anyhow::{Context, Result};
use axum::{
    body::Body,
    extract::{Query, RawQuery, State},
    http::{StatusCode, header},
    response::Response,
};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tracing::warn;

const PROJECT_URL: &str = "https://github.com/brege/ghrm";
const LANGUAGE_COLORS: &[&str] = &[
    "#d19a66", "#8b5cf6", "#f1e05a", "#e34c26", "#3572a5", "#000080",
];

struct StatDisplay {
    label: &'static str,
    icon: &'static str,
    is_list: bool,
    has_timestamp: bool,
}

const STAT_DISPLAYS: &[(ghrm_stat::Tool, StatDisplay)] = &[
    (
        ghrm_stat::Tool::Title,
        StatDisplay {
            label: "Title",
            icon: "",
            is_list: false,
            has_timestamp: false,
        },
    ),
    (
        ghrm_stat::Tool::Project,
        StatDisplay {
            label: "Project",
            icon: "ghrm-icon-table",
            is_list: false,
            has_timestamp: false,
        },
    ),
    (
        ghrm_stat::Tool::Description,
        StatDisplay {
            label: "Description",
            icon: "",
            is_list: false,
            has_timestamp: false,
        },
    ),
    (
        ghrm_stat::Tool::Head,
        StatDisplay {
            label: "Head",
            icon: "ghrm-icon-location",
            is_list: false,
            has_timestamp: false,
        },
    ),
    (
        ghrm_stat::Tool::Pending,
        StatDisplay {
            label: "Pending",
            icon: "",
            is_list: false,
            has_timestamp: false,
        },
    ),
    (
        ghrm_stat::Tool::Version,
        StatDisplay {
            label: "Version",
            icon: "ghrm-icon-fork",
            is_list: false,
            has_timestamp: false,
        },
    ),
    (
        ghrm_stat::Tool::Created,
        StatDisplay {
            label: "Created",
            icon: "ghrm-icon-created",
            is_list: false,
            has_timestamp: true,
        },
    ),
    (
        ghrm_stat::Tool::Languages,
        StatDisplay {
            label: "Languages",
            icon: "",
            is_list: false,
            has_timestamp: false,
        },
    ),
    (
        ghrm_stat::Tool::Authors,
        StatDisplay {
            label: "Authors",
            icon: "ghrm-icon-people",
            is_list: true,
            has_timestamp: false,
        },
    ),
    (
        ghrm_stat::Tool::LastChange,
        StatDisplay {
            label: "Updated",
            icon: "ghrm-icon-update",
            is_list: false,
            has_timestamp: true,
        },
    ),
    (
        ghrm_stat::Tool::Url,
        StatDisplay {
            label: "URL",
            icon: "",
            is_list: false,
            has_timestamp: false,
        },
    ),
    (
        ghrm_stat::Tool::Commits,
        StatDisplay {
            label: "Commits",
            icon: "ghrm-icon-commit",
            is_list: false,
            has_timestamp: false,
        },
    ),
    (
        ghrm_stat::Tool::Churn,
        StatDisplay {
            label: "Churn",
            icon: "ghrm-icon-repeat",
            is_list: true,
            has_timestamp: false,
        },
    ),
    (
        ghrm_stat::Tool::Loc,
        StatDisplay {
            label: "LOC",
            icon: "ghrm-icon-loc",
            is_list: false,
            has_timestamp: false,
        },
    ),
    (
        ghrm_stat::Tool::Size,
        StatDisplay {
            label: "Size",
            icon: "ghrm-icon-data",
            is_list: false,
            has_timestamp: false,
        },
    ),
    (
        ghrm_stat::Tool::License,
        StatDisplay {
            label: "License",
            icon: "ghrm-icon-scale",
            is_list: false,
            has_timestamp: false,
        },
    ),
];

fn stat_display(tool: ghrm_stat::Tool) -> &'static StatDisplay {
    STAT_DISPLAYS
        .iter()
        .find(|(t, _)| *t == tool)
        .map(|(_, d)| d)
        .expect("all tools must have display definitions")
}

#[derive(Default, Deserialize)]
pub(crate) struct AboutQuery {
    path: Option<String>,
    #[serde(flatten)]
    view: ViewQuery,
}

pub(crate) async fn show(
    State(s): State<AppState>,
    RawQuery(raw_query): RawQuery,
    Query(q): Query<AboutQuery>,
) -> Response {
    match show_inner(s, raw_query, q).await {
        Ok(response) => response,
        Err(_) => server_error(),
    }
}

async fn show_inner(s: AppState, raw_query: Option<String>, q: AboutQuery) -> Result<Response> {
    let stats_path = about_path(&s, q.path.as_deref());
    let view = view::from_query(&q.view, raw_query.as_deref(), &s.view_cfg, &s.filters);
    let source = s.repos.source_for(&stats_path);
    let stats_input = stats_input_path(&stats_path);
    let served_root = served_root(&s);
    let stats_cfg = s.stats.clone();
    let stats_source = source.clone();
    let stats_input_for_repo = stats_input.clone();
    let stats = if stats_cfg.enabled && source != SourceState::NoRepo {
        match tokio::task::spawn_blocking(move || {
            ghrm_stat::resolve_with_config(&stats_input_for_repo, stats_cfg)
                .map(|report| stats_model(report, &stats_source, &served_root))
        })
        .await
        {
            Ok(Ok(stats)) => stats,
            Ok(Err(e)) => {
                warn!(
                    "repository stats failed for {}: {e:#}",
                    stats_input.display()
                );
                AboutStats::default()
            }
            Err(e) => {
                warn!(
                    "repository stats task failed for {}: {e}",
                    stats_input.display()
                );
                AboutStats::default()
            }
        }
    } else {
        AboutStats::default()
    };
    let local_path = if source == SourceState::NoRepo {
        stats_path.display().to_string()
    } else {
        String::new()
    };
    let details = details_model(&s, &stats_input, &view, &source, raw_query.as_deref()).await?;

    Ok(html_response(&html_with_details(
        &details,
        &stats,
        true,
        &local_path,
    )))
}

async fn details_model(
    s: &AppState,
    path: &Path,
    view: &view::ViewState,
    source: &SourceState,
    raw_query: Option<&str>,
) -> Result<AboutDetails> {
    let mut details = runtime_details(&s.runtime_paths, source, raw_query);
    let fs_config = fs_config(s, view);
    let path = path.to_path_buf();
    let display_path = path.display().to_string();
    let fs_report =
        tokio::task::spawn_blocking(move || ghrm_stat::filesystem::scan(&path, &fs_config))
            .await
            .context("join filesystem stats task")?
            .with_context(|| format!("scan filesystem stats for {display_path}"))?;

    details.scope = scope_rows(s, &fs_report);
    details.directory = directory_rows(&fs_report);
    details.filters = filter_rows(&fs_report);
    Ok(details)
}

fn fs_config(s: &AppState, view: &view::ViewState) -> ghrm_stat::filesystem::FsConfig {
    ghrm_stat::filesystem::FsConfig {
        hidden: view.opts.show_hidden,
        use_ignore: view.use_ignore,
        show_excludes: view.opts.show_excludes,
        exclude_names: s.exclude_names.clone(),
        same_file_system: true,
        filter_groups: s
            .filters
            .groups()
            .iter()
            .filter_map(|group| {
                s.filters.group_globs(&group.name).map(|globs| {
                    ghrm_stat::filesystem::FsFilterGroup {
                        name: group.name.clone(),
                        label: group.label.clone(),
                        globs: globs.to_vec(),
                        default_enabled: s.view_cfg.default_groups.contains(&group.name),
                    }
                })
            })
            .collect(),
    }
}

pub(crate) fn html(
    runtime_paths: &runtime::Paths,
    stats: &AboutStats,
    stats_loaded: bool,
) -> String {
    let details = runtime_details(runtime_paths, &SourceState::NoRepo, None);
    html_with_details(&details, stats, stats_loaded, "")
}

fn html_with_details(
    details: &AboutDetails,
    stats: &AboutStats,
    stats_loaded: bool,
    local_path: &str,
) -> String {
    let project_version = env!("CARGO_PKG_VERSION");
    let project_release_href = format!("{PROJECT_URL}/releases/tag/v{project_version}");
    let about = AboutPeek {
        details,
        stats_loaded,
        stats,
        local_path,
        project_href: PROJECT_URL,
        project_release_href: &project_release_href,
        project_version,
    };
    match tmpl::about(about) {
        Ok(html) => html,
        Err(e) => {
            warn!("about template error: {}", e);
            String::new()
        }
    }
}

fn runtime_details(
    runtime_paths: &runtime::Paths,
    source: &SourceState,
    raw_query: Option<&str>,
) -> AboutDetails {
    let mut details = AboutDetails::default();
    for row in runtime_paths.rows() {
        match row.label {
            "local" | "network" => {
                details
                    .network
                    .push(detail_link(row.label, row.value.clone(), row.value.clone()))
            }
            "listen" => details
                .network
                .push(detail_row(row.label, row.value.clone())),
            _ => details.paths.push(detail_row(row.label, row.value.clone())),
        }
    }
    if let Some(row) = source_row(source) {
        details.network.insert(0, row);
    }
    if let Some(query) = raw_query {
        let href = format!("/_ghrm/about?{query}");
        details
            .network
            .push(detail_link("about", href.clone(), href));
    }
    details
}

fn source_row(source: &SourceState) -> Option<AboutDetailRow> {
    match source {
        SourceState::Web { url, .. } => Some(detail_link("source", url.clone(), url.clone())),
        SourceState::Transport { raw } => Some(detail_row("source", raw.clone())),
        SourceState::NoRemote => Some(detail_row("source", "git repo / no remote")),
        SourceState::NoRepo => None,
    }
}

fn scope_rows(s: &AppState, report: &ghrm_stat::filesystem::FsReport) -> Vec<AboutDetailRow> {
    vec![
        detail_row("directory", display_fs_path(s, &report.root)),
        detail_row("relative to", served_root(s).display().to_string()),
    ]
}

fn directory_rows(report: &ghrm_stat::filesystem::FsReport) -> Vec<AboutDetailRow> {
    vec![
        detail_row("Visible files", report.totals.files.to_string()),
        detail_row("Directories", report.totals.dirs.to_string()),
        detail_row("Symlinks", report.totals.symlinks.to_string()),
        detail_row(
            "Visible size",
            ghrm_stat::filesystem::format_bytes(report.totals.bytes),
        ),
        detail_row("Depth", level_value(report.max_depth)),
        detail_row(
            "File system",
            report
                .file_system
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
        ),
    ]
}

fn filter_rows(report: &ghrm_stat::filesystem::FsReport) -> Vec<AboutFilterRow> {
    let mut rows = vec![filter_row("All", false, &report.totals)];
    rows.extend(
        report
            .filters
            .iter()
            .map(|filter| filter_row(filter.label.clone(), filter.default_enabled, &filter.totals)),
    );
    rows
}

fn detail_row(label: impl Into<String>, value: impl Into<String>) -> AboutDetailRow {
    let value = value.into();
    AboutDetailRow {
        label: label.into(),
        title: value.clone(),
        value,
        href: String::new(),
    }
}

fn detail_link(
    label: impl Into<String>,
    value: impl Into<String>,
    href: impl Into<String>,
) -> AboutDetailRow {
    let value = value.into();
    AboutDetailRow {
        label: label.into(),
        title: value.clone(),
        value,
        href: href.into(),
    }
}

fn filter_row(
    label: impl Into<String>,
    default_enabled: bool,
    totals: &ghrm_stat::filesystem::FsTotals,
) -> AboutFilterRow {
    AboutFilterRow {
        label: label.into(),
        files: totals.files.to_string(),
        dirs: totals.dirs.to_string(),
        size: ghrm_stat::filesystem::format_bytes(totals.bytes),
        default_enabled,
    }
}

fn display_fs_path(s: &AppState, path: &Path) -> String {
    let root = served_root(s);
    path.strip_prefix(root)
        .ok()
        .filter(|rel| !rel.as_os_str().is_empty())
        .map(|rel| rel.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| ".".to_string())
}

fn level_value(depth: usize) -> String {
    format!("{} {}", depth, plural_usize(depth, "level", "levels"))
}

fn plural_usize(value: usize, single: &'static str, multiple: &'static str) -> &'static str {
    if value == 1 { single } else { multiple }
}

fn about_path(s: &AppState, raw_path: Option<&str>) -> PathBuf {
    let rel = raw_path.and_then(paths::safe_rel);
    match s.mode {
        Mode::File => {
            let base = s.target.parent().unwrap_or(&s.target);
            rel.map(|rel| base.join(rel))
                .unwrap_or_else(|| s.target.clone())
        }
        Mode::Dir => rel
            .map(|rel| s.target.join(rel))
            .unwrap_or_else(|| s.target.clone()),
    }
}

fn stats_input_path(path: &std::path::Path) -> PathBuf {
    if path.is_file() {
        return path.parent().unwrap_or(path).to_path_buf();
    }
    path.to_path_buf()
}

fn served_root(s: &AppState) -> PathBuf {
    match s.mode {
        Mode::File => s.target.parent().unwrap_or(&s.target).to_path_buf(),
        Mode::Dir => s.target.clone(),
    }
}

fn html_response(html: &str) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from(html.to_string()))
        .unwrap()
}

fn server_error() -> Response {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from("500"))
        .unwrap()
}

fn stats_model(report: ghrm_stat::Report, source: &SourceState, served_root: &Path) -> AboutStats {
    let mut about = AboutStats::default();
    let repo_root = report.root.clone();
    let has_languages = report
        .sections
        .iter()
        .any(|section| section.tool == ghrm_stat::Tool::Languages && !section.rows.is_empty());
    for section in report.sections {
        match section.tool {
            ghrm_stat::Tool::Languages => {
                let (languages, total) = language_rows(&section.rows);
                about.languages = languages;
                about.language_total = total;
            }
            ghrm_stat::Tool::Loc if has_languages => {}
            ghrm_stat::Tool::Project
            | ghrm_stat::Tool::Version
            | ghrm_stat::Tool::License
            | ghrm_stat::Tool::Url
            | ghrm_stat::Tool::Title
            | ghrm_stat::Tool::Description => {
                if let Some(row) = stat_row(section, source, &repo_root, served_root) {
                    about.metadata.push(row);
                }
            }
            ghrm_stat::Tool::Head | ghrm_stat::Tool::Created | ghrm_stat::Tool::LastChange => {
                if let Some(row) = stat_row(section, source, &repo_root, served_root) {
                    about.history.push(row);
                }
            }
            _ => {
                if let Some(row) = stat_row(section, source, &repo_root, served_root) {
                    about.activity.push(row);
                }
            }
        }
    }
    about
}

fn language_rows(rows: &[ghrm_stat::Row]) -> (Vec<AboutLanguage>, String) {
    let counts = rows
        .iter()
        .filter(|row| row.key != "total")
        .filter_map(|row| row.value.parse::<usize>().ok().map(|lines| (row, lines)))
        .collect::<Vec<_>>();
    let total = row_value(rows, "total")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or_else(|| counts.iter().map(|(_, lines)| lines).sum::<usize>());
    let languages = counts
        .iter()
        .enumerate()
        .map(|(i, (row, lines))| {
            let color = LANGUAGE_COLORS[i % LANGUAGE_COLORS.len()].to_string();
            let percent = if total == 0 {
                0.0
            } else {
                *lines as f64 / total as f64 * 100.0
            };
            let value = format!("{percent:.1}%");
            let lines = lines.to_string();
            AboutLanguage {
                name: row.key.clone(),
                value: value.clone(),
                lines: lines.clone(),
                style: format!("--ghrm-lang-color: {color}; width: {value}"),
                title: format!("{}: {lines} lines of code, {value}", row.key),
                color,
            }
        })
        .collect();
    (languages, total.to_string())
}

fn stat_row(
    section: ghrm_stat::Section,
    source: &SourceState,
    repo_root: &Path,
    served_root: &Path,
) -> Option<AboutStatRow> {
    let label = stat_title(section.tool);
    let items = stat_items(section.tool, &section.rows, repo_root, served_root);
    let parts = stat_parts(section.tool, &section.rows);
    let value = parts_text(&parts)
        .or_else(|| stat_value(section.tool, &section.rows))
        .unwrap_or_default();
    if value.is_empty() && parts.is_empty() && items.is_empty() {
        return None;
    }
    let icon = stat_icon(section.tool, &value);
    let href = stat_href(section.tool, &value, source);
    let title = stat_title_attr(section.tool, &section.rows, &parts);
    let title_ts = stat_title_ts(section.tool, &section.rows);
    Some(AboutStatRow {
        label: label.to_string(),
        value,
        title,
        title_ts,
        parts,
        icon,
        href,
        items,
    })
}

fn stat_value(tool: ghrm_stat::Tool, rows: &[ghrm_stat::Row]) -> Option<String> {
    match tool {
        ghrm_stat::Tool::Project | ghrm_stat::Tool::Head => None,
        ghrm_stat::Tool::Pending => pending_value(rows),
        ghrm_stat::Tool::Languages | ghrm_stat::Tool::Authors | ghrm_stat::Tool::Churn => None,
        ghrm_stat::Tool::Size => size_value(rows),
        ghrm_stat::Tool::Loc => row_value(rows, "linesOfCode").map(str::to_string),
        ghrm_stat::Tool::LastChange => row_value(rows, "lastChange").map(str::to_string),
        _ => compact_value(rows),
    }
}

fn stat_title_attr(
    tool: ghrm_stat::Tool,
    rows: &[ghrm_stat::Row],
    parts: &[AboutStatPart],
) -> String {
    match tool {
        ghrm_stat::Tool::Project if !parts.is_empty() => "project / branches / tags".to_string(),
        ghrm_stat::Tool::Head if parts.len() > 1 => "commit hash / refs".to_string(),
        ghrm_stat::Tool::Head if !parts.is_empty() => "commit hash".to_string(),
        ghrm_stat::Tool::Commits => row_value(rows, "commits")
            .map(|commits| format!("{commits} commits"))
            .unwrap_or_default(),
        ghrm_stat::Tool::Loc => row_value(rows, "linesOfCode")
            .map(|lines| format!("{lines} lines of code"))
            .unwrap_or_default(),
        _ => String::new(),
    }
}

fn stat_title_ts(tool: ghrm_stat::Tool, rows: &[ghrm_stat::Row]) -> Option<u64> {
    if !stat_display(tool).has_timestamp {
        return None;
    }
    rows.first()
        .and_then(|row| row_metric(row, "timestamp"))
        .and_then(|value| value.parse().ok())
}

fn stat_parts(tool: ghrm_stat::Tool, rows: &[ghrm_stat::Row]) -> Vec<AboutStatPart> {
    match tool {
        ghrm_stat::Tool::Project => project_parts(rows),
        ghrm_stat::Tool::Head => head_parts(rows),
        _ => Vec::new(),
    }
}

fn project_parts(rows: &[ghrm_stat::Row]) -> Vec<AboutStatPart> {
    let Some(name) = row_value(rows, "name") else {
        return Vec::new();
    };
    let branches = row_value(rows, "branches").unwrap_or("0");
    let tags = row_value(rows, "tags").unwrap_or("0");
    vec![
        stat_part(name, false),
        stat_part(
            &format!("{branches} {}", plural(branches, "branch", "branches")),
            true,
        ),
        stat_part(&format!("{tags} {}", plural(tags, "tag", "tags")), true),
    ]
}

fn head_parts(rows: &[ghrm_stat::Row]) -> Vec<AboutStatPart> {
    let Some(commit) = row_value(rows, "commit") else {
        return Vec::new();
    };
    let mut parts = vec![stat_part(commit, false)];
    if let Some(refs) = row_value(rows, "refs") {
        parts.push(stat_part(refs, true));
    }
    parts
}

fn stat_part(value: &str, separator: bool) -> AboutStatPart {
    AboutStatPart {
        value: value.to_string(),
        separator,
    }
}

fn parts_text(parts: &[AboutStatPart]) -> Option<String> {
    if parts.is_empty() {
        return None;
    }
    Some(
        parts
            .iter()
            .map(|part| part.value.as_str())
            .collect::<Vec<_>>()
            .join(" / "),
    )
}

fn pending_value(rows: &[ghrm_stat::Row]) -> Option<String> {
    let added = row_value(rows, "added").unwrap_or("0");
    let deleted = row_value(rows, "deleted").unwrap_or("0");
    let modified = row_value(rows, "modified").unwrap_or("0");
    if added == "0" && deleted == "0" && modified == "0" {
        return Some("clean".to_string());
    }
    Some(format!(
        "{added} added, {deleted} deleted, {modified} modified"
    ))
}

fn size_value(rows: &[ghrm_stat::Row]) -> Option<String> {
    let size = row_value(rows, "size")?;
    match row_value(rows, "files") {
        Some(files) => Some(format!("{size} ({files} files)")),
        None => Some(size.to_string()),
    }
}

fn stat_items(
    tool: ghrm_stat::Tool,
    rows: &[ghrm_stat::Row],
    repo_root: &Path,
    served_root: &Path,
) -> Vec<AboutStatItem> {
    if !stat_display(tool).is_list {
        return Vec::new();
    }
    rows.iter()
        .map(|row| AboutStatItem {
            label: row.key.clone(),
            value: row.value.clone(),
            href: stat_item_href(tool, row, repo_root, served_root),
            metrics: stat_item_metrics(tool, row),
        })
        .collect()
}

fn stat_item_metrics(tool: ghrm_stat::Tool, row: &ghrm_stat::Row) -> Vec<AboutStatMetric> {
    match tool {
        ghrm_stat::Tool::Authors => author_metrics(row),
        ghrm_stat::Tool::Churn => vec![AboutStatMetric {
            value: row.value.clone(),
            label: String::new(),
            title: format!("{} commits", row.value),
        }],
        _ => Vec::new(),
    }
}

fn author_metrics(row: &ghrm_stat::Row) -> Vec<AboutStatMetric> {
    let contribution = row_metric(row, "contribution");
    let commits = row_metric(row, "commits");
    let mut out = Vec::new();
    if let Some(commits) = commits {
        out.push(AboutStatMetric {
            value: commits.to_string(),
            label: String::new(),
            title: format!("{commits} commits"),
        });
    }
    if let Some(contribution) = contribution {
        out.push(AboutStatMetric {
            value: format!("{contribution}%"),
            label: String::new(),
            title: format!("{contribution}% of commits"),
        });
    }
    out
}

fn stat_item_href(
    tool: ghrm_stat::Tool,
    row: &ghrm_stat::Row,
    repo_root: &Path,
    served_root: &Path,
) -> String {
    if !matches!(tool, ghrm_stat::Tool::Churn) {
        return String::new();
    }
    let path = repo_root.join(&row.key);
    if !path.is_file() {
        return String::new();
    }
    path.strip_prefix(served_root)
        .ok()
        .filter(|rel| !rel.as_os_str().is_empty())
        .map(walk::file_href)
        .unwrap_or_default()
}

fn compact_value(rows: &[ghrm_stat::Row]) -> Option<String> {
    match rows {
        [row] => Some(row.value.clone()),
        _ => None,
    }
}

fn row_value<'a>(rows: &'a [ghrm_stat::Row], key: &str) -> Option<&'a str> {
    rows.iter()
        .find(|row| row.key == key)
        .map(|row| row.value.as_str())
}

fn row_metric<'a>(row: &'a ghrm_stat::Row, key: &str) -> Option<&'a str> {
    row.metrics
        .iter()
        .find(|metric| metric.key == key)
        .map(|metric| metric.value.as_str())
}

fn plural(value: &str, single: &'static str, multiple: &'static str) -> &'static str {
    if value == "1" { single } else { multiple }
}

fn stat_title(tool: ghrm_stat::Tool) -> &'static str {
    stat_display(tool).label
}

fn stat_icon(tool: ghrm_stat::Tool, value: &str) -> &'static str {
    if tool == ghrm_stat::Tool::Url {
        return forge_icon(value);
    }
    stat_display(tool).icon
}

fn stat_href(tool: ghrm_stat::Tool, value: &str, source: &SourceState) -> String {
    if !matches!(tool, ghrm_stat::Tool::Url) {
        return String::new();
    }
    match source {
        SourceState::Web { url, .. } => url.clone(),
        _ if value.starts_with("https://") || value.starts_with("http://") => value.to_string(),
        _ => String::new(),
    }
}

fn forge_icon(value: &str) -> &'static str {
    if value.contains("github.com") {
        return "ghrm-icon-github";
    }
    if value.contains("gitlab") {
        return "ghrm-icon-gitlab";
    }
    if value.contains("bitbucket") {
        return "ghrm-icon-bitbucket";
    }
    if value.contains("codeberg.org") {
        return "ghrm-icon-codeberg";
    }
    if value.contains("sourcehut") || value.contains("sr.ht") {
        return "ghrm-icon-sourcehut";
    }
    "ghrm-icon-git"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;
    use std::fs;

    fn test_runtime_paths() -> runtime::Paths {
        let td = TempDir::new("ghrm-about-runtime-paths");
        runtime::Paths::new(td.path(), None).unwrap()
    }

    fn about_row<'a>(rows: &'a [AboutStatRow], label: &str) -> &'a AboutStatRow {
        rows.iter().find(|row| row.label == label).unwrap()
    }

    fn part_values(parts: &[AboutStatPart]) -> Vec<&str> {
        parts.iter().map(|part| part.value.as_str()).collect()
    }

    fn metric_values(item: &AboutStatItem) -> Vec<&str> {
        item.metrics
            .iter()
            .map(|metric| metric.value.as_str())
            .collect()
    }

    fn metric_titles(item: &AboutStatItem) -> Vec<&str> {
        item.metrics
            .iter()
            .map(|metric| metric.title.as_str())
            .collect()
    }

    #[test]
    fn display_table_covers_all_tools() {
        use clap::ValueEnum;
        for tool in ghrm_stat::Tool::value_variants() {
            let display = stat_display(*tool);
            assert!(
                !display.label.is_empty(),
                "{tool:?} must have a non-empty label"
            );
        }
    }

    #[test]
    fn about_html_renders_runtime_and_app_links() {
        let runtime_paths = test_runtime_paths();
        let stats = AboutStats::default();
        let html = html(&runtime_paths, &stats, false);

        assert!(html.contains("Paths"));
        assert!(html.contains("href=\"https://github.com/brege/ghrm\""));
        assert!(html.contains(">brege/ghrm</span>"));
        assert!(html.contains("data-stats-loaded=\"false\""));
        assert!(html.contains("id=\"ghrm-about-panel-menu-toggle\""));
        assert!(html.contains("data-ghrm-about-panel-option=\"paths\""));
        assert!(html.contains("data-ghrm-about-panel=\"paths\""));
    }

    #[test]
    fn about_html_omits_empty_summary() {
        let runtime_paths = test_runtime_paths();
        let stats = AboutStats::default();
        let html = html(&runtime_paths, &stats, true);

        assert!(html.contains("class=\"ghrm-about-peek\""));
        assert!(!html.contains("data-details-only"));
        assert!(!html.contains("ghrm-about-summary"));
        assert!(html.contains("ghrm-about-stamp-shell"));
        assert!(html.contains("ghrm-about-stamp-button"));
    }

    #[test]
    fn about_html_renders_summary_when_stats_exist() {
        let runtime_paths = test_runtime_paths();
        let mut stats = AboutStats::default();
        stats.metadata.push(AboutStatRow {
            label: "Project".to_string(),
            value: "ghrm".to_string(),
            title: String::new(),
            title_ts: None,
            parts: Vec::new(),
            icon: "",
            href: String::new(),
            items: Vec::new(),
        });
        let html = html(&runtime_paths, &stats, true);

        assert!(html.contains("class=\"ghrm-about-peek\""));
        assert!(!html.contains("data-details-only"));
        assert!(html.contains("ghrm-about-summary"));
        assert!(html.contains("ghrm-about-stamp-button"));
    }

    #[test]
    fn about_html_renders_local_path_above_stamp_shell() {
        let runtime_paths = test_runtime_paths();
        let stats = AboutStats::default();
        let details = runtime_details(&runtime_paths, &SourceState::NoRepo, None);
        let html = html_with_details(&details, &stats, true, "/tmp/local");

        let path = html.find("/tmp/local").unwrap();
        let stamp = html.find("ghrm-about-stamp-shell").unwrap();
        assert!(html.contains("not a git repo"));
        assert!(path < stamp);
    }

    #[test]
    fn runtime_details_split_paths_and_network_without_dropping_listen() {
        let td = TempDir::new("ghrm-about-runtime-split");
        let config = td.path().join("config.toml");
        let runtime_paths = runtime::Paths::new(td.path(), Some(&config))
            .unwrap()
            .with_server(
                "127.0.0.1:1334".parse().unwrap(),
                "http://localhost:1334/".to_string(),
                Some("http://192.168.0.25:1334/".to_string()),
            );
        let details = runtime_details(&runtime_paths, &SourceState::NoRepo, Some("path=src"));

        assert!(details.paths.iter().any(|row| row.label == "root"));
        assert!(details.paths.iter().any(|row| row.label == "config"));
        assert!(details.paths.iter().any(|row| row.label == "assets"));
        assert!(details.paths.iter().any(|row| row.label == "vendor"));
        assert!(details.network.iter().any(|row| row.label == "listen"));
        assert!(details.network.iter().any(|row| row.label == "local"));
        assert!(details.network.iter().any(|row| row.label == "network"));
        assert!(details.network.iter().any(|row| row.label == "about"));
    }

    #[test]
    fn about_html_omits_current_source() {
        let runtime_paths = test_runtime_paths();
        let stats = AboutStats::default();
        let html = html(&runtime_paths, &stats, false);

        assert!(!html.contains("Current Source"));
    }

    #[test]
    fn stats_input_path_uses_parent_for_files() {
        let td = TempDir::new("ghrm-about-stats-input");
        let file = td.path().join("README.md");
        fs::write(&file, "# title\n").unwrap();

        assert_eq!(stats_input_path(&file), td.path());
    }

    #[test]
    fn stats_model_structures_scalar_rows() {
        let mut created = ghrm_stat::Row::new("created", "3 years ago");
        created
            .metrics
            .push(ghrm_stat::RowMetric::new("timestamp", "10"));
        let mut last_change = ghrm_stat::Row::new("lastChange", "2 days ago");
        last_change
            .metrics
            .push(ghrm_stat::RowMetric::new("timestamp", "1700000000"));
        let report = ghrm_stat::Report {
            root: PathBuf::from("/tmp/repo"),
            sections: vec![
                ghrm_stat::Section::new(
                    ghrm_stat::Tool::Project,
                    vec![
                        ghrm_stat::Row::new("name", "ghrm"),
                        ghrm_stat::Row::new("branches", "1"),
                        ghrm_stat::Row::new("tags", "7"),
                    ],
                ),
                ghrm_stat::Section::new(
                    ghrm_stat::Tool::Head,
                    vec![
                        ghrm_stat::Row::new("commit", "10314cff"),
                        ghrm_stat::Row::new("refs", "main, origin/main"),
                    ],
                ),
                ghrm_stat::Section::new(ghrm_stat::Tool::Created, vec![created]),
                ghrm_stat::Section::new(ghrm_stat::Tool::LastChange, vec![last_change]),
                ghrm_stat::Section::new(
                    ghrm_stat::Tool::Languages,
                    vec![
                        ghrm_stat::Row::new("Rust", "6"),
                        ghrm_stat::Row::new("CSS", "4"),
                    ],
                ),
            ],
        };
        let stats = stats_model(report, &SourceState::NoRepo, Path::new("/tmp/repo"));

        let project = about_row(&stats.metadata, "Project");
        assert_eq!(
            part_values(&project.parts),
            vec!["ghrm", "1 branch", "7 tags"]
        );
        assert_eq!(project.title, "project / branches / tags");

        let head = about_row(&stats.history, "Head");
        assert_eq!(head.value, "10314cff / main, origin/main");
        assert_eq!(
            part_values(&head.parts),
            vec!["10314cff", "main, origin/main"]
        );
        assert_eq!(head.title, "commit hash / refs");

        let created = about_row(&stats.history, "Created");
        assert_eq!(created.value, "3 years ago");
        assert_eq!(created.title_ts, Some(10));
        assert!(created.title.is_empty());

        let updated = about_row(&stats.history, "Updated");
        assert_eq!(updated.value, "2 days ago");
        assert_eq!(updated.title_ts, Some(1700000000));

        assert_eq!(stats.languages[0].name, "Rust");
        assert_eq!(stats.languages[0].value, "60.0%");
        assert_eq!(stats.languages[0].lines, "6");
        assert_eq!(stats.language_total, "10");
    }

    #[test]
    fn stats_model_preserves_unspecified_about_and_activity_rows() {
        let report = ghrm_stat::Report {
            root: PathBuf::from("/tmp/repo"),
            sections: vec![
                ghrm_stat::Section::new(
                    ghrm_stat::Tool::Title,
                    vec![ghrm_stat::Row::new("title", "Readable title")],
                ),
                ghrm_stat::Section::new(
                    ghrm_stat::Tool::Description,
                    vec![ghrm_stat::Row::new("description", "Short description")],
                ),
                ghrm_stat::Section::new(
                    ghrm_stat::Tool::Pending,
                    vec![
                        ghrm_stat::Row::new("added", "1"),
                        ghrm_stat::Row::new("deleted", "0"),
                        ghrm_stat::Row::new("modified", "2"),
                    ],
                ),
                ghrm_stat::Section::new(
                    ghrm_stat::Tool::Loc,
                    vec![ghrm_stat::Row::new("linesOfCode", "42")],
                ),
            ],
        };
        let stats = stats_model(report, &SourceState::NoRepo, Path::new("/tmp/repo"));

        assert_eq!(about_row(&stats.metadata, "Title").value, "Readable title");
        assert_eq!(
            about_row(&stats.metadata, "Description").value,
            "Short description"
        );
        assert_eq!(
            about_row(&stats.activity, "Pending").value,
            "1 added, 0 deleted, 2 modified"
        );
        assert_eq!(about_row(&stats.activity, "LOC").value, "42");
    }

    #[test]
    fn stats_model_structures_list_rows() {
        let td = TempDir::new("ghrm-about-list-stats");
        let file = td.path().join("src/main.rs");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "fn main() {}\n").unwrap();
        let report = ghrm_stat::Report {
            root: td.path().to_path_buf(),
            sections: vec![
                ghrm_stat::Section::new(
                    ghrm_stat::Tool::Authors,
                    vec![ghrm_stat::Row::with_metrics(
                        "Wyatt Brege",
                        vec![
                            ghrm_stat::RowMetric::new("contribution", "100"),
                            ghrm_stat::RowMetric::new("commits", "147"),
                        ],
                    )],
                ),
                ghrm_stat::Section::new(
                    ghrm_stat::Tool::Churn,
                    vec![ghrm_stat::Row::new("src/main.rs", "7")],
                ),
            ],
        };
        let stats = stats_model(report, &SourceState::NoRepo, td.path());

        assert!(stats.metadata.is_empty());
        let author = &about_row(&stats.activity, "Authors").items[0];
        assert_eq!(author.label, "Wyatt Brege");
        assert_eq!(metric_values(author), vec!["147", "100%"]);
        assert_eq!(
            metric_titles(author),
            vec!["147 commits", "100% of commits"]
        );

        let churn = &about_row(&stats.activity, "Churn").items[0];
        assert_eq!(churn.label, "src/main.rs");
        assert_eq!(churn.href, "/src/main.rs");
        assert_eq!(metric_values(churn), vec!["7"]);
        assert_eq!(metric_titles(churn), vec!["7 commits"]);
    }

    #[test]
    fn stats_model_omits_churn_links_outside_served_root() {
        let repo = TempDir::new("ghrm-about-churn-repo");
        let served = TempDir::new("ghrm-about-churn-served");
        let file = repo.path().join("src/main.rs");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "fn main() {}\n").unwrap();
        let report = ghrm_stat::Report {
            root: repo.path().to_path_buf(),
            sections: vec![ghrm_stat::Section::new(
                ghrm_stat::Tool::Churn,
                vec![ghrm_stat::Row::new("src/main.rs", "7")],
            )],
        };
        let stats = stats_model(report, &SourceState::NoRepo, served.path());

        assert!(stats.activity[0].items[0].href.is_empty());
    }

    #[test]
    fn url_stats_use_forge_icon() {
        let report = ghrm_stat::Report {
            root: PathBuf::from("/tmp/repo"),
            sections: vec![ghrm_stat::Section::new(
                ghrm_stat::Tool::Url,
                vec![ghrm_stat::Row::new("url", "git@gitlab.com:team/repo.git")],
            )],
        };
        let stats = stats_model(report, &SourceState::NoRepo, Path::new("/tmp/repo"));

        assert_eq!(stats.metadata[0].icon, "ghrm-icon-gitlab");
    }

    #[test]
    fn url_stats_link_to_source_web_url() {
        let report = ghrm_stat::Report {
            root: PathBuf::from("/tmp/repo"),
            sections: vec![ghrm_stat::Section::new(
                ghrm_stat::Tool::Url,
                vec![ghrm_stat::Row::new("url", "git@github.com:brege/ghrm.git")],
            )],
        };
        let source = SourceState::Web {
            url: "https://github.com/brege/ghrm".to_string(),
            raw: "git@github.com:brege/ghrm.git".to_string(),
            forge: crate::repo::Forge::GitHub,
        };
        let stats = stats_model(report, &source, Path::new("/tmp/repo"));

        assert_eq!(stats.metadata[0].href, "https://github.com/brege/ghrm");
    }
}
