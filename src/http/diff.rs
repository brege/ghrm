use crate::explorer::view::ViewState;
use crate::http::delivery;
use crate::http::server::AppState;
use std::path::Path;

#[cfg(feature = "repo")]
use crate::explorer::view::{self, ViewConfig, ViewQuery};
#[cfg(feature = "repo")]
use crate::http::server::{HtmxContext, page_crumbs};
#[cfg(feature = "repo")]
use crate::http::{shell, vendor};
#[cfg(feature = "repo")]
use crate::query;
#[cfg(feature = "repo")]
use crate::render::Rendered;
#[cfg(feature = "repo")]
use crate::repo::diff::{
    DiffOutcome, DiffSpec, DiffTarget, INDEX, Row, RowKind, WORKTREE, unified_diff,
};
#[cfg(feature = "repo")]
use crate::repo::refs::{RefList, refs_for};
#[cfg(feature = "repo")]
use crate::tmpl;
#[cfg(feature = "repo")]
use anyhow::Result;
#[cfg(feature = "repo")]
use axum::{
    body::Body,
    extract::{Query, RawQuery, State},
    http::{StatusCode, header},
    response::Response,
};
#[cfg(feature = "repo")]
use percent_encoding::{AsciiSet, CONTROLS, NON_ALPHANUMERIC, utf8_percent_encode};
#[cfg(feature = "repo")]
use serde::Deserialize;
#[cfg(feature = "repo")]
use tracing::warn;

#[cfg(feature = "repo")]
pub(crate) fn spec_from_query(raw_query: Option<&str>) -> Option<DiffSpec> {
    query::parse_pairs(raw_query.unwrap_or(""))
        .iter()
        .find(|(name, _)| name.as_str() == "diff")
        .and_then(|(_, value)| DiffSpec::parse(value))
}

// The compare form submits base and head; the server canonicalizes that
// spelling into the single rendered URL grammar with a redirect, so
// diff=<base>..<head> stays the only public state.
#[cfg(feature = "repo")]
pub(crate) fn canonical_query(raw_query: Option<&str>) -> Option<String> {
    let mut pairs = query::parse_pairs(raw_query.unwrap_or(""));
    if pairs.iter().any(|(name, _)| name.as_str() == "diff") {
        return None;
    }
    let value = |key: &str| {
        pairs
            .iter()
            .find(|(name, _)| name.as_str() == key)
            .map(|(_, value)| value.clone())
    };
    let base = DiffTarget::parse(&value("base")?)?;
    let head = DiffTarget::parse(&value("head")?)?;
    let spec = DiffSpec { base, head };
    pairs.retain(|(name, _)| name.as_str() != "base" && name.as_str() != "head");
    pairs.push(("diff".to_string(), spec.token()));
    Some(query::encode_pairs(&pairs))
}

// A native GET submission replaces the query string with the form fields,
// so the form must re-submit the current explorer view state; the pairs
// come from the same contract that builds view-preserving links.
#[cfg(feature = "repo")]
pub(crate) fn view_pairs(view: &ViewState, cfg: &ViewConfig) -> Vec<(String, String)> {
    let href = view::with_view("/", view, cfg);
    let query = href.split_once('?').map(|(_, query)| query).unwrap_or("");
    query::parse_pairs(query)
}

#[cfg(feature = "repo")]
pub(crate) fn file_view_attrs(
    s: &AppState,
    path: &Path,
    rel: &str,
    file_view: delivery::FileView,
    view: &ViewState,
) -> String {
    if s.repos.repo_for(path).is_none() {
        return delivery::file_view_attrs(rel, file_view);
    }
    compare_view_attrs(rel, file_view, None, &view_pairs(view, &s.view_cfg))
}

#[cfg(not(feature = "repo"))]
pub(crate) fn file_view_attrs(
    _: &AppState,
    _: &Path,
    rel: &str,
    file_view: delivery::FileView,
    _: &ViewState,
) -> String {
    delivery::file_view_attrs(rel, file_view)
}

#[cfg(feature = "repo")]
fn compare_view_attrs(
    rel: &str,
    view: delivery::FileView,
    spec: Option<&DiffSpec>,
    pairs: &[(String, String)],
) -> String {
    let path = utf8_percent_encode(rel.trim_matches('/'), NON_ALPHANUMERIC);
    let mut url = format!("/_ghrm/compare?path={path}");
    if !pairs.is_empty() {
        url.push('&');
        url.push_str(&query::encode_pairs(pairs));
    }

    let mut attrs = delivery::file_view_attrs(rel, view);
    attrs.push_str(&format!(
        " data-ghrm-compare-url=\"{}\"",
        html_escape::encode_double_quoted_attribute(&url),
    ));
    if let Some(spec) = spec {
        attrs.push_str(&format!(
            " data-ghrm-diff=\"{}\"",
            html_escape::encode_double_quoted_attribute(&spec.token()),
        ));
    }
    attrs
}

// The HTTP path keeps slash separators while each decoded filesystem
// segment is encoded with the WHATWG special-path segment set.
#[cfg(feature = "repo")]
const FILE_SEGMENT: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'/')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'\\')
    .add(b'}');

#[cfg(feature = "repo")]
fn file_action(rel: &str) -> String {
    let encoded = rel
        .trim_matches('/')
        .split('/')
        .map(|segment| utf8_percent_encode(segment, FILE_SEGMENT).to_string())
        .collect::<Vec<_>>()
        .join("/");
    format!("/{encoded}")
}

#[cfg(feature = "repo")]
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
    let base = root
        .or_else(|| path.parent())
        .expect("served files have a parent directory");
    let rel = path
        .strip_prefix(base)
        .expect("file dispatch passes a path under its serve root")
        .to_string_lossy()
        .replace('\\', "/");

    let task_spec = spec.clone();
    let (outcome, refs) = tokio::task::spawn_blocking(move || {
        (
            unified_diff(&repo_root, &task_spec, &repo_rel),
            refs_for(&repo_root, &repo_rel),
        )
    })
    .await
    .expect("join blocking diff task");
    let Some(refs) = refs else {
        warn!("malformed git ref listing");
        return Some(internal_error());
    };

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

#[cfg(feature = "repo")]
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

#[cfg(feature = "repo")]
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
    let hidden = view_pairs(page.view, &page.s.view_cfg);
    let file_view = delivery::FileView::source();
    let view_attrs = compare_view_attrs(page.rel, file_view, Some(page.spec), &hidden);
    let action = file_action(page.rel);
    let compare_html = match compare_fragment(
        &action,
        page.spec.base.token(),
        page.spec.head.token(),
        page.refs,
        &hidden,
    ) {
        Ok(html) => html,
        Err(e) => {
            warn!("compare template error: {}", e);
            return internal_error();
        }
    };
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
            return internal_error();
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

#[cfg(feature = "repo")]
#[derive(Deserialize)]
pub(crate) struct CompareQuery {
    path: Option<String>,
    diff: Option<String>,
    #[serde(flatten)]
    view: ViewQuery,
}

#[cfg(feature = "repo")]
pub(crate) async fn compare(
    State(s): State<AppState>,
    RawQuery(raw_query): RawQuery,
    Query(q): Query<CompareQuery>,
) -> Response {
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
        .expect("join blocking ref listing task");
    let Some(refs) = refs else {
        warn!("malformed git ref listing");
        return internal_error();
    };

    let view = view::from_query(&q.view, raw_query.as_deref(), &s.view_cfg, &s.filters);
    let hidden = view_pairs(&view, &s.view_cfg);
    let action = file_action(&rel);
    match compare_fragment(&action, &base, &head, &refs, &hidden) {
        Ok(html) => html_response(html),
        Err(e) => {
            warn!("compare template error: {}", e);
            internal_error()
        }
    }
}

#[cfg(feature = "repo")]
fn compare_fragment(
    action: &str,
    base: &str,
    head: &str,
    refs: &RefList,
    hidden: &[(String, String)],
) -> Result<String> {
    let map_refs = |entries: &[crate::repo::refs::RefEntry]| {
        entries
            .iter()
            .map(|entry| tmpl::CompareRef {
                value: entry.value.clone(),
                label: entry.label.clone(),
                timestamp: entry.timestamp,
            })
            .collect::<Vec<_>>()
    };
    let branches = map_refs(&refs.branches);
    let tags = map_refs(&refs.tags);
    let commits = refs
        .commits
        .iter()
        .map(|commit| tmpl::CompareCommit {
            value: commit.value.clone(),
            label: commit.label.clone(),
            subject: commit.subject.clone(),
            timestamp: commit.timestamp,
        })
        .collect::<Vec<_>>();
    let base_label = ref_label(base, refs);
    let head_label = ref_label(head, refs);
    tmpl::compare(tmpl::CompareCtx {
        action,
        base,
        head,
        base_label: &base_label,
        head_label: &head_label,
        base_extra: unlisted(base, refs),
        head_extra: unlisted(head, refs),
        hidden,
        branches: &branches,
        tags: &tags,
        commits: &commits,
        head_timestamp: refs.head_timestamp,
    })
}

#[cfg(feature = "repo")]
fn ref_label(token: &str, refs: &RefList) -> String {
    match token {
        WORKTREE => return "Working tree".to_string(),
        INDEX => return "Staged".to_string(),
        "HEAD" => return "HEAD".to_string(),
        _ => {}
    }
    if let Some(entry) = refs
        .branches
        .iter()
        .chain(&refs.tags)
        .find(|entry| entry.value == token)
    {
        return entry.label.clone();
    }
    if let Some(commit) = refs.commits.iter().find(|commit| commit.value == token) {
        return format!("{} {}", commit.label, commit.subject);
    }
    token.to_string()
}

// A hand-typed revision like HEAD~2 is valid but absent from the listing;
// a synthetic first option keeps the picker displaying the URL state.
#[cfg(feature = "repo")]
fn unlisted<'a>(token: &'a str, refs: &RefList) -> Option<&'a str> {
    let listed = token == WORKTREE
        || token == INDEX
        || token == "HEAD"
        || refs.branches.iter().any(|entry| entry.value == token)
        || refs.tags.iter().any(|entry| entry.value == token)
        || refs.commits.iter().any(|entry| entry.value == token);
    (!listed).then_some(token)
}

#[cfg(feature = "repo")]
fn raw_html(outcome: &DiffOutcome, spec: &DiffSpec) -> String {
    match outcome {
        DiffOutcome::Patch(patch) => delivery::raw_blob_html(
            &patch.text,
            Some("diff"),
            Some(&diff_rows_attr(&patch.rows)),
        ),
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

// Serializes the producer's typed gutter rows into `old,new,kind` cells
// joined by ';'; the browser deserializes them to render the diff gutter
// without re-parsing the patch. Meta and context lines carry no tint.
#[cfg(feature = "repo")]
fn diff_rows_attr(rows: &[Row]) -> String {
    rows.iter()
        .map(|row| {
            let old = row.old.map(|line| line.to_string()).unwrap_or_default();
            let new = row.new.map(|line| line.to_string()).unwrap_or_default();
            let kind = match row.kind {
                RowKind::Hunk => "h",
                RowKind::Addition => "a",
                RowKind::Deletion => "d",
                RowKind::Meta | RowKind::Context => "",
            };
            format!("{old},{new},{kind}")
        })
        .collect::<Vec<_>>()
        .join(";")
}

#[cfg(feature = "repo")]
fn html_response(html: String) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(html))
        .unwrap()
}

#[cfg(feature = "repo")]
fn not_found() -> Response {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from("404"))
        .unwrap()
}

#[cfg(feature = "repo")]
fn internal_error() -> Response {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from("internal error"))
        .unwrap()
}

#[cfg(all(test, feature = "repo"))]
mod tests {
    use super::*;
    use crate::explorer::column;
    use crate::explorer::view::ViewConfig;
    use crate::explorer::walk::{NavSet, Sort, ViewOpts};
    #[cfg(feature = "archive")]
    use crate::http::archive;
    use crate::http::server::Mode;
    use crate::repo::RepoSet;
    use crate::repo::diff::Patch;
    use crate::repo::refs::{CommitEntry, RefEntry};
    use crate::runtime;
    use crate::testutil::TempDir;
    use axum::body::to_bytes;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Arc, RwLock};
    use tokio::sync::broadcast;

    fn write_git_config(root: &Path) {
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(
            root.join(".git/config"),
            "[core]\nrepositoryformatversion = 0\n",
        )
        .unwrap();
    }

    fn view_config() -> ViewConfig {
        ViewConfig {
            default: ViewOpts {
                show_hidden: false,
                show_excludes: false,
                filter_ext: false,
            },
            default_use_ignore: true,
            default_groups: Vec::new(),
            default_sort: Sort::Name,
            default_columns: column::Set::from_defaults(|def| def.default_visible),
            can_toggle_excludes: false,
        }
    }

    fn view_state() -> ViewState {
        let cfg = view_config();
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

    fn app_state(target: &Path) -> AppState {
        AppState {
            target: target.to_path_buf(),
            mode: Mode::Dir,
            nav: Arc::new(RwLock::new(NavSet::default())),
            alternate_nav: Arc::new(RwLock::new(None)),
            repos: RepoSet::discover(target, &[]),
            reload: broadcast::channel(4).0,
            use_ignore: true,
            show_excludes: false,
            view_cfg: view_config(),
            filter_exts: Vec::new(),
            filters: crate::testutil::group_filters(),
            exclude_names: Vec::new(),
            #[cfg(feature = "archive")]
            archive_jobs: archive::ArchiveJobs::new().unwrap(),
            search_max_rows: 10,
            home: None,
            runtime_paths: runtime::Paths::new(target, None).unwrap(),
            #[cfg(feature = "stats")]
            stats: crate::stat::Config::default(),
            auth: None,
            #[cfg(feature = "gist")]
            gist: None,
            #[cfg(feature = "edit")]
            edit: false,
        }
    }

    fn sample_refs() -> RefList {
        RefList {
            branches: vec![RefEntry {
                value: "refs/heads/main".to_string(),
                label: "main".to_string(),
                timestamp: Some(1723600000),
            }],
            tags: Vec::new(),
            commits: vec![CommitEntry {
                value: "d888e48aaaa".to_string(),
                label: "d888e48".to_string(),
                timestamp: 1723600000,
                subject: "chore: upgrade benchmark tooling".to_string(),
            }],
            head_timestamp: Some(1723600000),
        }
    }

    async fn response_text(response: Response) -> String {
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    #[test]
    fn spec_from_query_reads_only_the_diff_param() {
        let spec = spec_from_query(Some("diff=HEAD..%3Aworktree")).unwrap();
        assert_eq!(spec.token(), "HEAD..:worktree");

        assert!(spec_from_query(None).is_none());
        assert!(spec_from_query(Some("hidden=1")).is_none());
        assert!(spec_from_query(Some("base=HEAD&head=%3Aworktree")).is_none());
        assert!(spec_from_query(Some("diff=--cached..HEAD")).is_none());
    }

    #[test]
    fn canonical_query_rewrites_the_form_spelling() {
        let canonical = canonical_query(Some("hidden=1&base=HEAD&head=%3Aworktree")).unwrap();
        assert_eq!(canonical, "hidden=1&diff=HEAD..%3Aworktree");
    }

    #[test]
    fn canonical_query_leaves_canonical_and_partial_input_alone() {
        assert!(canonical_query(Some("diff=HEAD..%3Aworktree")).is_none());
        assert!(canonical_query(Some("diff=a..b&base=c&head=d")).is_none());
        assert!(canonical_query(Some("base=HEAD")).is_none());
        assert!(canonical_query(Some("base=HEAD&head=--cached")).is_none());
        assert!(canonical_query(None).is_none());
    }

    #[test]
    fn compare_form_action_encodes_path_segments() {
        let action = file_action("/docs/caf\u{e9} ?#%\\.md/");
        let html =
            compare_fragment(&action, "HEAD", ":worktree", &RefList::default(), &[]).unwrap();

        assert!(html.contains(r#"action="/docs/caf%C3%A9%20%3F%23%25%5C.md""#,));
    }

    #[test]
    fn compare_view_attrs_include_fragment_url() {
        let attrs = compare_view_attrs("docs/read me.md", delivery::FileView::source(), None, &[]);

        assert!(
            attrs.contains(r#"data-ghrm-compare-url="/_ghrm/compare?path=docs%2Fread%20me%2Emd""#)
        );
        assert!(!attrs.contains("data-ghrm-diff"));
    }

    #[test]
    fn compare_view_attrs_preserve_view_state() {
        let pairs = vec![("hidden".to_string(), "1".to_string())];
        let attrs = compare_view_attrs("a.md", delivery::FileView::source(), None, &pairs);

        assert!(
            attrs.contains(r#"data-ghrm-compare-url="/_ghrm/compare?path=a%2Emd&amp;hidden=1""#)
        );
    }

    #[test]
    fn compare_view_attrs_include_diff_spec() {
        let spec = DiffSpec::parse("HEAD..:worktree").unwrap();
        let attrs = compare_view_attrs("a.md", delivery::FileView::source(), Some(&spec), &[]);

        assert!(attrs.contains(r#"data-ghrm-diff="HEAD..:worktree""#));
    }

    #[test]
    fn unlisted_marks_only_unknown_tokens() {
        let refs = sample_refs();
        assert_eq!(unlisted(":worktree", &refs), None);
        assert_eq!(unlisted("HEAD", &refs), None);
        assert_eq!(unlisted("refs/heads/main", &refs), None);
        assert_eq!(unlisted("d888e48aaaa", &refs), None);
        assert_eq!(unlisted("HEAD~2", &refs), Some("HEAD~2"));
    }

    #[test]
    fn raw_html_serializes_diff_rows_into_the_blob() {
        let spec = DiffSpec::parse("HEAD..:worktree").unwrap();
        let outcome = DiffOutcome::Patch(Patch {
            text: "@@ -1 +1 @@\n-a\n+b\n".to_string(),
            rows: vec![
                Row {
                    old: None,
                    new: None,
                    kind: RowKind::Hunk,
                },
                Row {
                    old: Some(1),
                    new: None,
                    kind: RowKind::Deletion,
                },
                Row {
                    old: None,
                    new: Some(1),
                    kind: RowKind::Addition,
                },
            ],
        });

        let html = raw_html(&outcome, &spec);

        assert!(html.contains("ghrm-blob"));
        assert!(html.contains(r#"class="language-diff""#));
        assert!(html.contains(r#"data-ghrm-diff-rows=",,h;1,,d;,1,a""#));
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

    #[tokio::test]
    async fn render_page_serves_patch_in_code_view() {
        let td = TempDir::new("ghrm-diff-page");
        let root = td.path().join("repo");
        write_git_config(&root);
        let file = root.join("a.md");
        fs::write(&file, "content\n").unwrap();

        let s = app_state(&root);
        let spec = DiffSpec::parse("HEAD..:worktree").unwrap();
        let outcome = DiffOutcome::Patch(Patch {
            text: "diff --git a/a.md b/a.md\n".to_string(),
            rows: vec![Row {
                old: None,
                new: None,
                kind: RowKind::Meta,
            }],
        });
        let refs = sample_refs();
        let view = view_state();

        let response = render_page(Page {
            s: &s,
            path: &file,
            root: &root,
            rel: "a.md",
            spec: &spec,
            outcome: &outcome,
            refs: &refs,
            view: &view,
            hx: HtmxContext::default(),
        });

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_text(response).await;
        assert!(body.contains("<!DOCTYPE html>"));
        assert!(body.contains(r#"data-ghrm-view-kind="source""#));
        assert!(body.contains(r#"data-ghrm-diff="HEAD..:worktree""#));
        assert!(body.contains(r#"class="language-diff""#));
        assert!(body.contains("data-ghrm-preview-pane hidden"));
        assert!(body.contains("id=\"ghrm-compare\""));
        assert!(body.contains("data-ghrm-compare-option=\"refs/heads/main\""));
    }

    #[tokio::test]
    async fn render_page_htmx_returns_fragment() {
        let td = TempDir::new("ghrm-diff-fragment");
        let root = td.path().join("repo");
        write_git_config(&root);
        let file = root.join("a.md");
        fs::write(&file, "content\n").unwrap();

        let s = app_state(&root);
        let spec = DiffSpec::parse("HEAD..:worktree").unwrap();
        let outcome = DiffOutcome::Clean;
        let refs = RefList::default();
        let view = view_state();

        let response = render_page(Page {
            s: &s,
            path: &file,
            root: &root,
            rel: "a.md",
            spec: &spec,
            outcome: &outcome,
            refs: &refs,
            view: &view,
            hx: HtmxContext { is_htmx: true },
        });

        assert!(response.headers().get("HX-Title").is_some());
        let body = response_text(response).await;
        assert!(!body.contains("<!DOCTYPE html>"));
        assert!(body.contains("hx-swap-oob"));
        assert!(body.contains("ghrm-diff-notice"));
        assert!(body.contains("No changes between HEAD and :worktree"));
    }

    #[tokio::test]
    async fn try_render_returns_none_outside_a_repository() {
        let td = TempDir::new("ghrm-diff-norepo");
        let root = td.path().join("dir");
        fs::create_dir_all(&root).unwrap();
        let file = root.join("a.md");
        fs::write(&file, "content\n").unwrap();

        let spec = DiffSpec::parse("HEAD..:worktree").unwrap();
        let view = view_state();

        let dir_state = app_state(&root);
        let dir_result = try_render(
            &dir_state,
            &file,
            Some(&root),
            &spec,
            &view,
            HtmxContext::default(),
        )
        .await;
        assert!(dir_result.is_none());

        let mut file_state = app_state(&root);
        file_state.mode = Mode::File;
        file_state.target = PathBuf::from(&file);
        let file_result = try_render(
            &file_state,
            &file,
            None,
            &spec,
            &view,
            HtmxContext::default(),
        )
        .await;
        assert!(file_result.is_none());
    }

    #[tokio::test]
    async fn compare_endpoint_requires_a_repository_backed_file() {
        let td = TempDir::new("ghrm-diff-compare-endpoint");
        let root = td.path().join("dir");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("a.md"), "content\n").unwrap();
        let s = app_state(&root);

        let outside = compare(
            State(s.clone()),
            RawQuery(None),
            Query(CompareQuery {
                path: Some("a.md".to_string()),
                diff: None,
                view: ViewQuery::default(),
            }),
        )
        .await;
        assert_eq!(outside.status(), StatusCode::NOT_FOUND);

        let missing = compare(
            State(s),
            RawQuery(None),
            Query(CompareQuery {
                path: Some("missing.md".to_string()),
                diff: None,
                view: ViewQuery::default(),
            }),
        )
        .await;
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn view_pairs_reflect_non_default_state() {
        let cfg = view_config();
        let mut view = view_state();
        view.opts.show_hidden = true;

        let pairs = view_pairs(&view, &cfg);

        assert_eq!(pairs, vec![("hidden".to_string(), "1".to_string())]);
        assert!(view_pairs(&view_state(), &cfg).is_empty());
    }

    #[test]
    fn form_view_state_survives_canonicalization() {
        let cfg = view_config();
        let mut view = view_state();
        view.opts.show_hidden = true;
        let mut submission = view_pairs(&view, &cfg);
        submission.push(("base".to_string(), "HEAD".to_string()));
        submission.push(("head".to_string(), ":worktree".to_string()));

        let canonical = canonical_query(Some(&query::encode_pairs(&submission))).unwrap();

        assert!(canonical.contains("hidden=1"));
        assert!(canonical.contains("diff=HEAD..%3Aworktree"));
    }
}
