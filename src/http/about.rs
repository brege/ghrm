use crate::http::server::{AppState, Mode};
use crate::paths;
use crate::runtime;
use crate::tmpl::{self, AboutPeek, AboutStatRow};

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

#[derive(Default, Deserialize)]
pub(crate) struct AboutQuery {
    path: Option<String>,
}

pub(crate) async fn show(State(s): State<AppState>, Query(q): Query<AboutQuery>) -> Response {
    let stats_path = about_path(&s, q.path.as_deref());
    let stats_cfg = s.stats.clone();
    let stats = if stats_cfg.enabled {
        tokio::task::spawn_blocking(move || {
            ghrm_stat::resolve_with_config(&stats_path, stats_cfg)
                .map(stat_rows)
                .unwrap_or_default()
        })
        .await
        .unwrap_or_default()
    } else {
        Vec::new()
    };

    html_response(&html(&s.runtime_paths, &stats, false, true))
}

pub(crate) fn html(
    runtime_paths: &runtime::Paths,
    stats: &[AboutStatRow],
    oob: bool,
    stats_loaded: bool,
) -> String {
    let project_version = env!("CARGO_PKG_VERSION");
    let project_release_href = format!("{PROJECT_URL}/releases/tag/v{project_version}");
    let about = AboutPeek {
        oob,
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

fn html_response(html: &str) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from(html.to_string()))
        .unwrap()
}

fn stat_rows(report: ghrm_stat::Report) -> Vec<AboutStatRow> {
    report.sections.into_iter().filter_map(stat_row).collect()
}

fn stat_row(section: ghrm_stat::Section) -> Option<AboutStatRow> {
    let label = stat_title(section.tool);
    let value = stat_value(section.tool, &section.rows)?;
    Some(AboutStatRow {
        label: label.to_string(),
        value,
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
        "{name} ({branches} {}, {tags} {})",
        plural(branches, "branch", "branches"),
        plural(tags, "tag", "tags")
    ))
}

fn head_value(rows: &[ghrm_stat::Row]) -> Option<String> {
    let commit = row_value(rows, "commit")?;
    match row_value(rows, "refs") {
        Some(refs) => Some(format!("{commit} ({refs})")),
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
        ghrm_stat::Tool::LastChange => "Changed",
        ghrm_stat::Tool::Url => "URL",
        ghrm_stat::Tool::Commits => "Commits",
        ghrm_stat::Tool::Churn => "Churn",
        ghrm_stat::Tool::Loc => "LOC",
        ghrm_stat::Tool::Size => "Size",
        ghrm_stat::Tool::License => "License",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    fn test_runtime_paths() -> runtime::Paths {
        let td = TempDir::new("ghrm-about-runtime-paths");
        runtime::Paths::new(td.path(), None).unwrap()
    }

    #[test]
    fn about_html_renders_runtime_and_app_links() {
        let runtime_paths = test_runtime_paths();
        let html = html(&runtime_paths, &[], false, false);

        assert!(html.contains("Runtime Paths"));
        assert!(html.contains("href=\"https://github.com/brege/ghrm\""));
        assert!(html.contains(">brege/ghrm</span>"));
        assert!(html.contains("data-stats-loaded=\"false\""));
    }

    #[test]
    fn about_html_omits_current_source() {
        let runtime_paths = test_runtime_paths();
        let html = html(&runtime_paths, &[], false, false);

        assert!(!html.contains("Current Source"));
    }

    #[test]
    fn about_oob_includes_swap_attribute() {
        let runtime_paths = test_runtime_paths();
        let html = html(&runtime_paths, &[], true, false);

        assert!(html.contains("id=\"ghrm-about-peek\""));
        assert!(html.contains("hx-swap-oob=\"true\""));
    }

    #[test]
    fn about_html_renders_stats_when_loaded() {
        let runtime_paths = test_runtime_paths();
        let stats = vec![AboutStatRow {
            label: "Project".to_string(),
            value: "ghrm".to_string(),
        }];
        let html = html(&runtime_paths, &stats, false, true);

        assert!(html.contains("Repository Stats"));
        assert!(html.contains("<dt>Project</dt>"));
        assert!(html.contains("<dd title=\"ghrm\">ghrm</dd>"));
        assert!(html.contains("data-stats-loaded=\"true\""));
    }

    #[test]
    fn stat_rows_compacts_tool_rows() {
        let report = ghrm_stat::Report {
            root: PathBuf::from("/tmp/repo"),
            sections: vec![ghrm_stat::Section::new(
                ghrm_stat::Tool::Project,
                vec![
                    ghrm_stat::Row::new("name", "ghrm"),
                    ghrm_stat::Row::new("branches", "1"),
                    ghrm_stat::Row::new("tags", "7"),
                ],
            )],
        };
        let rows = stat_rows(report);

        assert_eq!(rows[0].label, "Project");
        assert_eq!(rows[0].value, "ghrm (1 branch, 7 tags)");
    }
}
