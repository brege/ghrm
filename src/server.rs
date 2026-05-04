use crate::api;
use crate::auth;
use crate::column;
use crate::crumbs;
use crate::delivery;
use crate::explorer;
use crate::filter;
use crate::render::{self, Rendered};
use crate::repo::RepoSet;
use crate::runtime;
use crate::shell;
use crate::tmpl;
use crate::vendor;
use crate::view::{self, ViewConfig, ViewQuery, ViewState};
use crate::walk::{self, NavSet, ViewOpts};
use crate::watch;

use anyhow::Result;
use anyhow::anyhow;
use axum::{
    Router,
    body::Body,
    extract::{Path as AxPath, Query, RawQuery, State, ws::WebSocketUpgrade},
    http::{HeaderMap, StatusCode, header},
    middleware,
    response::Response,
    routing::get,
};
use futures_util::{SinkExt, StreamExt};
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tracing::{info, warn};

#[derive(Clone)]
pub struct AppState {
    pub target: PathBuf,
    pub mode: Mode,
    pub nav: Arc<RwLock<NavSet>>,
    pub alternate_nav: Arc<RwLock<Option<NavSet>>>,
    pub repos: RepoSet,
    pub reload: broadcast::Sender<&'static str>,
    pub use_ignore: bool,
    pub no_excludes: bool,
    pub view_cfg: ViewConfig,
    pub filter_exts: Vec<String>,
    pub filters: filter::Set,
    pub exclude_names: Vec<String>,
    pub search_max_rows: usize,
    pub home: Option<PathBuf>,
    pub runtime_paths: runtime::Paths,
    pub auth: Option<Arc<auth::AuthState>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    File,
    Dir,
}

#[derive(Clone, Default)]
pub(crate) struct HtmxContext {
    pub(crate) is_htmx: bool,
}

impl HtmxContext {
    fn from_headers(headers: &HeaderMap) -> Self {
        let is_htmx = headers
            .get("HX-Request")
            .and_then(|value| value.to_str().ok())
            .map(|value| value == "true")
            .unwrap_or(false);
        Self { is_htmx }
    }
}

fn native_file_request(headers: &HeaderMap) -> bool {
    if HtmxContext::from_headers(headers).is_htmx {
        return false;
    }
    if let Some(dest) = headers
        .get("Sec-Fetch-Dest")
        .and_then(|value| value.to_str().ok())
    {
        return !matches!(dest, "" | "document");
    }
    headers
        .get(header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|accept| {
            !accept.contains("text/html")
                && (accept.contains("image/")
                    || accept.contains("audio/")
                    || accept.contains("video/")
                    || accept.contains("font/")
                    || accept.contains("application/pdf"))
        })
}

pub struct Options {
    pub bind: String,
    pub port: u16,
    pub open: bool,
    pub target: PathBuf,
    pub use_ignore: bool,
    pub default_hidden: bool,
    pub default_filter_ext: bool,
    pub default_columns: column::Set,
    pub extensions: Vec<String>,
    pub filters: filter::Set,
    pub exclude_names: Vec<String>,
    pub no_excludes: bool,
    pub search_max_rows: usize,
    pub config_path: Option<PathBuf>,
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
        default_columns,
        extensions,
        filters,
        exclude_names,
        no_excludes,
        search_max_rows,
        config_path,
        auth,
    } = options;

    let meta = std::fs::metadata(&target)?;
    let mode = if meta.is_dir() { Mode::Dir } else { Mode::File };

    let (reload_tx, _) = broadcast::channel::<&'static str>(32);
    let nav = Arc::new(RwLock::new(NavSet::default()));
    let alternate_nav = Arc::new(RwLock::new(None));
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
        default_use_ignore: use_ignore,
        default_groups: filters.default_groups().to_vec(),
        default_sort: walk::Sort::Name,
        default_columns,
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

            // Build nav trees in background - don't block startup
            let nav_bg = nav.clone();
            let target_bg = target.clone();
            let walk_excludes = exclude_names.clone();
            let walk_extensions = extensions.clone();
            let nav_ready_tx = reload_tx.clone();
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
                    let _ = nav_ready_tx.send("nav-ready");
                }
            });

            // Watcher failure shouldn't kill the server
            if let Err(e) = watch::spawn_dir(
                target.clone(),
                watch::NavCache {
                    current: nav.clone(),
                    alternate: alternate_nav.clone(),
                },
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
        alternate_nav,
        repos,
        reload: reload_tx,
        use_ignore,
        no_excludes,
        view_cfg,
        filter_exts: extensions,
        filters,
        exclude_names,
        search_max_rows,
        home: std::env::var_os("HOME").map(PathBuf::from),
        runtime_paths: runtime::Paths::new(&target, config_path.as_deref())?,
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

impl AppState {
    pub(crate) fn cached_nav_tree(
        &self,
        view: &ViewState,
        matcher: Option<&filter::Matcher>,
    ) -> Option<Arc<walk::NavTree>> {
        if view.use_ignore == self.use_ignore {
            let nav = self.nav.read().unwrap();
            if !nav.is_ready() {
                return None;
            }
            return Some(nav.get(
                view.opts,
                view.sort,
                view.sort_dir,
                matcher,
                load_lines_for_view(view),
            ));
        }

        self.alternate_nav
            .read()
            .unwrap()
            .as_ref()
            .filter(|nav| nav.is_ready())
            .map(|nav| {
                nav.get(
                    view.opts,
                    view.sort,
                    view.sort_dir,
                    matcher,
                    load_lines_for_view(view),
                )
            })
    }

    pub(crate) fn nav_tree(
        &self,
        view: &ViewState,
        matcher: Option<&filter::Matcher>,
    ) -> Arc<walk::NavTree> {
        if let Some(tree) = self.cached_nav_tree(view, matcher) {
            return tree;
        }

        let mut guard = self.alternate_nav.write().unwrap();
        if guard.is_none() {
            *guard = Some(walk::build_all(
                &self.target,
                view.use_ignore,
                &self.exclude_names,
                &self.filter_exts,
                self.no_excludes,
            ));
        }
        guard.as_ref().unwrap().get(
            view.opts,
            view.sort,
            view.sort_dir,
            matcher,
            load_lines_for_view(view),
        )
    }
}

fn load_lines_for_view(view: &ViewState) -> bool {
    view.sort == walk::Sort::Lines
        || column::required_meta(&view.columns).contains(column::MetaReq::LINES)
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

async fn root(
    State(s): State<AppState>,
    headers: HeaderMap,
    RawQuery(raw_query): RawQuery,
    Query(q): Query<ViewQuery>,
) -> Response {
    let view = view::from_query(&q, raw_query.as_deref(), &s.view_cfg, &s.filters);
    let hx = HtmxContext::from_headers(&headers);
    match s.mode {
        Mode::File => render_target(&s, &s.target, None, view, hx).await,
        Mode::Dir => explorer::render(&s, "", view, hx).await,
    }
}

async fn any_path(
    State(s): State<AppState>,
    AxPath(path): AxPath<String>,
    headers: HeaderMap,
    RawQuery(raw_query): RawQuery,
    Query(q): Query<ViewQuery>,
) -> Response {
    let view = view::from_query(&q, raw_query.as_deref(), &s.view_cfg, &s.filters);
    let hx = HtmxContext::from_headers(&headers);
    let native = native_file_request(&headers);
    if s.mode == Mode::File {
        return serve_file_mode(&s, &path, view, hx, native).await;
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
                .header(header::VARY, "HX-Request")
                .body(Body::empty())
                .unwrap();
        }
        return explorer::render(&s, &clean, view, hx).await;
    }
    if joined.extension().and_then(|s| s.to_str()) == Some("md") {
        return render_file(&s, &joined, Some(s.target.as_path()), view, hx).await;
    }
    if native {
        return delivery::stream_file(&joined).await;
    }
    dispatch_file(&s, &joined, &s.target, &clean, view, hx).await
}

async fn serve_file_mode(
    s: &AppState,
    path: &str,
    view: ViewState,
    hx: HtmxContext,
    native: bool,
) -> Response {
    let Some(root) = s.target.parent() else {
        return not_found();
    };
    let clean = path.trim_matches('/');
    if clean.is_empty() {
        return render_target(s, &s.target, None, view, hx).await;
    }
    let joined = root.join(clean);
    let meta = match tokio::fs::metadata(&joined).await {
        Ok(m) => m,
        Err(_) => return not_found(),
    };
    if meta.is_dir() {
        return not_found();
    }
    if native {
        return delivery::stream_file(&joined).await;
    }
    render_target(s, &joined, None, view, hx).await
}

async fn render_target(
    s: &AppState,
    path: &Path,
    root: Option<&Path>,
    view: ViewState,
    hx: HtmxContext,
) -> Response {
    if path.extension().and_then(|s| s.to_str()) == Some("md") {
        render_file(s, path, root, view, hx).await
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
        dispatch_file(s, path, base, &rel, view, hx).await
    }
}

async fn render_file(
    s: &AppState,
    path: &Path,
    root: Option<&Path>,
    view: ViewState,
    hx: HtmxContext,
) -> Response {
    let md = match tokio::fs::read_to_string(path).await {
        Ok(m) => m,
        Err(_) => return not_found(),
    };
    let Some(root) = root.or_else(|| path.parent()) else {
        return not_found();
    };
    let rendered = render::render_at(&md, Some(render::RenderPath { root, src: path }));
    let features = vendor::feature_list(&rendered);
    let rel = path
        .strip_prefix(root)
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
        .or_else(|| path.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default();
    let crumbs = crumbs::html(root, s.home.as_deref(), &rel, &view, &s.view_cfg);
    let raw_html = delivery::raw_blob_html(&md, Some("markdown"));
    let view = delivery::FileView::markdown();
    let view_attrs = delivery::file_view_attrs(&rel, view);
    let body = match tmpl::page(tmpl::PageCtx {
        features: &features,
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
    let source = s.repos.source_for(path);
    let title = if rendered.title.is_empty() {
        "Preview"
    } else {
        &rendered.title
    };
    if hx.is_htmx {
        return shell::fragment(&body, title, source);
    }
    shell::full_page(&rendered, &body, source, s.auth.is_some(), &s.runtime_paths)
}

async fn dispatch_file(
    s: &AppState,
    path: &Path,
    root: &Path,
    rel: &str,
    view: ViewState,
    hx: HtmxContext,
) -> Response {
    let mode = delivery::file_mode_async(path).await;
    match mode {
        delivery::FileMode::Markdown => render_file(s, path, Some(root), view, hx).await,
        delivery::FileMode::Source => render_source_file(s, path, root, rel, view, hx).await,
        delivery::FileMode::Dual => render_dual_file(s, path, root, rel, view, hx).await,
        delivery::FileMode::Native => native_file(s, path, rel, &view, hx).await,
        delivery::FileMode::Download => download_file(s, path, rel, &view, hx).await,
    }
}

async fn download_file(
    s: &AppState,
    path: &Path,
    rel: &str,
    view: &ViewState,
    hx: HtmxContext,
) -> Response {
    if hx.is_htmx {
        let href = view::with_view(&format!("/{rel}"), view, &s.view_cfg);
        return shell::redirect(&href);
    }
    delivery::stream_download(path).await
}

async fn render_source_file(
    s: &AppState,
    path: &Path,
    root: &Path,
    rel: &str,
    view: ViewState,
    hx: HtmxContext,
) -> Response {
    let bytes = match tokio::fs::read(path).await {
        Ok(b) => b,
        Err(_) => return not_found(),
    };

    let text = match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => return download_file(s, path, rel, &view, hx).await,
    };

    let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("file");
    let rendered = render::render_text(filename, &text);
    let features = vendor::feature_list(&rendered);
    let crumbs = crumbs::html(root, s.home.as_deref(), rel, &view, &s.view_cfg);
    let raw_html = delivery::raw_blob_html(&text, rendered.lang.as_deref());
    let file_view = delivery::FileView::source();
    let view_attrs = delivery::file_view_attrs(rel, file_view);
    let body = match tmpl::page(tmpl::PageCtx {
        features: &features,
        crumbs: &crumbs,
        preview_html: &rendered.html,
        raw_html: &raw_html,
        view_attrs: &view_attrs,
        preview_hidden: file_view.preview_hidden,
        raw_hidden: file_view.raw_hidden,
    }) {
        Ok(b) => b,
        Err(e) => {
            warn!("template error: {}", e);
            return not_found();
        }
    };
    let source = s.repos.source_for(path);
    let title = if rendered.title.is_empty() {
        "Preview"
    } else {
        &rendered.title
    };
    if hx.is_htmx {
        return shell::fragment(&body, title, source);
    }
    shell::full_page(&rendered, &body, source, s.auth.is_some(), &s.runtime_paths)
}

async fn render_dual_file(
    s: &AppState,
    path: &Path,
    root: &Path,
    rel: &str,
    view: ViewState,
    hx: HtmxContext,
) -> Response {
    let bytes = match tokio::fs::read(path).await {
        Ok(b) => b,
        Err(_) => return not_found(),
    };

    let text = match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => return native_file(s, path, rel, &view, hx).await,
    };

    let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = path.extension().and_then(|s| s.to_str());
    let native_url = format!("/{}", rel.trim_matches('/'));
    let preview_html = dual_preview_html(ext, &native_url, filename);
    let rendered = Rendered {
        html: preview_html.clone(),
        title: filename.to_string(),
        lang: ext.map(String::from),
        has_mermaid: false,
        has_math: false,
        has_map: false,
    };
    let features = vendor::feature_list(&rendered);
    let crumbs = crumbs::html(root, s.home.as_deref(), rel, &view, &s.view_cfg);
    let raw_html = delivery::raw_blob_html(&text, ext);
    let file_view = delivery::FileView::dual();
    let view_attrs = delivery::file_view_attrs(rel, file_view);
    let body = match tmpl::page(tmpl::PageCtx {
        features: &features,
        crumbs: &crumbs,
        preview_html: &preview_html,
        raw_html: &raw_html,
        view_attrs: &view_attrs,
        preview_hidden: file_view.preview_hidden,
        raw_hidden: file_view.raw_hidden,
    }) {
        Ok(b) => b,
        Err(e) => {
            warn!("template error: {}", e);
            return not_found();
        }
    };
    let source = s.repos.source_for(path);
    if hx.is_htmx {
        return shell::fragment(&body, &rendered.title, source);
    }
    shell::full_page(&rendered, &body, source, s.auth.is_some(), &s.runtime_paths)
}

fn dual_preview_html(ext: Option<&str>, native_url: &str, filename: &str) -> String {
    let url = html_escape::encode_double_quoted_attribute(native_url);
    let alt = html_escape::encode_double_quoted_attribute(filename);
    match ext {
        Some(e) if e.eq_ignore_ascii_case("svg") => {
            format!(r#"<div class="ghrm-svg-preview"><img src="{url}" alt="{alt}"></div>"#)
        }
        _ => String::new(),
    }
}

async fn native_file(
    s: &AppState,
    path: &Path,
    rel: &str,
    view: &ViewState,
    hx: HtmxContext,
) -> Response {
    if hx.is_htmx {
        let href = view::with_view(&format!("/{rel}"), view, &s.view_cfg);
        return shell::redirect(&href);
    }
    delivery::stream_file(path).await
}

async fn ws_handler(State(s): State<AppState>, ws: WebSocketUpgrade) -> Response {
    let mut rx = s.reload.subscribe();
    ws.on_upgrade(|socket| async move {
        let (mut sink, mut stream) = socket.split();
        let send_task = tokio::spawn(async move {
            while let Ok(message) = rx.recv().await {
                if sink
                    .send(axum::extract::ws::Message::Text(message.into()))
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
    fn htmx_context_detects_hx_request_header() {
        let mut headers = HeaderMap::new();
        headers.insert("HX-Request", "true".parse().unwrap());
        let hx = HtmxContext::from_headers(&headers);
        assert!(hx.is_htmx);
    }

    #[test]
    fn htmx_context_ignores_missing_header() {
        let headers = HeaderMap::new();
        let hx = HtmxContext::from_headers(&headers);
        assert!(!hx.is_htmx);
    }

    #[test]
    fn htmx_context_ignores_non_true_value() {
        let mut headers = HeaderMap::new();
        headers.insert("HX-Request", "false".parse().unwrap());
        let hx = HtmxContext::from_headers(&headers);
        assert!(!hx.is_htmx);
    }

    #[test]
    fn native_file_request_detects_image_fetch() {
        let mut headers = HeaderMap::new();
        headers.insert("Sec-Fetch-Dest", "image".parse().unwrap());
        assert!(native_file_request(&headers));
    }

    #[test]
    fn native_file_request_ignores_document_navigation() {
        let mut headers = HeaderMap::new();
        headers.insert("Sec-Fetch-Dest", "document".parse().unwrap());
        assert!(!native_file_request(&headers));
    }

    #[test]
    fn native_file_request_ignores_htmx_requests() {
        let mut headers = HeaderMap::new();
        headers.insert("HX-Request", "true".parse().unwrap());
        headers.insert("Sec-Fetch-Dest", "image".parse().unwrap());
        assert!(!native_file_request(&headers));
    }
}
