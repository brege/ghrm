use crate::http::server::{AppState, Mode};
use crate::paths;
use crate::repo::SourceState;
use crate::runtime;
use crate::tmpl::{self, AboutLanguage, AboutPeek, AboutStatItem, AboutStatRow, AboutStats};

use axum::{
    body::Body,
    extract::{Query, State},
    http::{StatusCode, header},
    response::Response,
};
use serde::Deserialize;
use std::path::PathBuf;
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
    let stats_cfg = s.stats.clone();
    let stats = if stats_cfg.enabled {
        tokio::task::spawn_blocking(move || {
            ghrm_stat::resolve_with_config(&stats_input, stats_cfg)
                .map(|report| stats_model(report, &source))
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

fn html_response(html: &str) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from(html.to_string()))
        .unwrap()
}

fn stats_model(report: ghrm_stat::Report, source: &SourceState) -> AboutStats {
    let mut about = AboutStats::default();
    for section in report.sections {
        match section.tool {
            ghrm_stat::Tool::Languages => about.languages = language_rows(&section.rows),
            ghrm_stat::Tool::Project
            | ghrm_stat::Tool::Version
            | ghrm_stat::Tool::License
            | ghrm_stat::Tool::Url => {
                if let Some(row) = stat_row(section, source) {
                    about.metadata.push(row);
                }
            }
            _ => {
                if let Some(row) = stat_row(section, source) {
                    about.stats.push(row);
                }
            }
        }
    }
    about
}

fn language_rows(rows: &[ghrm_stat::Row]) -> Vec<AboutLanguage> {
    rows.iter()
        .enumerate()
        .map(|(i, row)| {
            let color = LANGUAGE_COLORS[i % LANGUAGE_COLORS.len()].to_string();
            AboutLanguage {
                name: row.key.clone(),
                value: row.value.clone(),
                style: format!("--ghrm-lang-color: {color}; width: {}", row.value),
                title: format!("{} {}", row.key, row.value),
                color,
            }
        })
        .collect()
}

fn stat_row(section: ghrm_stat::Section, source: &SourceState) -> Option<AboutStatRow> {
    let label = stat_title(section.tool);
    let value = stat_value(section.tool, &section.rows)?;
    let icon = stat_icon(section.tool, &value);
    let href = stat_href(section.tool, &value, source);
    let items = stat_items(section.tool, &section.rows);
    Some(AboutStatRow {
        label: label.to_string(),
        value,
        icon,
        href,
        items,
    })
}

fn stat_value(tool: ghrm_stat::Tool, rows: &[ghrm_stat::Row]) -> Option<String> {
    match tool {
        ghrm_stat::Tool::Project => project_value(rows),
        ghrm_stat::Tool::Head => head_value(rows),
        ghrm_stat::Tool::Pending => pending_value(rows),
        ghrm_stat::Tool::Languages | ghrm_stat::Tool::Authors | ghrm_stat::Tool::Churn => {
            pair_list(rows)
        }
        ghrm_stat::Tool::Size => size_value(rows),
        ghrm_stat::Tool::Loc => row_value(rows, "linesOfCode").map(str::to_string),
        ghrm_stat::Tool::LastChange => row_value(rows, "lastChange").map(str::to_string),
        _ => compact_value(rows),
    }
}

fn project_value(rows: &[ghrm_stat::Row]) -> Option<String> {
    let name = row_value(rows, "name")?;
    let branches = row_value(rows, "branches").unwrap_or("0");
    let tags = row_value(rows, "tags").unwrap_or("0");
    Some(format!(
        "{name} / {branches} {} / {tags} {}",
        plural(branches, "branch", "branches"),
        plural(tags, "tag", "tags")
    ))
}

fn head_value(rows: &[ghrm_stat::Row]) -> Option<String> {
    let commit = row_value(rows, "commit")?;
    match row_value(rows, "refs") {
        Some(refs) => Some(format!("{commit} / {}", refs.replace(", ", " / "))),
        None => Some(commit.to_string()),
    }
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

fn pair_list(rows: &[ghrm_stat::Row]) -> Option<String> {
    let pairs = rows
        .iter()
        .map(|row| format!("{} {}", row.key, row.value))
        .collect::<Vec<_>>();
    (!pairs.is_empty()).then(|| pairs.join(", "))
}

fn stat_items(tool: ghrm_stat::Tool, rows: &[ghrm_stat::Row]) -> Vec<AboutStatItem> {
    if !matches!(tool, ghrm_stat::Tool::Authors | ghrm_stat::Tool::Churn) {
        return Vec::new();
    }
    rows.iter()
        .map(|row| AboutStatItem {
            label: row.key.clone(),
            value: row.value.clone(),
        })
        .collect()
}

fn compact_value(rows: &[ghrm_stat::Row]) -> Option<String> {
    match rows {
        [row] => Some(row.value.clone()),
        _ => pair_list(rows),
    }
}

fn row_value<'a>(rows: &'a [ghrm_stat::Row], key: &str) -> Option<&'a str> {
    rows.iter()
        .find(|row| row.key == key)
        .map(|row| row.value.as_str())
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
        ghrm_stat::Tool::Dependencies => "Deps",
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
        ghrm_stat::Tool::Authors => "ghrm-icon-people",
        ghrm_stat::Tool::License => "ghrm-icon-scale",
        ghrm_stat::Tool::Dependencies => "ghrm-icon-package-deps",
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
                icon: "",
                href: String::new(),
                items: Vec::new(),
            }],
            stats: Vec::new(),
            languages: vec![AboutLanguage {
                name: "Rust".to_string(),
                value: "60.0%".to_string(),
                color: "#d19a66".to_string(),
                style: "--ghrm-lang-color: #d19a66; width: 60.0%".to_string(),
                title: "Rust 60.0%".to_string(),
            }],
        };
        let html = html(&runtime_paths, &stats, true);

        assert!(html.contains("About"));
        assert!(html.contains("Languages"));
        assert!(html.contains("ghrm-about-stamp-button"));
        assert!(html.contains("<span>Project</span>"));
        assert!(html.contains("title=\"ghrm\""));
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
                    vec![ghrm_stat::Row::new("Rust", "60.0%")],
                ),
            ],
        };
        let stats = stats_model(report, &SourceState::NoRepo);

        assert_eq!(stats.metadata[0].label, "Project");
        assert_eq!(stats.metadata[0].value, "ghrm / 1 branch / 7 tags");
        assert_eq!(stats.languages[0].name, "Rust");
        assert_eq!(stats.languages[0].value, "60.0%");
    }

    #[test]
    fn stats_model_moves_authors_to_stats() {
        let report = ghrm_stat::Report {
            root: PathBuf::from("/tmp/repo"),
            sections: vec![ghrm_stat::Section::new(
                ghrm_stat::Tool::Authors,
                vec![ghrm_stat::Row::new("Wyatt Brege", "100% 147")],
            )],
        };
        let stats = stats_model(report, &SourceState::NoRepo);

        assert!(stats.metadata.is_empty());
        assert_eq!(stats.stats[0].label, "Authors");
        assert_eq!(stats.stats[0].value, "Wyatt Brege 100% 147");
        assert_eq!(stats.stats[0].items[0].label, "Wyatt Brege");
        assert_eq!(stats.stats[0].items[0].value, "100% 147");
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
        let stats = stats_model(report, &SourceState::NoRepo);

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
        let stats = stats_model(report, &source);

        assert_eq!(stats.metadata[0].href, "https://github.com/brege/ghrm");
    }
}
