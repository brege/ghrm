use crate::explorer::view::{self, ViewConfig, ViewState};

use std::path::{Component, Path};

pub(crate) fn html(
    target: &Path,
    home: Option<&Path>,
    rel: &str,
    view: &ViewState,
    cfg: &ViewConfig,
) -> String {
    let display_root = home
        .and_then(|home| target.strip_prefix(home).ok())
        .unwrap_or(target);

    let base_parts: Vec<String> = display_root
        .components()
        .filter_map(|comp| match comp {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    let rel_parts: Vec<String> = Path::new(rel)
        .components()
        .filter_map(|comp| match comp {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();

    let root_idx = base_parts.len().saturating_sub(1);
    let total = base_parts.len() + rel_parts.len();
    let mut out = String::new();

    for idx in 0..total {
        if idx > 0 {
            out.push_str(r#"<span class="ghrm-crumb-sep">/</span>"#);
        }

        let label = if idx < base_parts.len() {
            &base_parts[idx]
        } else {
            &rel_parts[idx - base_parts.len()]
        };
        let label = html_escape::encode_text(label);
        let is_last = idx + 1 == total;

        if idx < root_idx {
            out.push_str(r#"<span class="ghrm-crumb ghrm-crumb-static">"#);
            out.push_str(&label);
            out.push_str("</span>");
            continue;
        }

        if is_last {
            out.push_str(r#"<strong class="ghrm-crumb ghrm-crumb-current">"#);
            out.push_str(&label);
            out.push_str("</strong>");
            continue;
        }

        let href = if idx == root_idx {
            "/".to_string()
        } else {
            let depth = idx - root_idx;
            format!("/{}/", rel_parts[..depth].join("/"))
        };
        out.push_str(r#"<a class="ghrm-crumb ghrm-crumb-link" href=""#);
        out.push_str(&html_escape::encode_double_quoted_attribute(
            &view::with_view(&href, view, cfg),
        ));
        out.push_str(r#"">"#);
        out.push_str(&label);
        out.push_str("</a>");
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::explorer::column;
    use crate::explorer::walk::{Sort, ViewOpts};
    use std::path::PathBuf;

    fn default_columns() -> column::Set {
        column::Set::from_defaults(|def| def.default_visible)
    }

    fn default_view_config() -> ViewConfig {
        ViewConfig {
            default: ViewOpts::default(),
            default_use_ignore: true,
            default_groups: Vec::new(),
            default_sort: Sort::Name,
            default_columns: default_columns(),
            can_toggle_excludes: false,
        }
    }

    fn default_view_state() -> ViewState {
        let cfg = default_view_config();
        ViewState {
            opts: cfg.default,
            use_ignore: cfg.default_use_ignore,
            groups: Vec::new(),
            sort: cfg.default_sort,
            sort_dir: cfg.default_sort.default_dir(),
            columns: cfg.default_columns.clone(),
            show_headers: false,
        }
    }

    #[test]
    fn crumbs_root_only() {
        let target = PathBuf::from("/home/user/project");
        let view = default_view_state();
        let cfg = default_view_config();

        let result = html(&target, Some(Path::new("/home/user")), "", &view, &cfg);

        assert!(result.contains("ghrm-crumb-current"));
        assert!(result.contains("project"));
    }

    #[test]
    fn crumbs_nested_path() {
        let target = PathBuf::from("/home/user/project");
        let view = default_view_state();
        let cfg = default_view_config();

        let result = html(
            &target,
            Some(Path::new("/home/user")),
            "src/lib",
            &view,
            &cfg,
        );

        assert!(result.contains("project"));
        assert!(result.contains("src"));
        assert!(result.contains("lib"));
        let sep_count = result.matches("ghrm-crumb-sep").count();
        assert_eq!(sep_count, 2);
    }

    #[test]
    fn crumbs_current_is_strong() {
        let target = PathBuf::from("/home/user/project");
        let view = default_view_state();
        let cfg = default_view_config();

        let result = html(&target, Some(Path::new("/home/user")), "src", &view, &cfg);

        assert!(result.contains(r#"<strong class="ghrm-crumb ghrm-crumb-current">src</strong>"#));
    }

    #[test]
    fn crumbs_link_hrefs_correct() {
        let target = PathBuf::from("/home/user/project");
        let view = default_view_state();
        let cfg = default_view_config();

        let result = html(
            &target,
            Some(Path::new("/home/user")),
            "src/lib/utils",
            &view,
            &cfg,
        );

        assert!(result.contains(r#"href="/""#));
        assert!(result.contains(r#"href="/src/""#));
        assert!(result.contains(r#"href="/src/lib/""#));
    }

    #[test]
    fn crumbs_escapes_label_html() {
        let target = PathBuf::from("/home/user/<script>");
        let view = default_view_state();
        let cfg = default_view_config();

        let result = html(&target, Some(Path::new("/home/user")), "", &view, &cfg);

        assert!(result.contains("&lt;script&gt;"));
        assert!(!result.contains("<script>"));
    }

    #[test]
    fn crumbs_static_before_root() {
        let target = PathBuf::from("/home/user/code/project");
        let view = default_view_state();
        let cfg = default_view_config();

        let result = html(&target, Some(Path::new("/home/user")), "", &view, &cfg);

        assert!(result.contains(r#"<span class="ghrm-crumb ghrm-crumb-static">code</span>"#));
        assert!(
            result.contains(r#"<strong class="ghrm-crumb ghrm-crumb-current">project</strong>"#)
        );
    }

    #[test]
    fn crumbs_no_home_uses_full_path() {
        let target = PathBuf::from("/data/project");
        let view = default_view_state();
        let cfg = default_view_config();

        let result = html(&target, None, "", &view, &cfg);

        assert!(result.contains("data"));
        assert!(result.contains("project"));
    }

    #[test]
    fn crumbs_preserves_view_state() {
        let target = PathBuf::from("/home/user/project");
        let cfg = default_view_config();
        let mut view = default_view_state();
        view.opts.show_hidden = true;

        let result = html(&target, Some(Path::new("/home/user")), "src", &view, &cfg);

        assert!(result.contains("hidden=1"));
    }

    #[test]
    fn crumbs_empty_rel_shows_root() {
        let target = PathBuf::from("/project");
        let view = default_view_state();
        let cfg = default_view_config();

        let result = html(&target, None, "", &view, &cfg);

        assert!(result.contains("ghrm-crumb-current"));
        assert!(result.contains("project"));
        assert!(!result.contains("ghrm-crumb-sep"));
    }

    #[test]
    fn crumbs_single_rel_part() {
        let target = PathBuf::from("/project");
        let view = default_view_state();
        let cfg = default_view_config();

        let result = html(&target, None, "src", &view, &cfg);

        assert!(result.contains(r#"href="/""#));
        assert!(result.contains(r#"ghrm-crumb-link"#));
        assert!(result.contains(r#"ghrm-crumb-current">src</strong>"#));
    }

    #[test]
    fn crumbs_file_target_keeps_parent_static() {
        let target = PathBuf::from("/home/user/code/project/README.md");
        let view = default_view_state();
        let cfg = default_view_config();

        let result = html(&target, Some(Path::new("/home/user")), "", &view, &cfg);

        assert!(result.contains(r#"<span class="ghrm-crumb ghrm-crumb-static">project</span>"#));
        assert!(
            result.contains(r#"<strong class="ghrm-crumb ghrm-crumb-current">README.md</strong>"#)
        );
        assert!(!result.contains(r#"href="/""#));
        assert!(!result.contains(r#"ghrm-crumb-link"#));
    }

    #[test]
    fn crumbs_depth_math_with_nonzero_root_idx() {
        let target = PathBuf::from("/home/user/code/project");
        let view = default_view_state();
        let cfg = default_view_config();

        let result = html(
            &target,
            Some(Path::new("/home/user")),
            "src/lib",
            &view,
            &cfg,
        );

        assert!(result.contains(r#"href="/""#), "project should link to /");
        assert!(
            result.contains(r#"href="/src/""#),
            "src should link to /src/, got: {result}"
        );
    }
}
