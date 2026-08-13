use crate::explorer::view::ViewState;
use crate::http::server::{AppState, HtmxContext, page_crumbs};
use crate::http::{delivery, shell, vendor};
use crate::query;
use crate::render::Rendered;
use crate::repo::diff::{DiffOutcome, DiffSpec, DiffTarget, WORKTREE, unified_diff};
use crate::repo::refs::{RefList, refs_for};
use crate::tmpl;

use axum::{
    body::Body,
    extract::{Query, State},
    http::{StatusCode, header},
    response::Response,
};
use serde::Deserialize;
use std::path::Path;
use tracing::warn;

pub(crate) fn spec_from_query(raw_query: Option<&str>) -> Option<DiffSpec> {
    let pairs = query::parse_pairs(raw_query.unwrap_or(""));
    let value = |key: &str| {
        pairs
            .iter()
            .find(|(name, _)| name.as_str() == key)
            .map(|(_, value)| value.as_str())
    };
    if let Some(raw) = value("diff") {
        return DiffSpec::parse(raw);
    }
    let base = DiffTarget::parse(value("base")?)?;
    let head = DiffTarget::parse(value("head")?)?;
    Some(DiffSpec { base, head })
}

pub(crate) async fn try_render(
    s: &AppState,
    path: &Path,
    root: Option<&Path>,
    spec: &DiffSpec,
    view: &ViewState,
    hx: HtmxContext,
) -> Option<Response> {
    let (repo_root, repo_rel) = s.repos.repo_for(path)?;
    let repo_root = repo_root.to_path_buf();
    let base = root.or_else(|| path.parent())?;
    let rel = path
        .strip_prefix(base)
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
        .or_else(|| path.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default();

    let task_spec = spec.clone();
    let (outcome, refs) = tokio::task::spawn_blocking(move || {
        (
            unified_diff(&repo_root, &task_spec, &repo_rel),
            refs_for(&repo_root, &repo_rel),
        )
    })
    .await
    .unwrap_or_else(|_| {
        (
            DiffOutcome::Failed("diff task failed".to_string()),
            RefList::default(),
        )
    });

    Some(render_page(Page {
        s,
        path,
        root: base,
        rel: &rel,
        spec,
        outcome: &outcome,
        refs: &refs,
        view,
        hx,
    }))
}

struct Page<'a> {
    s: &'a AppState,
    path: &'a Path,
    root: &'a Path,
    rel: &'a str,
    spec: &'a DiffSpec,
    outcome: &'a DiffOutcome,
    refs: &'a RefList,
    view: &'a ViewState,
    hx: HtmxContext,
}

fn render_page(page: Page<'_>) -> Response {
    let filename = page
        .path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let rendered = Rendered {
        html: String::new(),
        title: format!("{filename} ({})", page.spec.token()),
        langs: vec!["diff".to_string()],
        lang: Some("diff".to_string()),
        has_mermaid: false,
        has_math: false,
        has_map: false,
    };
    let features = vendor::feature_list(&rendered);
    let crumbs = page_crumbs(page.s, page.path, page.root, page.rel, page.view);
    let raw_html = raw_html(page.outcome, page.spec);
    let compare = delivery::compare_attrs(page.rel, Some(page.spec));
    let file_view = delivery::FileView::source();
    let view_attrs = delivery::file_view_attrs(page.rel, file_view, Some(&compare));
    let compare_html = compare_html(&page);
    let body = match tmpl::page(tmpl::PageCtx {
        features: &features,
        crumbs: &crumbs,
        preview_html: "",
        raw_html: &raw_html,
        view_attrs: &view_attrs,
        compare_html: &compare_html,
        preview_hidden: true,
        raw_hidden: false,
    }) {
        Ok(b) => b,
        Err(e) => {
            warn!("template error: {}", e);
            return not_found();
        }
    };
    let source = page.s.repos.source_for(page.path);
    if page.hx.is_htmx {
        return shell::fragment(
            &body,
            &rendered.title,
            None,
            source,
            &page.s.runtime_paths,
            false,
        );
    }
    shell::full_page(
        &rendered,
        &body,
        None,
        source,
        page.s.auth.is_some(),
        &page.s.runtime_paths,
        false,
    )
}

#[derive(Deserialize)]
pub(crate) struct CompareQuery {
    path: Option<String>,
    diff: Option<String>,
}

pub(crate) async fn compare(State(s): State<AppState>, Query(q): Query<CompareQuery>) -> Response {
    let rel = q
        .path
        .as_deref()
        .unwrap_or("")
        .trim_matches('/')
        .to_string();
    let Some(file_path) = delivery::resolve_internal_file(&s, &rel) else {
        return not_found();
    };
    let Some((repo_root, repo_rel)) = s.repos.repo_for(&file_path) else {
        return not_found();
    };

    let selected = q.diff.as_deref().and_then(DiffSpec::parse);
    let (base, head) = match &selected {
        Some(spec) => (spec.base.token().to_string(), spec.head.token().to_string()),
        None => ("HEAD".to_string(), WORKTREE.to_string()),
    };

    let repo_root = repo_root.to_path_buf();
    let refs = tokio::task::spawn_blocking(move || refs_for(&repo_root, &repo_rel))
        .await
        .unwrap_or_default();
    let commits = refs
        .commits
        .into_iter()
        .map(|commit| tmpl::CompareCommit {
            hash: commit.hash,
            subject: commit.subject,
            timestamp: commit.timestamp,
        })
        .collect::<Vec<_>>();

    let action = format!("/{rel}");
    match tmpl::compare(tmpl::CompareCtx {
        action: &action,
        base: &base,
        head: &head,
        branches: &refs.branches,
        tags: &refs.tags,
        commits: &commits,
    }) {
        Ok(html) => html_response(html),
        Err(e) => {
            warn!("compare template error: {}", e);
            not_found()
        }
    }
}

fn compare_html(page: &Page<'_>) -> String {
    let action = format!("/{}", page.rel.trim_matches('/'));
    let commits = page
        .refs
        .commits
        .iter()
        .map(|commit| tmpl::CompareCommit {
            hash: commit.hash.clone(),
            subject: commit.subject.clone(),
            timestamp: commit.timestamp,
        })
        .collect::<Vec<_>>();
    match tmpl::compare(tmpl::CompareCtx {
        action: &action,
        base: page.spec.base.token(),
        head: page.spec.head.token(),
        branches: &page.refs.branches,
        tags: &page.refs.tags,
        commits: &commits,
    }) {
        Ok(html) => html,
        Err(e) => {
            warn!("compare template error: {}", e);
            String::new()
        }
    }
}

fn raw_html(outcome: &DiffOutcome, spec: &DiffSpec) -> String {
    match outcome {
        DiffOutcome::Patch(patch) => delivery::raw_blob_html(patch, Some("diff")),
        DiffOutcome::Clean => format!(
            "<div class=\"ghrm-diff-notice\">No changes between {base} and {head} for this file.</div>",
            base = html_escape::encode_text(spec.base.token()),
            head = html_escape::encode_text(spec.head.token()),
        ),
        DiffOutcome::Failed(reason) => format!(
            "<div class=\"ghrm-diff-notice ghrm-diff-error\">Cannot compare {spec}: {reason}</div>",
            spec = html_escape::encode_text(&spec.token()),
            reason = html_escape::encode_text(reason),
        ),
    }
}

fn html_response(html: String) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(html))
        .unwrap()
}

fn not_found() -> Response {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from("404"))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_from_query_reads_diff_param() {
        let spec = spec_from_query(Some("diff=HEAD..%3Aworktree")).unwrap();
        assert_eq!(spec.token(), "HEAD..:worktree");
    }

    #[test]
    fn spec_from_query_reads_base_and_head_pair() {
        let spec = spec_from_query(Some("base=%3Aindex&head=%3Aworktree")).unwrap();
        assert_eq!(spec.token(), ":index..:worktree");
    }

    #[test]
    fn spec_from_query_prefers_diff_param_over_pair() {
        let spec = spec_from_query(Some("diff=a..b&base=c&head=d")).unwrap();
        assert_eq!(spec.token(), "a..b");
    }

    #[test]
    fn spec_from_query_rejects_partial_or_invalid_input() {
        assert!(spec_from_query(None).is_none());
        assert!(spec_from_query(Some("hidden=1")).is_none());
        assert!(spec_from_query(Some("base=HEAD")).is_none());
        assert!(spec_from_query(Some("diff=--cached..HEAD")).is_none());
    }

    #[test]
    fn raw_html_wraps_patch_as_diff_blob() {
        let spec = DiffSpec::parse("HEAD..:worktree").unwrap();
        let outcome = DiffOutcome::Patch("diff --git a/x b/x\n".to_string());

        let html = raw_html(&outcome, &spec);

        assert!(html.contains("ghrm-blob"));
        assert!(html.contains(r#"class="language-diff""#));
    }

    #[test]
    fn raw_html_reports_clean_and_failed_states() {
        let spec = DiffSpec::parse("HEAD..:index").unwrap();

        let clean = raw_html(&DiffOutcome::Clean, &spec);
        assert!(clean.contains("ghrm-diff-notice"));
        assert!(clean.contains("No changes between HEAD and :index"));

        let failed = raw_html(&DiffOutcome::Failed("bad <ref>".to_string()), &spec);
        assert!(failed.contains("ghrm-diff-error"));
        assert!(failed.contains("bad &lt;ref&gt;"));
    }
}
