use crate::filter;
use crate::walk::{self, ViewOpts};

use serde::Deserialize;

#[derive(Clone)]
pub(crate) struct ViewConfig {
    pub(crate) default: ViewOpts,
    pub(crate) default_groups: Vec<String>,
    pub(crate) default_sort: walk::Sort,
    pub(crate) can_toggle_excludes: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ViewState {
    pub(crate) opts: ViewOpts,
    pub(crate) groups: Vec<String>,
    pub(crate) sort: walk::Sort,
    pub(crate) sort_dir: walk::SortDir,
}

#[derive(Default, Deserialize)]
pub(crate) struct ViewQuery {
    pub(crate) hidden: Option<String>,
    pub(crate) excludes: Option<String>,
    pub(crate) filter: Option<String>,
    pub(crate) sort: Option<String>,
    pub(crate) dir: Option<String>,
}

pub(crate) fn matcher(view: &ViewState, filters: &filter::Set) -> Option<filter::Matcher> {
    if !view.opts.filter_ext || view.groups.is_empty() {
        return None;
    }
    filters.matcher_for_groups(&view.groups).ok().flatten()
}

pub(crate) fn filter_exts<'a>(view: &ViewState, filter_exts: &'a [String]) -> Option<&'a [String]> {
    if view.opts.filter_ext && view.groups.is_empty() {
        Some(filter_exts)
    } else {
        None
    }
}

pub(crate) fn from_query(
    q: &ViewQuery,
    raw_query: Option<&str>,
    cfg: &ViewConfig,
    filters: &filter::Set,
) -> ViewState {
    let mut groups = parse_group_params(raw_query, filters);
    if groups.is_empty() {
        groups = cfg.default_groups.clone();
    }
    let filter_ext = q
        .filter
        .as_deref()
        .and_then(parse_bool_param)
        .unwrap_or(cfg.default.filter_ext);
    if filter_ext && groups.is_empty() {
        groups = cfg.default_groups.clone();
    }
    let sort = q
        .sort
        .as_deref()
        .and_then(walk::Sort::parse)
        .unwrap_or(cfg.default_sort);
    let sort_dir = q
        .dir
        .as_deref()
        .and_then(walk::SortDir::parse)
        .unwrap_or_else(|| sort.default_dir());

    ViewState {
        opts: ViewOpts {
            show_hidden: q
                .hidden
                .as_deref()
                .and_then(parse_bool_param)
                .unwrap_or(cfg.default.show_hidden),
            show_excludes: if cfg.can_toggle_excludes {
                q.excludes
                    .as_deref()
                    .and_then(parse_bool_param)
                    .unwrap_or(cfg.default.show_excludes)
            } else {
                false
            },
            filter_ext,
        },
        groups,
        sort,
        sort_dir,
    }
}

fn parse_group_params(raw_query: Option<&str>, filters: &filter::Set) -> Vec<String> {
    let mut groups = Vec::new();
    for pair in raw_query
        .unwrap_or("")
        .split('&')
        .filter(|pair| !pair.is_empty())
    {
        let (key, value) = pair
            .split_once('=')
            .map_or((pair, ""), |(key, value)| (key, value));
        if key == "group" {
            groups.push(decode_query_value(value));
        }
    }
    filters.normalize_groups(&groups)
}

fn decode_query_value(raw: &str) -> String {
    let raw = raw.replace('+', " ");
    percent_encoding::percent_decode_str(&raw)
        .decode_utf8_lossy()
        .into_owned()
}

fn parse_bool_param(raw: &str) -> Option<bool> {
    match raw {
        "1" | "true" => Some(true),
        "0" | "false" => Some(false),
        _ => None,
    }
}

pub(crate) fn with_view(href: &str, view: &ViewState, cfg: &ViewConfig) -> String {
    let (base, fragment) = href.split_once('#').map_or((href, ""), |(a, b)| (a, b));
    let (path, query) = base.split_once('?').map_or((base, ""), |(a, b)| (a, b));
    let mut pairs = parse_query_pairs(query);
    set_bool_param(
        &mut pairs,
        "hidden",
        view.opts.show_hidden,
        cfg.default.show_hidden,
    );
    if cfg.can_toggle_excludes {
        set_bool_param(
            &mut pairs,
            "excludes",
            view.opts.show_excludes,
            cfg.default.show_excludes,
        );
    } else {
        pairs.retain(|(key, _)| key != "excludes");
    }
    set_bool_param(
        &mut pairs,
        "filter",
        view.opts.filter_ext,
        cfg.default.filter_ext,
    );
    set_string_param(
        &mut pairs,
        "sort",
        view.sort.as_str(),
        cfg.default_sort.as_str(),
    );
    set_string_param(
        &mut pairs,
        "dir",
        view.sort_dir.as_str(),
        view.sort.default_dir().as_str(),
    );
    set_multi_string_param(&mut pairs, "group", &view.groups, &cfg.default_groups);

    let mut out = path.to_string();
    if !pairs.is_empty() {
        out.push('?');
        out.push_str(
            &pairs
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join("&"),
        );
    }
    if !fragment.is_empty() {
        out.push('#');
        out.push_str(fragment);
    }
    out
}

fn parse_query_pairs(query: &str) -> Vec<(String, String)> {
    if query.is_empty() {
        return Vec::new();
    }
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            pair.split_once('=')
                .map_or((pair, ""), |(key, value)| (key, value))
        })
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

fn set_bool_param(pairs: &mut Vec<(String, String)>, key: &str, value: bool, default_value: bool) {
    pairs.retain(|(current, _)| current != key);
    if value != default_value {
        pairs.push((key.to_string(), if value { "1" } else { "0" }.to_string()));
    }
}

fn set_string_param(
    pairs: &mut Vec<(String, String)>,
    key: &str,
    value: &str,
    default_value: &str,
) {
    pairs.retain(|(current, _)| current != key);
    if value != default_value {
        pairs.push((key.to_string(), value.to_string()));
    }
}

fn set_multi_string_param(
    pairs: &mut Vec<(String, String)>,
    key: &str,
    values: &[String],
    default_values: &[String],
) {
    pairs.retain(|(current, _)| current != key);
    if values != default_values {
        for value in values {
            pairs.push((key.to_string(), value.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn group_filters() -> filter::Set {
        let mut groups = BTreeMap::new();
        groups.insert(
            "docs".to_string(),
            crate::config::FilterGroupConfig {
                label: Some("Docs".to_string()),
                globs: vec!["*.md".to_string()],
            },
        );
        groups.insert(
            "web".to_string(),
            crate::config::FilterGroupConfig {
                label: Some("Web".to_string()),
                globs: vec!["*.html".to_string()],
            },
        );
        filter::Set::resolve(&crate::config::FilterConfig {
            enabled: Some(false),
            default_group: Some("docs".to_string()),
            groups,
        })
        .unwrap()
    }

    #[test]
    fn parse_group_params_accepts_repeated_keys() {
        let filters = group_filters();
        let groups = parse_group_params(Some("filter=1&group=docs&group=web"), &filters);
        assert_eq!(groups, vec!["docs".to_string(), "web".to_string()]);
    }

    #[test]
    fn with_view_omits_default_flags() {
        let cfg = ViewConfig {
            default: ViewOpts {
                show_hidden: false,
                show_excludes: true,
                filter_ext: false,
            },
            default_groups: Vec::new(),
            default_sort: walk::Sort::Name,
            can_toggle_excludes: true,
        };
        let view = ViewState {
            opts: cfg.default,
            groups: Vec::new(),
            sort: cfg.default_sort,
            sort_dir: cfg.default_sort.default_dir(),
        };
        assert_eq!(with_view("/", &view, &cfg), "/");
    }

    #[test]
    fn with_view_preserves_non_default_flags() {
        let cfg = ViewConfig {
            default: ViewOpts {
                show_hidden: false,
                show_excludes: true,
                filter_ext: false,
            },
            default_groups: Vec::new(),
            default_sort: walk::Sort::Name,
            can_toggle_excludes: true,
        };
        let view = ViewState {
            opts: ViewOpts {
                show_hidden: true,
                show_excludes: false,
                filter_ext: true,
            },
            groups: Vec::new(),
            sort: walk::Sort::Timestamp,
            sort_dir: walk::Sort::Timestamp.default_dir(),
        };
        assert_eq!(
            with_view("/docs/", &view, &cfg),
            "/docs/?hidden=1&excludes=0&filter=1&sort=timestamp"
        );
    }

    #[test]
    fn with_view_preserves_selected_groups() {
        let filters = group_filters();
        let cfg = ViewConfig {
            default: ViewOpts {
                show_hidden: false,
                show_excludes: false,
                filter_ext: false,
            },
            default_groups: filters.default_groups().to_vec(),
            default_sort: walk::Sort::Name,
            can_toggle_excludes: false,
        };
        let view = ViewState {
            opts: ViewOpts {
                show_hidden: false,
                show_excludes: false,
                filter_ext: true,
            },
            groups: vec!["docs".to_string(), "web".to_string()],
            sort: walk::Sort::Name,
            sort_dir: walk::Sort::Name.default_dir(),
        };

        assert_eq!(
            with_view("/docs/", &view, &cfg),
            "/docs/?filter=1&group=docs&group=web"
        );
    }
}
