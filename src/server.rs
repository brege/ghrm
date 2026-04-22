use crate::render::{self, Rendered};
use crate::repo::{Forge, RepoSet, SourceState};
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
    pub repos: RepoSet,
    pub reload: broadcast::Sender<()>,
    pub default_scope: Scope,
    pub has_ext_filter: bool,
    pub home: Option<PathBuf>,
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
    pub default_scope: Scope,
    pub extensions: Vec<String>,
    pub exclude_names: Vec<String>,
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

pub async fn run(options: Options) -> Result<()> {
    let Options {
        bind,
        port,
        open,
        target,
        use_ignore,
        default_scope,
        extensions,
        exclude_names,
    } = options;

    let meta = std::fs::metadata(&target)?;
    let mode = if meta.is_dir() { Mode::Dir } else { Mode::File };

    let (reload_tx, _) = broadcast::channel::<()>(32);
    let nav = Arc::new(RwLock::new(NavSet::default()));
    let has_ext_filter = !extensions.is_empty();
    let repo_root_buf = if mode == Mode::Dir {
        target.clone()
    } else {
        target.parent().unwrap_or(&target).to_path_buf()
    };

    let repos = match mode {
        Mode::Dir => {
            let target2 = target.clone();
            let repo_root2 = repo_root_buf.clone();
            let walk_excludes = exclude_names.clone();
            let repo_excludes = exclude_names.clone();
            let walk_extensions = extensions.clone();
            let walk_h = tokio::task::spawn_blocking(move || {
                walk::build_all(&target2, use_ignore, &walk_excludes, &walk_extensions)
            });
            let repo_h =
                tokio::task::spawn_blocking(move || RepoSet::discover(&repo_root2, &repo_excludes));
            let (fresh, repos) = tokio::join!(walk_h, repo_h);
            *nav.write().unwrap() = fresh?;
            watch::spawn_dir(
                target.clone(),
                nav.clone(),
                reload_tx.clone(),
                use_ignore,
                exclude_names.clone(),
                extensions.clone(),
            )?;
            repos?
        }
        Mode::File => {
            watch::spawn_file(target.clone(), reload_tx.clone())?;
            tokio::task::spawn_blocking(move || RepoSet::discover(&repo_root_buf, &exclude_names))
                .await?
        }
    };

    let state = AppState {
        target: target.clone(),
        mode,
        nav,
        repos,
        reload: reload_tx,
        default_scope,
        has_ext_filter,
        home: std::env::var_os("HOME").map(PathBuf::from),
    };

    let app = Router::new()
        .route("/", get(root))
        .route("/_ghrm/ws", get(ws_handler))
        .route("/_ghrm/tree", get(api_tree))
        .route("/_ghrm/render", get(api_render))
        .route("/_ghrm/raw/{*path}", get(raw_file))
        .route("/_ghrm/download/{*path}", get(download_file))
        .route("/_ghrm/assets/{*path}", get(theme_asset))
        .route("/vendor/{*path}", get(vendor))
        .route("/{*path}", get(any_path))
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

fn scope_from_query(q: &ScopeQuery, default_scope: Scope, has_ext_filter: bool) -> Scope {
    let scope = q
        .scope
        .as_deref()
        .and_then(Scope::parse)
        .unwrap_or(default_scope);
    if scope == Scope::Filtered && !has_ext_filter {
        default_scope
    } else {
        scope
    }
}

fn with_scope(href: &str, scope: Scope, default_scope: Scope) -> String {
    if scope == default_scope {
        return href.to_string();
    }
    let scope = scope_name(scope);
    if href.contains('?') {
        format!("{href}&scope={scope}")
    } else {
        format!("{href}?scope={scope}")
    }
}

fn breadcrumb_html(
    target: &Path,
    home: Option<&Path>,
    rel: &str,
    scope: Scope,
    default_scope: Scope,
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
        out.push_str(&html_escape::encode_double_quoted_attribute(&with_scope(
            &href,
            scope,
            default_scope,
        )));
        out.push_str(r#"">"#);
        out.push_str(&label);
        out.push_str("</a>");
    }

    out
}

async fn root(State(s): State<AppState>, Query(q): Query<ScopeQuery>) -> Response {
    let scope = scope_from_query(&q, s.default_scope, s.has_ext_filter);
    match s.mode {
        Mode::File => render_target(&s, &s.target, None, scope).await,
        Mode::Dir => render_explorer(&s, "", scope).await,
    }
}

async fn any_path(
    State(s): State<AppState>,
    AxPath(path): AxPath<String>,
    Query(q): Query<ScopeQuery>,
) -> Response {
    let scope = scope_from_query(&q, s.default_scope, s.has_ext_filter);
    if s.mode == Mode::File {
        return serve_file_mode(&s, &path, scope).await;
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
            let loc = with_scope(&format!("/{}/", clean), scope, s.default_scope);
            return Response::builder()
                .status(StatusCode::MOVED_PERMANENTLY)
                .header(header::LOCATION, loc)
                .body(Body::empty())
                .unwrap();
        }
        return render_explorer(&s, &clean, scope).await;
    }
    if joined.extension().and_then(|s| s.to_str()) == Some("md") {
        return render_file(&s, &joined, Some(s.target.as_path()), scope).await;
    }
    dispatch_file(&s, &joined, &s.target, &clean, scope).await
}

async fn serve_file_mode(s: &AppState, path: &str, scope: Scope) -> Response {
    let Some(root) = s.target.parent() else {
        return not_found();
    };
    let clean = path.trim_matches('/');
    if clean.is_empty() {
        return render_target(s, &s.target, None, scope).await;
    }
    let joined = root.join(clean);
    let meta = match tokio::fs::metadata(&joined).await {
        Ok(m) => m,
        Err(_) => return not_found(),
    };
    if meta.is_dir() {
        return not_found();
    }
    render_target(s, &joined, None, scope).await
}

async fn render_target(s: &AppState, path: &Path, root: Option<&Path>, scope: Scope) -> Response {
    if path.extension().and_then(|s| s.to_str()) == Some("md") {
        render_file(s, path, root, scope).await
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
        dispatch_file(s, path, base, &rel, scope).await
    }
}

async fn render_file(s: &AppState, path: &Path, root: Option<&Path>, scope: Scope) -> Response {
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
    let crumbs = breadcrumb_html(root, s.home.as_deref(), &rel, scope, s.default_scope);
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
    respond_html_with_scope(
        &rendered,
        &body,
        s.repos.source_for(path),
        s.default_scope,
        s.has_ext_filter,
    )
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
    let parent_href = with_scope(&parent_href, scope, s.default_scope);

    let entries: Vec<ExplorerEntry> = dir
        .entries
        .iter()
        .map(|e| ExplorerEntry {
            name: e.name.clone(),
            href: with_scope(&e.href, scope, s.default_scope),
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
    let crumbs = breadcrumb_html(&s.target, s.home.as_deref(), rel, scope, s.default_scope);

    let body = match tmpl::explorer(ExplorerCtx {
        crumbs: &crumbs,
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

    let (has_mermaid, has_math, has_map) = readme_rendered
        .as_ref()
        .map(|r| (r.has_mermaid, r.has_math, r.has_map))
        .unwrap_or_default();
    let combined = Rendered {
        html: String::new(),
        title,
        has_mermaid,
        has_math,
        has_map,
    };

    let current = if rel.is_empty() {
        s.target.clone()
    } else {
        s.target.join(rel)
    };
    respond_html_with_scope(
        &combined,
        &body,
        s.repos.source_for(&current),
        s.default_scope,
        s.has_ext_filter,
    )
}

fn respond_html_with_scope(
    r: &Rendered,
    body: &str,
    source: SourceState,
    default_scope: Scope,
    has_ext_filter: bool,
) -> Response {
    let title = if r.title.is_empty() {
        "Preview"
    } else {
        &r.title
    };
    let source = source_html(&source);
    let html = match tmpl::base(PageShell {
        title,
        body,
        source: &source,
        default_scope: scope_name(default_scope),
        has_ext_filter,
    }) {
        Ok(h) => h,
        Err(e) => {
            warn!("template error: {}", e);
            return not_found();
        }
    };
    Html(html).into_response()
}

fn scope_name(scope: Scope) -> &'static str {
    match scope {
        Scope::Filtered => "filter",
        Scope::Files => "files",
        Scope::All => "all",
    }
}

fn source_html(source: &SourceState) -> String {
    match source {
        SourceState::Web { url, label, forge } => web_source_html(url, label, *forge),
        SourceState::Transport { raw } => format!(
            "<span id=\"ghrm-source-slot\" class=\"ghrm-source-link is-disabled\" aria-label=\"Transport-only remote\" title=\"Transport-only remote: {raw}\"><svg aria-hidden=\"true\" focusable=\"false\"><use href=\"#ghrm-icon-git\"></use></svg><span class=\"ghrm-source-text\">{text}</span></span>",
            raw = html_escape::encode_double_quoted_attribute(raw),
            text = html_escape::encode_text(raw),
        ),
        SourceState::NoRemote => "<span id=\"ghrm-source-slot\" class=\"ghrm-source-link is-disabled\" aria-label=\"Git repository has no remote\" title=\"Git repository has no remote\"><svg aria-hidden=\"true\" focusable=\"false\"><use href=\"#ghrm-icon-git\"></use></svg><span class=\"ghrm-source-text\">no remote</span></span>".to_string(),
        SourceState::NoRepo => String::new(),
    }
}

fn web_source_html(url: &str, label: &str, forge: Forge) -> String {
    let icon = forge_icon(forge);
    let href = html_escape::encode_double_quoted_attribute(url);
    let title_attr = html_escape::encode_double_quoted_attribute(url);

    let label_parts: Vec<&str> = label.split(" / ").collect();
    let after_scheme = url.find("://").map_or(0, |i| i + 3);
    let host_end = after_scheme
        + url[after_scheme..]
            .find('/')
            .unwrap_or(url.len() - after_scheme);
    let base = &url[..host_end];
    let path_segs: Vec<&str> = url[host_end..].trim_start_matches('/').split('/').collect();

    if label_parts.len() != path_segs.len() || path_segs.iter().any(|s| s.is_empty()) {
        return format!(
            "<a id=\"ghrm-source-slot\" class=\"ghrm-source-link\" href=\"{href}\" target=\"_blank\" rel=\"noopener noreferrer\" aria-label=\"Open source remote: {title_attr}\" title=\"Open source remote: {title_attr}\"><svg aria-hidden=\"true\" focusable=\"false\"><use href=\"#{icon}\"></use></svg><span class=\"ghrm-source-text\">{text}</span></a>",
            text = html_escape::encode_text(label),
        );
    }

    let n = label_parts.len();
    let mut segs = String::new();
    for (i, display) in label_parts.iter().enumerate() {
        if i > 0 {
            segs.push_str("<span class=\"ghrm-source-sep\"> / </span>");
        }
        let seg_url = format!("{}/{}", base, path_segs[..=i].join("/"));
        let seg_href = html_escape::encode_double_quoted_attribute(&seg_url);
        let seg_text = html_escape::encode_text(display);
        let class = if i + 1 == n {
            "ghrm-source-repo"
        } else {
            "ghrm-source-owner"
        };
        segs.push_str(&format!(
            "<a href=\"{seg_href}\" class=\"{class}\" target=\"_blank\" rel=\"noopener noreferrer\">{seg_text}</a>",
        ));
    }

    format!(
        "<span id=\"ghrm-source-slot\" class=\"ghrm-source-link\"><a href=\"{href}\" class=\"ghrm-source-icon-link\" target=\"_blank\" rel=\"noopener noreferrer\" aria-label=\"Open source remote: {title_attr}\" title=\"Open source remote: {title_attr}\"><svg aria-hidden=\"true\" focusable=\"false\"><use href=\"#{icon}\"></use></svg></a><span class=\"ghrm-source-text\">{segs}</span></span>",
    )
}

fn forge_icon(forge: Forge) -> &'static str {
    match forge {
        Forge::GitHub => "ghrm-icon-github",
        Forge::Bitbucket => "ghrm-icon-bitbucket",
        Forge::GitLab => "ghrm-icon-gitlab",
        Forge::Codeberg => "ghrm-icon-codeberg",
        Forge::SourceHut => "ghrm-icon-sourcehut",
        Forge::Generic => "ghrm-icon-git",
    }
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
    let scope = scope_from_query(&q, s.default_scope, s.has_ext_filter);
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
    scope: Scope,
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
    let crumbs = breadcrumb_html(root, s.home.as_deref(), rel, scope, s.default_scope);
    let raw_html = raw_blob_html(
        &text,
        path.extension()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty()),
    );
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
    respond_html_with_scope(
        &rendered,
        &body,
        s.repos.source_for(path),
        s.default_scope,
        s.has_ext_filter,
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
    fn scope_from_query_uses_default_when_missing() {
        let q = ScopeQuery::default();
        assert_eq!(scope_from_query(&q, Scope::All, false), Scope::All);
        assert_eq!(scope_from_query(&q, Scope::Filtered, true), Scope::Filtered);
    }

    #[test]
    fn with_scope_omits_default_scope() {
        assert_eq!(with_scope("/", Scope::All, Scope::All), "/");
        assert_eq!(with_scope("/", Scope::Filtered, Scope::Filtered), "/");
    }

    #[test]
    fn with_scope_preserves_non_default_scope() {
        assert_eq!(with_scope("/", Scope::All, Scope::Filtered), "/?scope=all");
        assert_eq!(
            with_scope("/docs/", Scope::Filtered, Scope::All),
            "/docs/?scope=filter"
        );
    }
}
