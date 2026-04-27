use crate::auth;
use crate::filter;
use crate::render::{self, Rendered};
use crate::repo::{RepoSet, SourceState};
use crate::search;
use crate::tmpl::{self, ExplorerCtx, ExplorerEntry, ExplorerReadme, PageShell};
use crate::walk::{self, NavSet, ViewOpts};
use crate::watch;

use anyhow::Result;
use anyhow::anyhow;
use axum::{
    Router,
    body::Body,
    extract::{Path as AxPath, Query, RawQuery, State, ws::WebSocketUpgrade},
    http::{StatusCode, header},
    middleware,
    response::{Html, IntoResponse, Response},
    routing::get,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, RwLock};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tracing::{info, warn};

#[derive(Clone)]
pub struct AppState {
    pub target: PathBuf,
    pub mode: Mode,
    pub nav: Arc<RwLock<NavSet>>,
    pub repos: RepoSet,
    pub reload: broadcast::Sender<()>,
    pub use_ignore: bool,
    pub view_cfg: ViewConfig,
    pub filter_exts: Vec<String>,
    pub filters: filter::Set,
    pub exclude_names: Vec<String>,
    pub search_max_rows: usize,
    pub home: Option<PathBuf>,
    pub auth: Option<Arc<auth::AuthState>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    File,
    Dir,
}

pub struct Options {
    pub bind: String,
    pub port: u16,
    pub open: bool,
    pub target: PathBuf,
    pub use_ignore: bool,
    pub default_hidden: bool,
    pub default_filter_ext: bool,
    pub extensions: Vec<String>,
    pub filters: filter::Set,
    pub exclude_names: Vec<String>,
    pub no_excludes: bool,
    pub search_max_rows: usize,
    pub auth: Option<auth::AuthConfig>,
}

#[derive(Clone, Copy)]
struct FileView {
    kind: &'static str,
    preview_hidden: bool,
    raw_hidden: bool,
}

impl FileView {
    fn markdown() -> Self {
        Self {
            kind: "markdown",
            preview_hidden: false,
            raw_hidden: true,
        }
    }

    fn raw() -> Self {
        Self {
            kind: "raw",
            preview_hidden: true,
            raw_hidden: false,
        }
    }
}

#[derive(Clone)]
pub(crate) struct ViewConfig {
    default: ViewOpts,
    default_groups: Vec<String>,
    default_sort: walk::Sort,
    can_toggle_excludes: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ViewState {
    opts: ViewOpts,
    groups: Vec<String>,
    sort: walk::Sort,
    sort_dir: walk::SortDir,
}

fn view_matcher(view: &ViewState, filters: &filter::Set) -> Option<filter::Matcher> {
    if !view.opts.filter_ext || view.groups.is_empty() {
        return None;
    }
    filters.matcher_for_groups(&view.groups).ok().flatten()
}

fn view_filter_exts<'a>(view: &ViewState, filter_exts: &'a [String]) -> Option<&'a [String]> {
    if view.opts.filter_ext && view.groups.is_empty() {
        Some(filter_exts)
    } else {
        None
    }
}

pub async fn run(options: Options) -> Result<()> {
    let Options {
        bind,
        port,
        open,
        target,
        use_ignore,
        default_hidden,
        default_filter_ext,
        extensions,
        filters,
        exclude_names,
        no_excludes,
        search_max_rows,
        auth,
    } = options;

    let meta = std::fs::metadata(&target)?;
    let mode = if meta.is_dir() { Mode::Dir } else { Mode::File };

    let (reload_tx, _) = broadcast::channel::<()>(32);
    let nav = Arc::new(RwLock::new(NavSet::default()));
    let auth = auth
        .map(|auth| auth::AuthState::new(auth, port))
        .transpose()?
        .map(Arc::new);
    let view_cfg = ViewConfig {
        default: ViewOpts {
            show_hidden: default_hidden,
            show_excludes: no_excludes,
            filter_ext: default_filter_ext,
        },
        default_groups: filters.default_groups().to_vec(),
        default_sort: walk::Sort::Name,
        can_toggle_excludes: no_excludes,
    };
    let repo_root_buf = if mode == Mode::Dir {
        target.clone()
    } else {
        target.parent().unwrap_or(&target).to_path_buf()
    };

    let repos = match mode {
        Mode::Dir => {
            let repo_root2 = repo_root_buf.clone();
            let repo_excludes = exclude_names.clone();
            let repo_h =
                tokio::task::spawn_blocking(move || RepoSet::discover(&repo_root2, &repo_excludes));

            // Build nav tree in background - don't block startup
            let nav_bg = nav.clone();
            let target_bg = target.clone();
            let walk_excludes = exclude_names.clone();
            let walk_extensions = extensions.clone();
            tokio::task::spawn_blocking(move || {
                let fresh = walk::build_all(
                    &target_bg,
                    use_ignore,
                    &walk_excludes,
                    &walk_extensions,
                    no_excludes,
                );
                if let Ok(mut guard) = nav_bg.write() {
                    *guard = fresh;
                }
            });

            // Watcher failure shouldn't kill the server
            if let Err(e) = watch::spawn_dir(
                target.clone(),
                nav.clone(),
                reload_tx.clone(),
                use_ignore,
                exclude_names.clone(),
                extensions.clone(),
                no_excludes,
            ) {
                warn!("file watcher disabled: {e}");
            }
            repo_h.await?
        }
        Mode::File => {
            if let Err(e) = watch::spawn_file(target.clone(), reload_tx.clone()) {
                warn!("file watcher disabled: {e}");
            }
            let repo_excludes = exclude_names.clone();
            tokio::task::spawn_blocking(move || RepoSet::discover(&repo_root_buf, &repo_excludes))
                .await?
        }
    };

    let state = AppState {
        target: target.clone(),
        mode,
        nav,
        repos,
        reload: reload_tx,
        use_ignore,
        view_cfg,
        filter_exts: extensions,
        filters,
        exclude_names,
        search_max_rows,
        home: std::env::var_os("HOME").map(PathBuf::from),
        auth,
    };

    let protected = Router::new()
        .route("/", get(root))
        .route("/_ghrm/ws", get(ws_handler))
        .route("/_ghrm/tree", get(api_tree))
        .route("/_ghrm/path-search", get(api_path_search))
        .route("/_ghrm/search", get(api_search))
        .route("/_ghrm/render", get(api_render))
        .route("/_ghrm/raw/{*path}", get(raw_file))
        .route("/_ghrm/html/{*path}", get(html_file))
        .route("/_ghrm/download/{*path}", get(download_file))
        .route("/{*path}", get(any_path));

    let app = if state.auth.is_some() {
        Router::new()
            .route(
                "/_ghrm/login",
                get(auth::login_form).post(auth::login_submit),
            )
            .route("/_ghrm/logout", get(auth::logout))
            .route("/_ghrm/assets/{*path}", get(theme_asset))
            .route("/vendor/{*path}", get(vendor))
            .merge(protected.layer(middleware::from_fn_with_state(state.clone(), auth::require)))
            .with_state(state)
    } else {
        Router::new()
            .route("/_ghrm/assets/{*path}", get(theme_asset))
            .route("/vendor/{*path}", get(vendor))
            .merge(protected)
            .with_state(state)
    };

    let addr = find_addr(&bind, port).await?;
    let listener = TcpListener::bind(addr).await?;
    let actual = listener.local_addr()?;
    info!(%actual, "ghrm listening");
    if open {
        open_browser(&actual);
    }
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

async fn find_addr(bind: &str, start_port: u16) -> Result<SocketAddr> {
    for port in start_port..start_port.saturating_add(50) {
        let addr: SocketAddr = format!("{}:{}", bind, port).parse()?;
        match TcpListener::bind(addr).await {
            Ok(l) => {
                drop(l);
                return Ok(addr);
            }
            Err(_) => continue,
        }
    }
    Err(anyhow!("no free port in range"))
}

fn open_browser(addr: &SocketAddr) {
    let url = format!("http://{}/", addr);
    let _ = std::process::Command::new("xdg-open")
        .arg(&url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

#[derive(Default, Deserialize)]
struct ViewQuery {
    hidden: Option<String>,
    excludes: Option<String>,
    filter: Option<String>,
    sort: Option<String>,
    dir: Option<String>,
}

fn view_from_query(
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

fn with_view(href: &str, view: &ViewState, cfg: &ViewConfig) -> String {
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

fn breadcrumb_html(
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
        out.push_str(&html_escape::encode_double_quoted_attribute(&with_view(
            &href, view, cfg,
        )));
        out.push_str(r#"">"#);
        out.push_str(&label);
        out.push_str("</a>");
    }

    out
}

async fn root(
    State(s): State<AppState>,
    RawQuery(raw_query): RawQuery,
    Query(q): Query<ViewQuery>,
) -> Response {
    let view = view_from_query(&q, raw_query.as_deref(), &s.view_cfg, &s.filters);
    match s.mode {
        Mode::File => render_target(&s, &s.target, None, view).await,
        Mode::Dir => render_explorer(&s, "", view).await,
    }
}

async fn any_path(
    State(s): State<AppState>,
    AxPath(path): AxPath<String>,
    RawQuery(raw_query): RawQuery,
    Query(q): Query<ViewQuery>,
) -> Response {
    let view = view_from_query(&q, raw_query.as_deref(), &s.view_cfg, &s.filters);
    if s.mode == Mode::File {
        return serve_file_mode(&s, &path, view).await;
    }
    let had_trailing = path.ends_with('/');
    let clean = path.trim_matches('/').to_string();
    let joined = if clean.is_empty() {
        s.target.clone()
    } else {
        s.target.join(&clean)
    };
    let meta = match tokio::fs::metadata(&joined).await {
        Ok(m) => m,
        Err(_) => return not_found(),
    };
    if meta.is_dir() {
        if !had_trailing {
            let loc = with_view(&format!("/{}/", clean), &view, &s.view_cfg);
            return Response::builder()
                .status(StatusCode::MOVED_PERMANENTLY)
                .header(header::LOCATION, loc)
                .body(Body::empty())
                .unwrap();
        }
        return render_explorer(&s, &clean, view).await;
    }
    if joined.extension().and_then(|s| s.to_str()) == Some("md") {
        return render_file(&s, &joined, Some(s.target.as_path()), view).await;
    }
    dispatch_file(&s, &joined, &s.target, &clean, view).await
}

async fn serve_file_mode(s: &AppState, path: &str, view: ViewState) -> Response {
    let Some(root) = s.target.parent() else {
        return not_found();
    };
    let clean = path.trim_matches('/');
    if clean.is_empty() {
        return render_target(s, &s.target, None, view).await;
    }
    let joined = root.join(clean);
    let meta = match tokio::fs::metadata(&joined).await {
        Ok(m) => m,
        Err(_) => return not_found(),
    };
    if meta.is_dir() {
        return not_found();
    }
    render_target(s, &joined, None, view).await
}

async fn render_target(
    s: &AppState,
    path: &Path,
    root: Option<&Path>,
    view: ViewState,
) -> Response {
    if path.extension().and_then(|s| s.to_str()) == Some("md") {
        render_file(s, path, root, view).await
    } else {
        let Some(base) = root.or_else(|| path.parent()) else {
            return not_found();
        };
        let rel = path
            .strip_prefix(base)
            .ok()
            .map(|p| p.to_string_lossy().into_owned())
            .or_else(|| path.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_default();
        dispatch_file(s, path, base, &rel, view).await
    }
}

async fn render_file(s: &AppState, path: &Path, root: Option<&Path>, view: ViewState) -> Response {
    let md = match tokio::fs::read_to_string(path).await {
        Ok(m) => m,
        Err(_) => return not_found(),
    };
    let Some(root) = root.or_else(|| path.parent()) else {
        return not_found();
    };
    let rendered = render::render_at(&md, Some(render::RenderPath { root, src: path }));
    let rel = path
        .strip_prefix(root)
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
        .or_else(|| path.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default();
    let crumbs = breadcrumb_html(root, s.home.as_deref(), &rel, &view, &s.view_cfg);
    let raw_html = raw_blob_html(&md, Some("markdown"));
    let view = FileView::markdown();
    let view_attrs = file_view_attrs(&rel, view);
    let body = match tmpl::page(tmpl::PageCtx {
        crumbs: &crumbs,
        preview_html: &rendered.html,
        raw_html: &raw_html,
        view_attrs: &view_attrs,
        preview_hidden: view.preview_hidden,
        raw_hidden: view.raw_hidden,
    }) {
        Ok(b) => b,
        Err(e) => {
            warn!("template error: {}", e);
            return not_found();
        }
    };
    respond_html(
        &rendered,
        &body,
        s.repos.source_for(path),
        &s.view_cfg,
        s.auth.is_some(),
    )
}

async fn render_explorer(s: &AppState, rel: &str, view: ViewState) -> Response {
    let matcher = view_matcher(&view, &s.filters);
    let filter_exts = view_filter_exts(&view, &s.filter_exts);
    let dir_opt = {
        let guard = s.nav.read().unwrap();
        guard
            .get(view.opts, view.sort, view.sort_dir, matcher.as_ref())
            .dirs
            .get(rel)
            .cloned()
    };

    let dir = match dir_opt {
        Some(d) if d.entries.is_empty() => walk::list_dir(
            &s.target,
            Path::new(rel),
            &s.exclude_names,
            filter_exts.unwrap_or(&[]),
            matcher.as_ref(),
            view.opts,
            walk::SortSpec {
                sort: view.sort,
                dir: view.sort_dir,
            },
        )
        .unwrap_or(d),
        Some(d) => d,
        None => match walk::list_dir(
            &s.target,
            Path::new(rel),
            &s.exclude_names,
            filter_exts.unwrap_or(&[]),
            matcher.as_ref(),
            view.opts,
            walk::SortSpec {
                sort: view.sort,
                dir: view.sort_dir,
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
    let parent_href = with_view(&parent_href, &view, &s.view_cfg);

    let entries: Vec<ExplorerEntry> = dir
        .entries
        .iter()
        .map(|e| ExplorerEntry {
            name: e.name.clone(),
            href: with_view(&e.href, &view, &s.view_cfg),
            is_dir: e.is_dir,
            modified: e.modified,
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
    let crumbs = breadcrumb_html(&s.target, s.home.as_deref(), rel, &view, &s.view_cfg);
    let body = match tmpl::explorer(ExplorerCtx {
        crumbs: &crumbs,
        current_path: rel,
        has_parent,
        parent_href: &parent_href,
        show_excludes: s.view_cfg.can_toggle_excludes,
        filter_groups: s.filters.groups(),
        entries: &entries,
        readme: readme_tmpl,
    }) {
        Ok(b) => b,
        Err(e) => {
            warn!("template error: {}", e);
            return not_found();
        }
    };

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

    let current = if rel.is_empty() {
        s.target.clone()
    } else {
        s.target.join(rel)
    };
    respond_html(
        &combined,
        &body,
        s.repos.source_for(&current),
        &s.view_cfg,
        s.auth.is_some(),
    )
}

fn respond_html(
    r: &Rendered,
    body: &str,
    source: SourceState,
    cfg: &ViewConfig,
    show_logout: bool,
) -> Response {
    let title = if r.title.is_empty() {
        "Preview"
    } else {
        &r.title
    };
    let source = source_html(&source);
    let shell = PageShell {
        title,
        body,
        source: &source,
        favicon: tmpl::FAVICON_SVG_URL,
        show_logout,
        default_show_hidden: cfg.default.show_hidden,
        default_show_excludes: cfg.default.show_excludes,
        default_filter_ext: cfg.default.filter_ext,
        default_filter_group: cfg.default_groups.first().map(String::as_str),
        default_sort: cfg.default_sort.as_str(),
        can_toggle_excludes: cfg.can_toggle_excludes,
        has_mermaid: r.has_mermaid,
        has_math: r.has_math,
        has_map: r.has_map,
    };
    let html = match tmpl::base(shell) {
        Ok(h) => h,
        Err(e) => {
            warn!("template error: {}", e);
            return not_found();
        }
    };
    Html(html).into_response()
}

fn source_html(source: &SourceState) -> String {
    match source {
        SourceState::Web { url, label, .. } => web_source_html(url, label),
        SourceState::Transport { raw } => format!(
            "<span id=\"ghrm-source-slot\" class=\"ghrm-source-link is-disabled\" aria-label=\"Transport-only remote\" title=\"Transport-only remote: {raw}\"><svg aria-hidden=\"true\" focusable=\"false\"><use href=\"#ghrm-icon-git\"></use></svg><span class=\"ghrm-source-text\">{text}</span></span>",
            raw = html_escape::encode_double_quoted_attribute(raw),
            text = html_escape::encode_text(raw),
        ),
        SourceState::NoRemote => "<span id=\"ghrm-source-slot\" class=\"ghrm-source-link is-disabled\" aria-label=\"Git repository has no remote\" title=\"Git repository has no remote\"><svg aria-hidden=\"true\" focusable=\"false\"><use href=\"#ghrm-icon-git\"></use></svg><span class=\"ghrm-source-text\">no remote</span></span>".to_string(),
        SourceState::NoRepo => String::new(),
    }
}

const PROJECT_REMOTE_URL: &str = "https://github.com/brege/ghrm";

fn web_source_html(url: &str, label: &str) -> String {
    let href = html_escape::encode_double_quoted_attribute(url);
    let title_attr = html_escape::encode_double_quoted_attribute(url);
    let (host, repo) = source_display(url, label);
    let host_href = if host.is_empty() {
        None
    } else {
        Some(format!("https://{host}"))
    };
    let host = html_escape::encode_text(&host);
    let repo = html_escape::encode_text(&repo);
    let project_href = html_escape::encode_double_quoted_attribute(PROJECT_REMOTE_URL);

    let host_html = match host_href {
        Some(host_href) => {
            let host_href = html_escape::encode_double_quoted_attribute(&host_href);
            format!(
                "<a class=\"ghrm-source-host\" href=\"{host_href}\" target=\"_blank\" rel=\"noopener noreferrer\" title=\"Open {host}\">{host}</a>"
            )
        }
        None => String::new(),
    };

    format!(
        "<span id=\"ghrm-source-slot\" class=\"ghrm-source-link\"><a class=\"ghrm-source-badge\" href=\"{project_href}\" target=\"_blank\" rel=\"noopener noreferrer\" aria-label=\"ghrm source code\" title=\"ghrm source code\"><svg aria-hidden=\"true\" focusable=\"false\"><use href=\"#ghrm-icon-source\"></use></svg></a><span class=\"ghrm-source-text\">{host_html}<a class=\"ghrm-source-repo\" href=\"{href}\" target=\"_blank\" rel=\"noopener noreferrer\" aria-label=\"Open source remote: {title_attr}\" title=\"Open source remote: {title_attr}\">{repo}</a></span></span>",
    )
}

fn source_display(url: &str, label: &str) -> (String, String) {
    let after_scheme = url.find("://").map_or(0, |i| i + 3);
    let host_end = after_scheme
        + url[after_scheme..]
            .find('/')
            .unwrap_or(url.len() - after_scheme);
    let host = url[after_scheme..host_end].trim_end_matches('/');
    let repo = url[host_end..].trim_matches('/');
    if host.is_empty() || repo.is_empty() {
        let repo = label.replace(" / ", "/");
        return (String::new(), repo);
    }
    (host.to_string(), repo.to_string())
}

// --- JSON APIs for optional SPA navigation ---

#[derive(Serialize)]
struct TreeResponse {
    mode: &'static str,
    root: String,
    dirs: BTreeMap<String, crate::walk::NavDir>,
}

async fn api_tree(
    State(s): State<AppState>,
    RawQuery(raw_query): RawQuery,
    Query(q): Query<ViewQuery>,
) -> Response {
    let nav = s.nav.read().unwrap();
    let view = view_from_query(&q, raw_query.as_deref(), &s.view_cfg, &s.filters);
    let matcher = view_matcher(&view, &s.filters);
    let tree = nav.get(view.opts, view.sort, view.sort_dir, matcher.as_ref());
    let root = s
        .target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let resp = TreeResponse {
        mode: if s.mode == Mode::Dir { "dir" } else { "file" },
        root,
        dirs: tree.dirs.clone(),
    };
    match serde_json::to_string(&resp) {
        Ok(json) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json))
            .unwrap(),
        Err(e) => {
            warn!("api_tree error: {}", e);
            not_found()
        }
    }
}

#[derive(Deserialize)]
struct SearchQuery {
    q: Option<String>,
    hidden: Option<u8>,
    excludes: Option<u8>,
    filter: Option<u8>,
    sort: Option<String>,
    dir: Option<String>,
}

async fn api_search(
    State(s): State<AppState>,
    RawQuery(raw_query): RawQuery,
    Query(q): Query<SearchQuery>,
) -> Response {
    let query = match q.q.as_deref() {
        Some(q) if !q.is_empty() => q,
        _ => {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"error":"missing query"}"#))
                .unwrap();
        }
    };

    let view = view_from_query(
        &ViewQuery {
            hidden: q.hidden.map(|value| value.to_string()),
            excludes: q.excludes.map(|value| value.to_string()),
            filter: q.filter.map(|value| value.to_string()),
            sort: q.sort.clone(),
            dir: q.dir.clone(),
        },
        raw_query.as_deref(),
        &s.view_cfg,
        &s.filters,
    );
    let exclude_names = if view.opts.show_excludes {
        &[][..]
    } else {
        &s.exclude_names
    };
    let matcher = view_matcher(&view, &s.filters);
    let filter_exts = view_filter_exts(&view, &s.filter_exts);

    let resp = search::search(search::SearchOpts {
        query,
        root: &s.target,
        use_ignore: s.use_ignore,
        hidden: view.opts.show_hidden,
        exclude_names,
        filter_exts,
        group_filter: matcher.as_ref(),
        max_rows: s.search_max_rows,
    });

    match serde_json::to_string(&resp) {
        Ok(json) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json))
            .unwrap(),
        Err(e) => {
            warn!("api_search error: {}", e);
            not_found()
        }
    }
}

#[derive(Serialize)]
struct PathSearchResult {
    href: String,
    display: String,
    is_dir: bool,
    modified: Option<u64>,
}

#[derive(Serialize)]
struct PathSearchResponse {
    results: Vec<PathSearchResult>,
    truncated: bool,
    max_rows: usize,
}

#[derive(Default, Deserialize)]
struct PathSearchQuery {
    q: Option<String>,
    path: Option<String>,
    #[serde(flatten)]
    view: ViewQuery,
}

async fn api_path_search(
    State(s): State<AppState>,
    RawQuery(raw_query): RawQuery,
    Query(q): Query<PathSearchQuery>,
) -> Response {
    let query = match q.q.as_deref() {
        Some(q) if !q.is_empty() => q,
        _ => {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"error":"missing query"}"#))
                .unwrap();
        }
    };

    let view = view_from_query(&q.view, raw_query.as_deref(), &s.view_cfg, &s.filters);
    let current_path = q.path.as_deref().unwrap_or("").trim_matches('/');
    let nav = s.nav.read().unwrap();
    let matcher = view_matcher(&view, &s.filters);
    let tree = nav.get(view.opts, view.sort, view.sort_dir, matcher.as_ref());
    let resp = path_search(
        &tree,
        current_path,
        query,
        s.search_max_rows,
        view.sort,
        view.sort_dir,
    );

    match serde_json::to_string(&resp) {
        Ok(json) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json))
            .unwrap(),
        Err(e) => {
            warn!("api_path_search error: {}", e);
            not_found()
        }
    }
}

fn path_search(
    tree: &walk::NavTree,
    current_path: &str,
    query: &str,
    max_rows: usize,
    sort: walk::Sort,
    dir: walk::SortDir,
) -> PathSearchResponse {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return PathSearchResponse {
            results: Vec::new(),
            truncated: false,
            max_rows,
        };
    }

    let prefix = (!current_path.is_empty()).then(|| format!("{current_path}/"));
    let mut rows: Vec<PathSearchResult> = Vec::new();

    for (dir, nav_dir) in &tree.dirs {
        if let Some(prefix) = &prefix {
            if dir != current_path && !dir.starts_with(prefix) {
                continue;
            }
        }

        let rel_dir = if dir == current_path {
            ""
        } else if let Some(prefix) = &prefix {
            dir.strip_prefix(prefix).unwrap_or(dir.as_str())
        } else {
            dir.as_str()
        };

        for entry in &nav_dir.entries {
            let rel_path = if rel_dir.is_empty() {
                entry.name.clone()
            } else {
                format!("{rel_dir}/{}", entry.name)
            };
            if !rel_path.to_lowercase().contains(&needle) {
                continue;
            }
            rows.push(PathSearchResult {
                href: entry.href.clone(),
                display: if entry.is_dir {
                    format!("{rel_path}/")
                } else {
                    rel_path
                },
                is_dir: entry.is_dir,
                modified: entry.modified,
            });
        }
    }

    rows.sort_by(|a, b| {
        let a_name = a.display.to_lowercase();
        let b_name = b.display.to_lowercase();
        let a_base = a_name
            .rsplit('/')
            .next()
            .is_some_and(|name| name.contains(&needle)) as u8;
        let b_base = b_name
            .rsplit('/')
            .next()
            .is_some_and(|name| name.contains(&needle)) as u8;
        b_base
            .cmp(&a_base)
            .then_with(|| cmp_path_rows(a, b, sort, dir))
    });

    let truncated = rows.len() > max_rows;
    rows.truncate(max_rows);

    PathSearchResponse {
        results: rows,
        truncated,
        max_rows,
    }
}

fn cmp_path_rows(
    a: &PathSearchResult,
    b: &PathSearchResult,
    sort: walk::Sort,
    dir: walk::SortDir,
) -> std::cmp::Ordering {
    match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => match sort {
            walk::Sort::Name => {
                apply_path_dir(a.display.to_lowercase().cmp(&b.display.to_lowercase()), dir)
            }
            walk::Sort::Type => apply_path_dir(
                path_ext(&a.display)
                    .cmp(&path_ext(&b.display))
                    .then_with(|| a.display.to_lowercase().cmp(&b.display.to_lowercase())),
                dir,
            ),
            walk::Sort::Timestamp => apply_path_dir(
                a.modified
                    .cmp(&b.modified)
                    .then_with(|| a.display.to_lowercase().cmp(&b.display.to_lowercase())),
                dir,
            ),
        },
    }
}

fn path_ext(display: &str) -> String {
    Path::new(display.trim_end_matches('/'))
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_lowercase()
}

fn apply_path_dir(order: std::cmp::Ordering, dir: walk::SortDir) -> std::cmp::Ordering {
    match dir {
        walk::SortDir::Asc => order,
        walk::SortDir::Desc => order.reverse(),
    }
}

#[derive(Deserialize)]
struct RenderQuery {
    path: Option<String>,
}

async fn api_render(State(s): State<AppState>, Query(q): Query<RenderQuery>) -> Response {
    let rel = q.path.as_deref().unwrap_or("").trim_matches('/');

    let (file_path, root) = if s.mode == Mode::File {
        let parent = s.target.parent().unwrap_or(&s.target).to_path_buf();
        let fp = if rel.is_empty() {
            s.target.clone()
        } else {
            parent.join(rel)
        };
        (fp, parent)
    } else {
        let fp = if rel.is_empty() {
            s.target.clone()
        } else {
            s.target.join(rel)
        };
        (fp, s.target.clone())
    };

    let md = match tokio::fs::read_to_string(&file_path).await {
        Ok(m) => m,
        Err(_) => return not_found(),
    };

    let rendered = render::render_at(
        &md,
        Some(render::RenderPath {
            root: &root,
            src: &file_path,
        }),
    );

    match serde_json::to_string(&rendered) {
        Ok(json) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json))
            .unwrap(),
        Err(e) => {
            warn!("api_render error: {}", e);
            not_found()
        }
    }
}

async fn theme_asset(AxPath(path): AxPath<String>) -> Response {
    let base = match crate::theme::dir() {
        Ok(d) => d,
        Err(e) => {
            warn!("theme dir error: {}", e);
            return not_found();
        }
    };
    let rel = path.trim_start_matches('/');
    for comp in PathBuf::from(rel).components() {
        if !matches!(comp, Component::Normal(_)) {
            return not_found();
        }
    }
    stream_file(&base.join(rel)).await
}

async fn vendor(AxPath(path): AxPath<String>) -> Response {
    let path = match crate::vendor::path(&path) {
        Ok(p) => p,
        Err(_) => return not_found(),
    };
    stream_file(&path).await
}

fn is_binary_ext(ext: &str) -> bool {
    matches!(
        ext,
        "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "svg"
            | "webp"
            | "ico"
            | "bmp"
            | "tiff"
            | "tif"
            | "pdf"
            | "woff"
            | "woff2"
            | "ttf"
            | "otf"
            | "eot"
            | "zip"
            | "gz"
            | "tar"
            | "bz2"
            | "xz"
            | "7z"
            | "rar"
            | "zst"
            | "exe"
            | "bin"
            | "so"
            | "dylib"
            | "dll"
            | "o"
            | "a"
            | "lib"
            | "mp3"
            | "mp4"
            | "wav"
            | "ogg"
            | "flac"
            | "mkv"
            | "avi"
            | "mov"
            | "webm"
            | "sqlite"
            | "db"
            | "sqlite3"
            | "class"
            | "jar"
            | "pyc"
    )
}

fn stream_bytes(path: &Path, bytes: Vec<u8>) -> Response {
    let mime = mime_guess(path);
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from(bytes))
        .unwrap()
}

async fn stream_file(path: &Path) -> Response {
    let bytes = match tokio::fs::read(path).await {
        Ok(b) => b,
        Err(_) => return not_found(),
    };
    stream_bytes(path, bytes)
}

async fn dispatch_file(
    s: &AppState,
    path: &Path,
    root: &Path,
    rel: &str,
    view: ViewState,
) -> Response {
    if path
        .extension()
        .and_then(|s| s.to_str())
        .map(|ext| is_binary_ext(&ext.to_lowercase()))
        .unwrap_or(false)
    {
        return stream_file(path).await;
    }

    let bytes = match tokio::fs::read(path).await {
        Ok(b) => b,
        Err(_) => return not_found(),
    };

    if bytes.contains(&0u8) {
        return stream_bytes(path, bytes);
    }

    let text = match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => return stream_bytes(path, e.into_bytes()),
    };

    let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("file");
    let rendered = render::render_text(filename, &text);
    let crumbs = breadcrumb_html(root, s.home.as_deref(), rel, &view, &s.view_cfg);
    let raw_html = raw_blob_html(&text, rendered.lang.as_deref());
    let view = FileView::raw();
    let view_attrs = file_view_attrs(rel, view);
    let body = match tmpl::page(tmpl::PageCtx {
        crumbs: &crumbs,
        preview_html: &rendered.html,
        raw_html: &raw_html,
        view_attrs: &view_attrs,
        preview_hidden: view.preview_hidden,
        raw_hidden: view.raw_hidden,
    }) {
        Ok(b) => b,
        Err(e) => {
            warn!("template error: {}", e);
            return not_found();
        }
    };
    respond_html(
        &rendered,
        &body,
        s.repos.source_for(path),
        &s.view_cfg,
        s.auth.is_some(),
    )
}

fn mime_guess(path: &Path) -> &'static str {
    match path.extension().and_then(|s| s.to_str()) {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("css") => "text/css; charset=utf-8",
        Some("js") | Some("mjs") => "application/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("html") => "text/html; charset=utf-8",
        _ => "application/octet-stream",
    }
}

async fn raw_file(State(s): State<AppState>, AxPath(path): AxPath<String>) -> Response {
    let Some(path) = resolve_internal_file(&s, &path) else {
        return not_found();
    };
    stream_export(&path, false).await
}

async fn html_file(State(s): State<AppState>, AxPath(path): AxPath<String>) -> Response {
    let Some(path) = resolve_internal_file(&s, &path) else {
        return not_found();
    };
    let bytes = match tokio::fs::read(&path).await {
        Ok(b) => b,
        Err(_) => return not_found(),
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime_guess(&path))
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from(bytes))
        .unwrap()
}

async fn download_file(State(s): State<AppState>, AxPath(path): AxPath<String>) -> Response {
    let Some(path) = resolve_internal_file(&s, &path) else {
        return not_found();
    };
    stream_export(&path, true).await
}

fn raw_blob_html(text: &str, lang: Option<&str>) -> String {
    let attrs = lang
        .map(|lang| {
            format!(
                r#" class="language-{lang}" data-lang="{lang}""#,
                lang = html_escape::encode_double_quoted_attribute(lang),
            )
        })
        .unwrap_or_default();
    format!(
        "<div class=\"ghrm-blob\">{}<div class=\"highlight ghrm-blob-source\" hidden><pre tabindex=\"0\" class=\"chroma\"><code{attrs}>{body}</code></pre></div><table class=\"ghrm-blob-table\" role=\"presentation\"><tbody></tbody></table></div>",
        raw_source_html(text),
        attrs = attrs,
        body = html_escape::encode_text(text),
    )
}

fn raw_source_html(text: &str) -> String {
    format!(
        "<template class=\"ghrm-data\">{}</template>",
        html_escape::encode_text(text),
    )
}

fn file_view_attrs(rel: &str, view: FileView) -> String {
    format!(
        "data-ghrm-view-kind=\"{kind}\" data-ghrm-raw-url=\"{raw}\" data-ghrm-download-url=\"{download}\"",
        kind = view.kind,
        raw = html_escape::encode_double_quoted_attribute(&internal_file_href("raw", rel)),
        download =
            html_escape::encode_double_quoted_attribute(&internal_file_href("download", rel)),
    )
}

fn internal_file_href(kind: &str, rel: &str) -> String {
    format!("/_ghrm/{kind}/{}", rel.trim_matches('/'))
}

fn resolve_internal_file(s: &AppState, rel: &str) -> Option<PathBuf> {
    let clean = rel.trim_matches('/');
    if clean.is_empty() {
        return None;
    }

    let rel_path = Path::new(clean);
    for comp in rel_path.components() {
        if !matches!(comp, Component::Normal(_)) {
            return None;
        }
    }

    let base = if s.mode == Mode::File {
        s.target.parent().unwrap_or(s.target.as_path())
    } else {
        s.target.as_path()
    };
    let path = base.join(rel_path);
    if path.is_file() { Some(path) } else { None }
}

async fn stream_export(path: &Path, attachment: bool) -> Response {
    let bytes = match tokio::fs::read(path).await {
        Ok(b) => b,
        Err(_) => return not_found(),
    };

    let mut res = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, export_mime(path, &bytes))
        .header(header::CACHE_CONTROL, "no-cache");
    if attachment {
        res = res.header(header::CONTENT_DISPOSITION, content_disposition(path));
    }
    res.body(Body::from(bytes)).unwrap()
}

fn export_mime(path: &Path, bytes: &[u8]) -> &'static str {
    let is_binary = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|ext| is_binary_ext(&ext.to_lowercase()))
        .unwrap_or(false)
        || bytes.contains(&0);
    if is_binary {
        mime_guess(path)
    } else {
        "text/plain; charset=utf-8"
    }
}

fn content_disposition(path: &Path) -> String {
    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    format!("attachment; filename=\"{filename}\"")
}

async fn ws_handler(State(s): State<AppState>, ws: WebSocketUpgrade) -> Response {
    let mut rx = s.reload.subscribe();
    ws.on_upgrade(|socket| async move {
        let (mut sink, mut stream) = socket.split();
        let send_task = tokio::spawn(async move {
            while rx.recv().await.is_ok() {
                if sink
                    .send(axum::extract::ws::Message::Text("reload".into()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        });
        while let Some(Ok(_)) = stream.next().await {}
        send_task.abort();
    })
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
    fn content_disposition_escapes_quotes() {
        let value = content_disposition(Path::new("odd\"name.md"));
        assert_eq!(value, "attachment; filename=\"odd\\\"name.md\"");
    }

    #[test]
    fn export_mime_prefers_text_plain_for_text_files() {
        assert_eq!(
            export_mime(Path::new("README.md"), b"# hello\n"),
            "text/plain; charset=utf-8",
        );
    }

    #[test]
    fn raw_blob_includes_hidden_source_block() {
        let html = raw_blob_html("fn main() {}\n", Some("rust"));
        assert!(html.contains("ghrm-blob-source"));
        assert!(html.contains(r#"class="language-rust""#));
        assert!(html.contains("<tbody></tbody>"));
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

    #[test]
    fn path_search_uses_selected_sort() {
        let mut dirs = BTreeMap::new();
        dirs.insert(
            String::new(),
            walk::NavDir {
                entries: vec![
                    walk::NavEntry {
                        name: "src".to_string(),
                        href: "/src/".to_string(),
                        is_dir: true,
                        modified: Some(3),
                    },
                    walk::NavEntry {
                        name: "older.md".to_string(),
                        href: "/older.md".to_string(),
                        is_dir: false,
                        modified: Some(1),
                    },
                    walk::NavEntry {
                        name: "newer.md".to_string(),
                        href: "/newer.md".to_string(),
                        is_dir: false,
                        modified: Some(9),
                    },
                ],
                readme: None,
            },
        );
        let tree = walk::NavTree { dirs };

        let resp = path_search(
            &tree,
            "",
            "m",
            10,
            walk::Sort::Timestamp,
            walk::SortDir::Desc,
        );
        let names: Vec<_> = resp.results.into_iter().map(|row| row.display).collect();
        assert_eq!(names, vec!["newer.md", "older.md"]);
    }

    #[test]
    fn source_display_splits_host_and_repo() {
        let (host, repo) = source_display("https://github.com/brege/ghrm", "brege / ghrm");
        assert_eq!(host, "github.com");
        assert_eq!(repo, "brege/ghrm");
    }
}
