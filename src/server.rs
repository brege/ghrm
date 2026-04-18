use crate::assets::{BUNDLE_CSS, PREVIEW_JS};
use crate::render::{self, Rendered};
use crate::tmpl::{self, ExplorerCtx, ExplorerEntry, ExplorerReadme, PageShell};
use crate::walk::{self, NavTree};
use crate::watch;

use anyhow::{Result, anyhow};
use axum::{
    Router,
    body::Body,
    extract::{Path as AxPath, State, ws::WebSocketUpgrade},
    http::{StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tracing::{info, warn};

#[derive(Clone)]
pub struct AppState {
    pub target: PathBuf,
    pub mode: Mode,
    pub nav: Arc<RwLock<NavTree>>,
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
    let nav = Arc::new(RwLock::new(NavTree::default()));

    match mode {
        Mode::Dir => {
            let fresh = walk::build(&target, use_ignore);
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
        .route("/_ghrm/css/bundle.css", get(bundle_css))
        .route("/js/preview.js", get(preview_js))
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

async fn root(State(s): State<AppState>) -> Response {
    match s.mode {
        Mode::File => render_file(&s.target, None).await,
        Mode::Dir => render_explorer(&s, "").await,
    }
}

async fn any_path(State(s): State<AppState>, AxPath(path): AxPath<String>) -> Response {
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
            let loc = format!("/{}/", clean);
            return Response::builder()
                .status(StatusCode::MOVED_PERMANENTLY)
                .header(header::LOCATION, loc)
                .body(Body::empty())
                .unwrap();
        }
        return render_explorer(&s, &clean).await;
    }
    if joined.extension().and_then(|s| s.to_str()) == Some("md") {
        return render_file(&joined, Some(&s.target)).await;
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
    let body = tmpl::page(&rendered.html);
    respond_html(&rendered, &body)
}

async fn render_explorer(s: &AppState, rel: &str) -> Response {
    let dir_opt = {
        let guard = s.nav.read().unwrap();
        guard.dirs.get(rel).cloned()
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

    let entries: Vec<ExplorerEntry> = dir
        .entries
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

    let body = tmpl::explorer(ExplorerCtx {
        show_title: true,
        title: &title,
        has_parent,
        parent_href: &parent_href,
        entries: &entries,
        readme: readme_tmpl,
    });

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
    let html = tmpl::base(PageShell {
        title,
        body,
        live_reload: true,
    });
    Html(html).into_response()
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

async fn bundle_css() -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/css; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from(BUNDLE_CSS.clone()))
        .unwrap()
}

async fn preview_js() -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )
        .body(Body::from(PREVIEW_JS))
        .unwrap()
}

async fn vendor(AxPath(path): AxPath<String>) -> Response {
    let path = match crate::vendor::path(&path) {
        Ok(path) => path,
        Err(_) => return not_found(),
    };
    stream_file(&path).await
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

fn _internal(msg: String) -> Response {
    warn!("internal error: {}", msg);
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(Body::from(msg))
        .unwrap()
}
