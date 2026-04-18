use crate::render::{self, Rendered};
use crate::tmpl::{self, ExplorerCtx, ExplorerEntry, ExplorerReadme, PageShell};
use crate::walk::{self, NavSet, Scope};
use crate::watch;

use anyhow::Result;
use anyhow::anyhow;
use axum::{
    Router,
    body::Body,
    extract::{Path as AxPath, Query, State, ws::WebSocketUpgrade},
    http::{StatusCode, header},
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
    pub reload: broadcast::Sender<()>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    File,
    Dir,
}

pub async fn run(
    bind: String,
    port: u16,
    open: bool,
    target: PathBuf,
    use_ignore: bool,
) -> Result<()> {
    let meta = std::fs::metadata(&target)?;
    let mode = if meta.is_dir() { Mode::Dir } else { Mode::File };

    let (reload_tx, _) = broadcast::channel::<()>(32);
    let nav = Arc::new(RwLock::new(NavSet::default()));

    match mode {
        Mode::Dir => {
            let fresh = walk::build_all(&target, use_ignore);
            *nav.write().unwrap() = fresh;
            watch::spawn_dir(target.clone(), nav.clone(), reload_tx.clone(), use_ignore)?;
        }
        Mode::File => {
            watch::spawn_file(target.clone(), reload_tx.clone())?;
        }
    }

    let state = AppState {
        target: target.clone(),
        mode,
        nav,
        reload: reload_tx,
    };

    let app = Router::new()
        .route("/", get(root))
        .route("/_ghrm/ws", get(ws_handler))
        .route("/_ghrm/tree", get(api_tree))
        .route("/_ghrm/render", get(api_render))
        .route("/_ghrm/assets/*path", get(theme_asset))
        .route("/vendor/*path", get(vendor))
        .route("/*path", get(any_path))
        .with_state(state);

    let addr = find_addr(&bind, port).await?;
    let listener = TcpListener::bind(addr).await?;
    let actual = listener.local_addr()?;
    info!(%actual, "ghrm listening");
    if open {
        open_browser(&actual);
    }
    axum::serve(listener, app).await?;
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
struct ScopeQuery {
    scope: Option<String>,
}

fn scope_from_query(q: &ScopeQuery) -> Scope {
    Scope::parse(q.scope.as_deref())
}

fn with_scope(href: &str, scope: Scope) -> String {
    match scope.query() {
        Some(scope) if href.contains('?') => format!("{href}&scope={scope}"),
        Some(scope) => format!("{href}?scope={scope}"),
        None => href.to_string(),
    }
}

async fn root(State(s): State<AppState>, Query(q): Query<ScopeQuery>) -> Response {
    let scope = scope_from_query(&q);
    match s.mode {
        Mode::File => render_file(&s.target, None).await,
        Mode::Dir => render_explorer(&s, "", scope).await,
    }
}

async fn any_path(
    State(s): State<AppState>,
    AxPath(path): AxPath<String>,
    Query(q): Query<ScopeQuery>,
) -> Response {
    let scope = scope_from_query(&q);
    if s.mode == Mode::File {
        return serve_file_mode(&s, &path).await;
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
            let loc = with_scope(&format!("/{}/", clean), scope);
            return Response::builder()
                .status(StatusCode::MOVED_PERMANENTLY)
                .header(header::LOCATION, loc)
                .body(Body::empty())
                .unwrap();
        }
        return render_explorer(&s, &clean, scope).await;
    }
    if joined.extension().and_then(|s| s.to_str()) == Some("md") {
        return render_file(&joined, Some(&s.target)).await;
    }
    stream_file(&joined).await
}

async fn serve_file_mode(s: &AppState, path: &str) -> Response {
    let Some(root) = s.target.parent() else {
        return not_found();
    };
    let clean = path.trim_matches('/');
    if clean.is_empty() {
        return render_file(&s.target, None).await;
    }
    let joined = root.join(clean);
    let meta = match tokio::fs::metadata(&joined).await {
        Ok(m) => m,
        Err(_) => return not_found(),
    };
    if meta.is_dir() {
        return not_found();
    }
    if joined.extension().and_then(|s| s.to_str()) == Some("md") {
        return render_file(&joined, None).await;
    }
    stream_file(&joined).await
}

async fn render_file(path: &Path, root: Option<&Path>) -> Response {
    let md = match tokio::fs::read_to_string(path).await {
        Ok(m) => m,
        Err(_) => return not_found(),
    };
    let Some(root) = root.or_else(|| path.parent()) else {
        return not_found();
    };
    let rendered = render::render_at(&md, Some(render::RenderPath { root, src: path }));
    let body = match tmpl::page(&rendered.html) {
        Ok(b) => b,
        Err(e) => {
            warn!("template error: {}", e);
            return not_found();
        }
    };
    respond_html(&rendered, &body)
}

async fn render_explorer(s: &AppState, rel: &str, scope: Scope) -> Response {
    let dir_opt = {
        let guard = s.nav.read().unwrap();
        guard.get(scope).dirs.get(rel).cloned()
    };
    let Some(dir) = dir_opt else {
        return not_found();
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
    let parent_href = with_scope(&parent_href, scope);

    struct ScopedEntry {
        name: String,
        href: String,
        is_dir: bool,
    }

    let scoped_entries: Vec<ScopedEntry> = dir
        .entries
        .iter()
        .map(|e| ScopedEntry {
            name: e.name.clone(),
            href: with_scope(&e.href, scope),
            is_dir: e.is_dir,
        })
        .collect();
    let entries: Vec<ExplorerEntry> = scoped_entries
        .iter()
        .map(|e| ExplorerEntry {
            name: &e.name,
            href: &e.href,
            is_dir: e.is_dir,
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

    let body = match tmpl::explorer(ExplorerCtx {
        show_title: true,
        title: &title,
        has_parent,
        parent_href: &parent_href,
        entries: &entries,
        readme: readme_tmpl,
    }) {
        Ok(b) => b,
        Err(e) => {
            warn!("template error: {}", e);
            return not_found();
        }
    };

    let combined = Rendered {
        html: String::new(),
        title,
        has_mermaid: readme_rendered
            .as_ref()
            .map(|r| r.has_mermaid)
            .unwrap_or(false),
        has_math: readme_rendered
            .as_ref()
            .map(|r| r.has_math)
            .unwrap_or(false),
        has_map: readme_rendered.as_ref().map(|r| r.has_map).unwrap_or(false),
    };

    respond_html(&combined, &body)
}

fn respond_html(r: &Rendered, body: &str) -> Response {
    let title = if r.title.is_empty() {
        "Preview"
    } else {
        &r.title
    };
    let html = match tmpl::base(PageShell { title, body }) {
        Ok(h) => h,
        Err(e) => {
            warn!("template error: {}", e);
            return not_found();
        }
    };
    Html(html).into_response()
}

// --- JSON APIs for optional SPA navigation ---

#[derive(Serialize)]
struct TreeResponse {
    mode: &'static str,
    root: String,
    dirs: BTreeMap<String, crate::walk::NavDir>,
}

async fn api_tree(State(s): State<AppState>, Query(q): Query<ScopeQuery>) -> Response {
    let nav = s.nav.read().unwrap();
    let scope = scope_from_query(&q);
    let tree = nav.get(scope);
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

async fn stream_file(path: &Path) -> Response {
    let bytes = match tokio::fs::read(path).await {
        Ok(b) => b,
        Err(_) => return not_found(),
    };
    let mime = mime_guess(path);
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from(bytes))
        .unwrap()
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
