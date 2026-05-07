use crate::explorer::walk;
use crate::http::server::{AppState, Mode};
use crate::paths;
use crate::repo::SourceState;
use crate::runtime;
use crate::tmpl::{
    self, AboutLanguage, AboutPeek, AboutStatItem, AboutStatMetric, AboutStatPart, AboutStatRow,
    AboutStats,
};

use axum::{
    body::Body,
    extract::{Query, State},
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

#[derive(Default, Deserialize)]
pub(crate) struct AboutQuery {
    path: Option<String>,
}

pub(crate) async fn show(State(s): State<AppState>, Query(q): Query<AboutQuery>) -> Response {
    let stats_path = about_path(&s, q.path.as_deref());
    let source = s.repos.source_for(&stats_path);
    let stats_input = stats_input_path(&stats_path);
    let served_root = served_root(&s);
    let stats_cfg = s.stats.clone();
    let stats = if stats_cfg.enabled {
        tokio::task::spawn_blocking(move || {
            ghrm_stat::resolve_with_config(&stats_input, stats_cfg)
                .map(|report| stats_model(report, &source, &served_root))
                .unwrap_or_default()
        })
        .await
        .unwrap_or_default()
    } else {
        AboutStats::default()
    };

    html_response(&html(&s.runtime_paths, &stats, true))
}

pub(crate) fn html(
    runtime_paths: &runtime::Paths,
    stats: &AboutStats,
    stats_loaded: bool,
) -> String {
    let project_version = env!("CARGO_PKG_VERSION");
    let project_release_href = format!("{PROJECT_URL}/releases/tag/v{project_version}");
    let about = AboutPeek {
        runtime_paths: runtime_paths.rows(),
        stats_loaded,
        stats,
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
            | ghrm_stat::Tool::Url => {
                if let Some(row) = stat_row(section, source, &repo_root, served_root) {
                    about.metadata.push(row);
                }
            }
            _ => {
                if let Some(row) = stat_row(section, source, &repo_root, served_root) {
                    about.stats.push(row);
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
    if !matches!(tool, ghrm_stat::Tool::Created | ghrm_stat::Tool::LastChange) {
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
    if !matches!(tool, ghrm_stat::Tool::Authors | ghrm_stat::Tool::Churn) {
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
    match tool {
        ghrm_stat::Tool::Title => "Title",
        ghrm_stat::Tool::Project => "Project",
        ghrm_stat::Tool::Description => "Description",
        ghrm_stat::Tool::Head => "Head",
        ghrm_stat::Tool::Pending => "Pending",
        ghrm_stat::Tool::Version => "Version",
        ghrm_stat::Tool::Created => "Created",
        ghrm_stat::Tool::Languages => "Languages",
        ghrm_stat::Tool::Authors => "Authors",
        ghrm_stat::Tool::LastChange => "Updated",
        ghrm_stat::Tool::Url => "URL",
        ghrm_stat::Tool::Commits => "Commits",
        ghrm_stat::Tool::Churn => "Churn",
        ghrm_stat::Tool::Loc => "LOC",
        ghrm_stat::Tool::Size => "Size",
        ghrm_stat::Tool::License => "License",
    }
}

fn stat_icon(tool: ghrm_stat::Tool, value: &str) -> &'static str {
    match tool {
        ghrm_stat::Tool::Url => forge_icon(value),
        ghrm_stat::Tool::Version => "ghrm-icon-fork",
        ghrm_stat::Tool::Project => "ghrm-icon-table",
        ghrm_stat::Tool::Head => "ghrm-icon-location",
        ghrm_stat::Tool::Created => "ghrm-icon-created",
        ghrm_stat::Tool::Authors => "ghrm-icon-people",
        ghrm_stat::Tool::License => "ghrm-icon-scale",
        ghrm_stat::Tool::LastChange => "ghrm-icon-update",
        ghrm_stat::Tool::Commits => "ghrm-icon-commit",
        ghrm_stat::Tool::Churn => "ghrm-icon-repeat",
        ghrm_stat::Tool::Loc => "ghrm-icon-loc",
        ghrm_stat::Tool::Size => "ghrm-icon-data",
        _ => "",
    }
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

    #[test]
    fn about_html_renders_runtime_and_app_links() {
        let runtime_paths = test_runtime_paths();
        let stats = AboutStats::default();
        let html = html(&runtime_paths, &stats, false);

        assert!(html.contains("Runtime Paths"));
        assert!(html.contains("href=\"https://github.com/brege/ghrm\""));
        assert!(html.contains(">brege/ghrm</span>"));
        assert!(html.contains("data-stats-loaded=\"false\""));
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
    fn about_html_renders_stats_when_loaded() {
        let runtime_paths = test_runtime_paths();
        let stats = AboutStats {
            metadata: vec![AboutStatRow {
                label: "Project".to_string(),
                value: "ghrm".to_string(),
                title: "project".to_string(),
                title_ts: None,
                parts: Vec::new(),
                icon: "",
                href: String::new(),
                items: Vec::new(),
            }],
            stats: Vec::new(),
            languages: vec![AboutLanguage {
                name: "Rust".to_string(),
                value: "60.0%".to_string(),
                lines: "6".to_string(),
                color: "#d19a66".to_string(),
                style: "--ghrm-lang-color: #d19a66; width: 60.0%".to_string(),
                title: "Rust 6 LOC 60.0%".to_string(),
            }],
            language_total: "10".to_string(),
        };
        let html = html(&runtime_paths, &stats, true);

        assert!(html.contains("About"));
        assert!(html.contains("Languages"));
        assert!(html.contains("ghrm-about-stamp-button"));
        assert!(html.contains("<span>Project</span>"));
        assert!(html.contains("title=\"project\""));
        assert!(html.contains("Rust"));
        assert!(html.contains("60.0%"));
        assert!(html.contains("data-stats-loaded=\"true\""));
    }

    #[test]
    fn stats_model_compacts_tool_rows() {
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
                    ghrm_stat::Tool::Languages,
                    vec![
                        ghrm_stat::Row::new("Rust", "6"),
                        ghrm_stat::Row::new("CSS", "4"),
                    ],
                ),
            ],
        };
        let stats = stats_model(report, &SourceState::NoRepo, Path::new("/tmp/repo"));

        assert_eq!(stats.metadata[0].label, "Project");
        assert_eq!(stats.metadata[0].value, "ghrm / 1 branch / 7 tags");
        assert_eq!(stats.metadata[0].title, "project / branches / tags");
        assert_eq!(stats.metadata[0].parts[0].value, "ghrm");
        assert!(!stats.metadata[0].parts[0].separator);
        assert_eq!(stats.metadata[0].parts[1].value, "1 branch");
        assert!(stats.metadata[0].parts[1].separator);
        assert_eq!(stats.metadata[0].parts[2].value, "7 tags");
        assert!(stats.metadata[0].parts[2].separator);
        assert_eq!(stats.languages[0].name, "Rust");
        assert_eq!(stats.languages[0].value, "60.0%");
        assert_eq!(stats.languages[0].lines, "6");
        assert_eq!(stats.language_total, "10");
    }

    #[test]
    fn stats_model_keeps_head_refs_grouped() {
        let report = ghrm_stat::Report {
            root: PathBuf::from("/tmp/repo"),
            sections: vec![ghrm_stat::Section::new(
                ghrm_stat::Tool::Head,
                vec![
                    ghrm_stat::Row::new("commit", "10314cff"),
                    ghrm_stat::Row::new("refs", "main, origin/main"),
                ],
            )],
        };
        let stats = stats_model(report, &SourceState::NoRepo, Path::new("/tmp/repo"));

        assert_eq!(stats.stats[0].label, "Head");
        assert_eq!(stats.stats[0].value, "10314cff / main, origin/main");
        assert_eq!(stats.stats[0].title, "commit hash / refs");
        assert_eq!(stats.stats[0].parts[0].value, "10314cff");
        assert!(!stats.stats[0].parts[0].separator);
        assert_eq!(stats.stats[0].parts[1].value, "main, origin/main");
        assert!(stats.stats[0].parts[1].separator);
    }

    #[test]
    fn stats_model_keeps_date_tooltip_timestamp_numeric() {
        let mut row = ghrm_stat::Row::new("created", "3 years ago");
        row.metrics
            .push(ghrm_stat::RowMetric::new("timestamp", "10"));
        let report = ghrm_stat::Report {
            root: PathBuf::from("/tmp/repo"),
            sections: vec![ghrm_stat::Section::new(ghrm_stat::Tool::Created, vec![row])],
        };
        let stats = stats_model(report, &SourceState::NoRepo, Path::new("/tmp/repo"));

        assert_eq!(stats.stats[0].label, "Created");
        assert_eq!(stats.stats[0].value, "3 years ago");
        assert!(stats.stats[0].title.is_empty());
        assert_eq!(stats.stats[0].title_ts, Some(10));
    }

    #[test]
    fn stats_model_moves_authors_to_stats() {
        let report = ghrm_stat::Report {
            root: PathBuf::from("/tmp/repo"),
            sections: vec![ghrm_stat::Section::new(
                ghrm_stat::Tool::Authors,
                vec![ghrm_stat::Row::with_metrics(
                    "Wyatt Brege",
                    vec![
                        ghrm_stat::RowMetric::new("contribution", "100"),
                        ghrm_stat::RowMetric::new("commits", "147"),
                    ],
                )],
            )],
        };
        let stats = stats_model(report, &SourceState::NoRepo, Path::new("/tmp/repo"));

        assert!(stats.metadata.is_empty());
        assert_eq!(stats.stats[0].label, "Authors");
        assert!(stats.stats[0].value.is_empty());
        assert_eq!(stats.stats[0].items[0].label, "Wyatt Brege");
        assert!(stats.stats[0].items[0].value.is_empty());
        assert_eq!(stats.stats[0].items[0].metrics[0].value, "147");
        assert!(stats.stats[0].items[0].metrics[0].label.is_empty());
        assert_eq!(stats.stats[0].items[0].metrics[0].title, "147 commits");
        assert_eq!(stats.stats[0].items[0].metrics[1].value, "100%");
        assert!(stats.stats[0].items[0].metrics[1].label.is_empty());
        assert_eq!(stats.stats[0].items[0].metrics[1].title, "100% of commits");
        assert!(stats.stats[0].items[0].href.is_empty());
    }

    #[test]
    fn stats_model_links_churn_paths_under_served_root() {
        let td = TempDir::new("ghrm-about-churn-links");
        let file = td.path().join("src/main.rs");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "fn main() {}\n").unwrap();
        let report = ghrm_stat::Report {
            root: td.path().to_path_buf(),
            sections: vec![ghrm_stat::Section::new(
                ghrm_stat::Tool::Churn,
                vec![ghrm_stat::Row::new("src/main.rs", "7")],
            )],
        };
        let stats = stats_model(report, &SourceState::NoRepo, td.path());

        assert_eq!(stats.stats[0].items[0].label, "src/main.rs");
        assert_eq!(stats.stats[0].items[0].href, "/src/main.rs");
        assert_eq!(stats.stats[0].items[0].metrics[0].value, "7");
        assert!(stats.stats[0].items[0].metrics[0].label.is_empty());
        assert_eq!(stats.stats[0].items[0].metrics[0].title, "7 commits");
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

        assert!(stats.stats[0].items[0].href.is_empty());
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
