use crate::api;
use crate::auth;
use crate::delivery;
use crate::filter;
use crate::render::{self, Rendered};
use crate::repo::{RepoSet, SourceState};
use crate::tmpl::{self, ExplorerCtx, ExplorerEntry, ExplorerReadme, PageShell};
use crate::view::{self, ViewConfig, ViewQuery, ViewState};
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
use std::net::{IpAddr, SocketAddr, UdpSocket};
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
        .route("/_ghrm/tree", get(api::tree))
        .route("/_ghrm/path-search", get(api::path_search))
        .route("/_ghrm/search", get(api::search))
        .route("/_ghrm/render", get(api::render))
        .route("/_ghrm/raw/{*path}", get(delivery::raw_file))
        .route("/_ghrm/html/{*path}", get(delivery::html_file))
        .route("/_ghrm/download/{*path}", get(delivery::download_file))
        .route("/{*path}", get(any_path));

    let auth_enabled = state.auth.is_some();
    let app = if auth_enabled {
        Router::new()
            .route(
                "/_ghrm/login",
                get(auth::login_form).post(auth::login_submit),
            )
            .route("/_ghrm/logout", get(auth::logout))
            .route("/_ghrm/assets/{*path}", get(delivery::theme_asset))
            .route("/vendor/{*path}", get(delivery::vendor))
            .merge(protected.layer(middleware::from_fn_with_state(state.clone(), auth::require)))
            .with_state(state)
    } else {
        Router::new()
            .route("/_ghrm/assets/{*path}", get(delivery::theme_asset))
            .route("/vendor/{*path}", get(delivery::vendor))
            .merge(protected)
            .with_state(state)
    };

    let addr = find_addr(&bind, port).await?;
    let listener = TcpListener::bind(addr).await?;
    let actual = listener.local_addr()?;
    let url = server_url(&actual);
    info!(%actual, "ghrm listening");
    info!("local: {}", url);
    if let Some(url) = network_url(&actual) {
        if auth_enabled {
            info!("network: {} (auth required)", url);
        } else {
            info!("network: {}", url);
        }
    }
    if open {
        open_browser(&url);
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

fn server_url(addr: &SocketAddr) -> String {
    let host = if addr.ip().is_loopback() || addr.ip().is_unspecified() {
        "localhost".to_string()
    } else if addr.ip().is_ipv6() {
        format!("[{}]", addr.ip())
    } else {
        addr.ip().to_string()
    };
    format!("http://{}:{}/", host, addr.port())
}

fn network_url(addr: &SocketAddr) -> Option<String> {
    let ip = if addr.ip().is_unspecified() {
        outbound_ip()?
    } else if addr.ip().is_loopback() {
        return None;
    } else {
        addr.ip()
    };
    Some(format!("http://{}:{}/", url_host(ip), addr.port()))
}

fn outbound_ip() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    Some(socket.local_addr().ok()?.ip())
}

fn url_host(ip: IpAddr) -> String {
    if ip.is_ipv6() {
        format!("[{ip}]")
    } else {
        ip.to_string()
    }
}

fn open_browser(url: &str) {
    let _ = std::process::Command::new("xdg-open")
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
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
        out.push_str(&html_escape::encode_double_quoted_attribute(
            &view::with_view(&href, view, cfg),
        ));
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
    let view = view::from_query(&q, raw_query.as_deref(), &s.view_cfg, &s.filters);
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
    let view = view::from_query(&q, raw_query.as_deref(), &s.view_cfg, &s.filters);
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
            let loc = view::with_view(&format!("/{}/", clean), &view, &s.view_cfg);
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
    let raw_html = delivery::raw_blob_html(&md, Some("markdown"));
    let view = delivery::FileView::markdown();
    let view_attrs = delivery::file_view_attrs(&rel, view);
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
    let matcher = view::matcher(&view, &s.filters);
    let filter_exts = view::filter_exts(&view, &s.filter_exts);
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
    let parent_href = view::with_view(&parent_href, &view, &s.view_cfg);

    let entries: Vec<ExplorerEntry> = dir
        .entries
        .iter()
        .map(|e| ExplorerEntry {
            name: e.name.clone(),
            href: view::with_view(&e.href, &view, &s.view_cfg),
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
        .map(|ext| delivery::is_binary_ext(&ext.to_lowercase()))
        .unwrap_or(false)
    {
        return delivery::stream_file(path).await;
    }

    let bytes = match tokio::fs::read(path).await {
        Ok(b) => b,
        Err(_) => return not_found(),
    };

    if bytes.contains(&0u8) {
        return delivery::stream_bytes(path, bytes);
    }

    let text = match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => return delivery::stream_bytes(path, e.into_bytes()),
    };

    let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("file");
    let rendered = render::render_text(filename, &text);
    let crumbs = breadcrumb_html(root, s.home.as_deref(), rel, &view, &s.view_cfg);
    let raw_html = delivery::raw_blob_html(&text, rendered.lang.as_deref());
    let view = delivery::FileView::raw();
    let view_attrs = delivery::file_view_attrs(rel, view);
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

    #[test]
    fn source_display_splits_host_and_repo() {
        let (host, repo) = source_display("https://github.com/brege/ghrm", "brege / ghrm");
        assert_eq!(host, "github.com");
        assert_eq!(repo, "brege/ghrm");
    }
}
