use crate::column;
use crate::crumbs;
use crate::filter;
use crate::http::server::{AppState, HtmxContext};
use crate::http::{shell, vendor};
use crate::render::{self, Rendered};
use crate::tmpl::{
    self, ColumnControl, ExplorerCtx, ExplorerEntry, ExplorerReadme, FilterControl, SortControl,
    SortDirControl,
};
use crate::view::{self, ViewConfig, ViewState};
use crate::walk;

use axum::{body::Body, http::StatusCode, response::Response};
use std::cmp::Ordering;
use std::path::Path;
use tracing::warn;

pub(crate) async fn render(s: &AppState, rel: &str, view: ViewState, hx: HtmxContext) -> Response {
    let matcher = view::matcher(&view, &s.filters);
    let filter_exts = view::filter_exts(&view, &s.filter_exts);
    let tree = s.cached_nav_tree(&view, matcher.as_ref());
    let dir_opt = tree.as_ref().and_then(|tree| tree.dirs.get(rel).cloned());

    let dir = match dir_opt {
        Some(d) if d.entries.is_empty() => walk::list_dir(
            &s.target,
            Path::new(rel),
            walk::ListSpec {
                use_ignore: view.use_ignore,
                exclude_names: &s.exclude_names,
                extensions: filter_exts.unwrap_or(&[]),
                matcher: matcher.as_ref(),
                opts: view.opts,
                order: walk::SortSpec {
                    sort: view.sort,
                    dir: view.sort_dir,
                },
            },
        )
        .unwrap_or(d),
        Some(d) => d,
        None => match walk::list_dir(
            &s.target,
            Path::new(rel),
            walk::ListSpec {
                use_ignore: view.use_ignore,
                exclude_names: &s.exclude_names,
                extensions: filter_exts.unwrap_or(&[]),
                matcher: matcher.as_ref(),
                opts: view.opts,
                order: walk::SortSpec {
                    sort: view.sort,
                    dir: view.sort_dir,
                },
            },
        ) {
            Some(d) => d,
            None => return not_found(),
        },
    };

    let parent_href = if rel.is_empty() {
        String::new()
    } else if let Some(p) = Path::new(rel).parent() {
        let ps = p.to_string_lossy();
        if ps.is_empty() {
            "/".to_string()
        } else {
            format!("/{}/", ps)
        }
    } else {
        "/".to_string()
    };
    let has_parent = !rel.is_empty();
    let parent_href = view::with_view(&parent_href, &view, &s.view_cfg);

    let meta_req = column::required_meta(&view.columns);
    let entry_paths: Vec<_> = if meta_req.contains(column::MetaReq::COMMIT) {
        dir.entries
            .iter()
            .map(|e| Path::new(rel).join(&e.name))
            .map(|path| s.target.join(path))
            .collect()
    } else {
        Vec::new()
    };
    let commits = if meta_req.contains(column::MetaReq::COMMIT) {
        s.repos.commit_info(&entry_paths)
    } else {
        Default::default()
    };

    let mut entry_order: Vec<_> = dir.entries.iter().enumerate().collect();
    if matches!(
        view.sort,
        walk::Sort::CommitMessage | walk::Sort::CommitDate
    ) {
        entry_order.sort_by(|(a_idx, a), (b_idx, b)| {
            let a_commit = entry_paths.get(*a_idx).and_then(|path| commits.get(path));
            let b_commit = entry_paths.get(*b_idx).and_then(|path| commits.get(path));
            cmp_commit_entries(
                a.name.as_str(),
                a_commit,
                b.name.as_str(),
                b_commit,
                view.sort,
                view.sort_dir,
            )
        });
    }

    let entries: Vec<ExplorerEntry> = entry_order
        .into_iter()
        .map(|(idx, e)| {
            let commit = entry_paths.get(idx).and_then(|path| commits.get(path));
            let meta = column::RowMeta {
                modified: e.modified,
                size: e.size,
                lines: e.lines,
                commit_subject: commit.map(|commit| commit.subject.as_str()),
                commit_timestamp: commit.map(|commit| commit.timestamp),
            };
            ExplorerEntry {
                name: e.name.clone(),
                href: view::with_view(&e.href, &view, &s.view_cfg),
                is_dir: e.is_dir,
                cells: meta.cells(&view.columns),
            }
        })
        .collect();

    let mut readme_rendered: Option<Rendered> = None;
    let mut readme_name = String::new();
    if let Some(rel_readme) = &dir.readme {
        let readme_abs = s.target.join(rel_readme);
        if let Ok(md) = tokio::fs::read_to_string(&readme_abs).await {
            let r = render::render_at(
                &md,
                Some(render::RenderPath {
                    root: &s.target,
                    src: &readme_abs,
                }),
            );
            readme_name = Path::new(rel_readme)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            readme_rendered = Some(r);
        }
    }

    let title = if rel.is_empty() {
        s.target
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Preview".to_string())
    } else {
        rel.to_string()
    };

    let readme_tmpl = readme_rendered.as_ref().map(|r| ExplorerReadme {
        name: &readme_name,
        html: &r.html,
    });
    let crumbs = crumbs::html(&s.target, s.home.as_deref(), rel, &view, &s.view_cfg);
    let article_class = view.columns.article_class("markdown-body");
    let empty_cells = view.columns.empty_cells();
    let current_href = if rel.is_empty() {
        "/".to_string()
    } else {
        format!("/{rel}/")
    };
    let controls = build_controls(&current_href, &view, &s.view_cfg, &s.filters);
    let (has_mermaid, has_math, has_map) = readme_rendered
        .as_ref()
        .map(|r| (r.has_mermaid, r.has_math, r.has_map))
        .unwrap_or_default();
    let combined = Rendered {
        html: String::new(),
        title,
        lang: None,
        has_mermaid,
        has_math,
        has_map,
    };
    let features = vendor::feature_list(&combined);
    let body = match tmpl::explorer(ExplorerCtx {
        article_class: &article_class,
        features: &features,
        crumbs: &crumbs,
        current_path: rel,
        has_parent,
        parent_href: &parent_href,
        filter_menu_active: controls.filter_menu_active,
        filter_controls: &controls.filter_controls,
        sort_menu_active: controls.sort_menu_active,
        sort_controls: &controls.sort_controls,
        sort_dir_control: &controls.sort_dir_control,
        column_menu_active: controls.column_menu_active,
        column_controls: &controls.column_controls,
        headers_control: &controls.headers_control,
        column_defs: column::DEFS,
        show_headers: view.show_headers,
        empty_cells: &empty_cells,
        entries: &entries,
        readme: readme_tmpl,
    }) {
        Ok(b) => b,
        Err(e) => {
            warn!("template error: {}", e);
            return not_found();
        }
    };

    let current = if rel.is_empty() {
        s.target.clone()
    } else {
        s.target.join(rel)
    };
    let source = s.repos.source_for(&current);
    if hx.is_htmx {
        return shell::fragment(&body, &combined.title, source);
    }
    shell::full_page(&combined, &body, source, s.auth.is_some(), &s.runtime_paths)
}

fn cmp_commit_entries(
    a_name: &str,
    a_commit: Option<&crate::repo::CommitInfo>,
    b_name: &str,
    b_commit: Option<&crate::repo::CommitInfo>,
    sort: walk::Sort,
    dir: walk::SortDir,
) -> Ordering {
    let order = match sort {
        walk::Sort::CommitMessage => a_commit
            .map(|commit| commit.subject.to_lowercase())
            .cmp(&b_commit.map(|commit| commit.subject.to_lowercase()))
            .then_with(|| a_name.to_lowercase().cmp(&b_name.to_lowercase())),
        walk::Sort::CommitDate => a_commit
            .map(|commit| commit.timestamp)
            .cmp(&b_commit.map(|commit| commit.timestamp))
            .then_with(|| a_name.to_lowercase().cmp(&b_name.to_lowercase())),
        _ => a_name.to_lowercase().cmp(&b_name.to_lowercase()),
    };
    match dir {
        walk::SortDir::Asc => order,
        walk::SortDir::Desc => order.reverse(),
    }
}

struct Controls {
    filter_menu_active: bool,
    filter_controls: Vec<FilterControl>,
    sort_menu_active: bool,
    sort_controls: Vec<SortControl>,
    sort_dir_control: SortDirControl,
    column_menu_active: bool,
    column_controls: Vec<ColumnControl>,
    headers_control: ColumnControl,
}

fn build_controls(
    href: &str,
    view: &ViewState,
    cfg: &ViewConfig,
    filters: &filter::Set,
) -> Controls {
    let filter_menu_active = view.opts.show_hidden != cfg.default.show_hidden
        || (cfg.can_toggle_excludes && view.opts.show_excludes != cfg.default.show_excludes)
        || view.use_ignore != cfg.default_use_ignore
        || view.opts.filter_ext != cfg.default.filter_ext
        || view.groups != cfg.default_groups;
    let mut filter_controls = vec![
        FilterControl {
            href: view::with_view(href, &view::toggle_hidden(view), cfg),
            label: "Show hidden".to_string(),
            title: "Set by -H".to_string(),
            active: view.opts.show_hidden,
            hidden: false,
            group: false,
        },
        FilterControl {
            href: view::with_view(href, &view::toggle_excludes(view, cfg), cfg),
            label: "Show excludes".to_string(),
            title: "Set by -E".to_string(),
            active: view.opts.show_excludes,
            hidden: !cfg.can_toggle_excludes,
            group: false,
        },
        FilterControl {
            href: view::with_view(href, &view::toggle_ignore(view), cfg),
            label: "Show gitignores".to_string(),
            title: "Set by -I".to_string(),
            active: !view.use_ignore,
            hidden: false,
            group: false,
        },
        FilterControl {
            href: view::with_view(href, &view::toggle_filter(view, cfg), cfg),
            label: "Filter files".to_string(),
            title: "Customize with -e <file extension>".to_string(),
            active: view.opts.filter_ext,
            hidden: false,
            group: false,
        },
    ];
    for group in filters.groups() {
        filter_controls.push(FilterControl {
            href: view::with_view(href, &view::toggle_group(view, &group.name), cfg),
            label: group.label.clone(),
            title: group.detail.clone(),
            active: view.opts.filter_ext && view.groups.contains(&group.name),
            hidden: false,
            group: true,
        });
    }

    let sort_menu_active =
        view.sort != cfg.default_sort || view.sort_dir != view.sort.default_dir();
    let sort_controls = walk::SORT_DEFS
        .iter()
        .map(|def| SortControl {
            href: view::with_view(href, &view::set_sort(view, def.sort), cfg),
            label: def.label,
            title: def.title,
            active: view.sort == def.sort,
            hidden: def
                .column_key
                .is_some_and(|key| !view.columns.is_visible_key(key)),
        })
        .collect();
    let sort_dir_desc = view.sort_dir == walk::SortDir::Desc;
    let sort_dir_control = SortDirControl {
        href: view::with_view(href, &view::toggle_sort_dir(view), cfg),
        label: if sort_dir_desc {
            "Sort descending"
        } else {
            "Sort ascending"
        },
        icon: if sort_dir_desc {
            "ghrm-icon-chevron-down"
        } else {
            "ghrm-icon-chevron-up"
        },
        active: view.sort_dir != view.sort.default_dir(),
    };

    let column_menu_active = view.columns != cfg.default_columns || view.show_headers;
    let column_controls = column::DEFS
        .iter()
        .map(|def| ColumnControl {
            href: view::with_view(href, &view::toggle_column(view, def.key), cfg),
            key: def.key,
            label: def.label,
            title: def.title,
            active: view.columns.is_visible(def),
            edge: def.edge,
        })
        .collect();
    let headers_control = ColumnControl {
        href: view::with_view(href, &view::toggle_headers(view), cfg),
        key: "headers",
        label: "Show column headers",
        title: "Show column headers in explorer",
        active: view.show_headers,
        edge: false,
    };

    Controls {
        filter_menu_active,
        filter_controls,
        sort_menu_active,
        sort_controls,
        sort_dir_control,
        column_menu_active,
        column_controls,
        headers_control,
    }
}

fn not_found() -> Response {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(axum::http::header::CACHE_CONTROL, "no-store")
        .body(Body::from("404"))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::group_filters;

    fn columns(date: bool, size: bool, lines: bool) -> column::Set {
        column::Set::from_defaults(|def| match def.key {
            "date" => date,
            "size" => size,
            "lines" => lines,
            _ => false,
        })
    }

    #[test]
    fn controls_reflect_active_view_state() {
        let filters = group_filters();
        let cfg = ViewConfig {
            default: walk::ViewOpts {
                show_hidden: false,
                show_excludes: true,
                filter_ext: false,
            },
            default_use_ignore: true,
            default_groups: filters.default_groups().to_vec(),
            default_sort: walk::Sort::Name,
            default_columns: columns(true, false, false),
            can_toggle_excludes: true,
        };
        let view = ViewState {
            opts: walk::ViewOpts {
                show_hidden: true,
                show_excludes: false,
                filter_ext: true,
            },
            use_ignore: false,
            groups: vec!["docs".to_string()],
            sort: walk::Sort::Size,
            sort_dir: walk::SortDir::Desc,
            columns: columns(true, true, false),
            show_headers: true,
        };

        let controls = build_controls("/docs/", &view, &cfg, &filters);
        let hidden = controls
            .filter_controls
            .iter()
            .find(|control| control.label == "Show hidden")
            .unwrap();
        let docs = controls
            .filter_controls
            .iter()
            .find(|control| control.label == "Docs")
            .unwrap();
        let size = controls
            .sort_controls
            .iter()
            .find(|control| control.label == "Sort by size")
            .unwrap();
        let lines = controls
            .sort_controls
            .iter()
            .find(|control| control.label == "Sort by lines")
            .unwrap();

        assert!(controls.filter_menu_active);
        assert!(hidden.active);
        assert!(docs.active);
        assert!(controls.sort_menu_active);
        assert!(size.active);
        assert!(!size.hidden);
        assert!(lines.hidden);
        assert!(controls.column_menu_active);
        assert!(controls.headers_control.active);
    }
}
