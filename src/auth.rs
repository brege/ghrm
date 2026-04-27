use crate::server::AppState;

use anyhow::Result;
use axum::{
    body::Body,
    extract::{ConnectInfo, Form, Query, State},
    http::{HeaderMap, Request, StatusCode, Uri, header},
    middleware::Next,
    response::{Html, IntoResponse, Response},
};
use hmac::{Hmac, Mac};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde::Deserialize;
use std::net::SocketAddr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<sha2::Sha256>;

pub struct AuthConfig {
    pub username: String,
    pub password: String,
}

#[derive(Clone)]
pub struct AuthState {
    username: String,
    password: String,
    cookie_name: String,
}

impl AuthState {
    pub fn new(auth: AuthConfig, port: u16) -> Result<Self> {
        if auth.password.is_empty() {
            anyhow::bail!("auth.password must not be empty");
        }
        Ok(Self {
            username: auth.username,
            password: auth.password,
            cookie_name: format!("ghrm_{port}_sid"),
        })
    }
}

const SESSION_TTL: Duration = Duration::from_secs(60 * 60 * 12);

#[derive(Default, Deserialize)]
pub struct LoginQuery {
    next: Option<String>,
    error: Option<u8>,
}

#[derive(Deserialize)]
pub struct LoginFormData {
    username: String,
    password: String,
    next: Option<String>,
}

pub async fn require(
    State(s): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let Some(auth) = s.auth.as_ref() else {
        return next.run(req).await;
    };
    if addr.ip().is_loopback() {
        return next.run(req).await;
    }
    if has_session(auth, req.headers()) {
        return next.run(req).await;
    }
    challenge(req.uri())
}

pub async fn login_form(
    State(s): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<LoginQuery>,
) -> Response {
    let Some(auth) = s.auth.as_ref() else {
        return not_found();
    };
    let next = sanitize_next(q.next.as_deref());
    if has_session(auth, &headers) {
        return redirect(&next);
    }
    Html(login_html(&auth.username, &next, q.error == Some(1))).into_response()
}

pub async fn login_submit(State(s): State<AppState>, Form(form): Form<LoginFormData>) -> Response {
    let Some(auth) = s.auth.as_ref() else {
        return not_found();
    };
    let next = sanitize_next(form.next.as_deref());
    if form.username != auth.username || !verify_password(auth, &form.password) {
        let loc = format!(
            "/_ghrm/login?next={}&error=1",
            utf8_percent_encode(&next, NON_ALPHANUMERIC)
        );
        return redirect(&loc);
    }

    let token = session_token(auth, expires_at());

    Response::builder()
        .status(StatusCode::SEE_OTHER)
        .header(header::LOCATION, next)
        .header(
            header::SET_COOKIE,
            session_cookie(&auth.cookie_name, &token),
        )
        .body(Body::empty())
        .unwrap()
}

pub async fn logout(State(s): State<AppState>) -> Response {
    let Some(auth) = s.auth.as_ref() else {
        return not_found();
    };
    Response::builder()
        .status(StatusCode::SEE_OTHER)
        .header(header::LOCATION, "/_ghrm/login")
        .header(header::SET_COOKIE, clear_session_cookie(&auth.cookie_name))
        .body(Body::empty())
        .unwrap()
}

fn verify_password(auth: &AuthState, password: &str) -> bool {
    auth.password == password
}

fn has_session(auth: &AuthState, headers: &HeaderMap) -> bool {
    let Some(token) = cookie_value(headers, &auth.cookie_name) else {
        return false;
    };
    verify_token(auth, &token)
}

fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|part| {
            let (key, value) = part.trim().split_once('=')?;
            (key == name).then(|| value.to_string())
        })
}

fn challenge(uri: &Uri) -> Response {
    if uri.path().starts_with("/_ghrm/") {
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body(Body::empty())
            .unwrap();
    }

    let next = request_target(uri);
    let loc = format!(
        "/_ghrm/login?next={}",
        utf8_percent_encode(&next, NON_ALPHANUMERIC)
    );
    redirect(&loc)
}

fn request_target(uri: &Uri) -> String {
    uri.path_and_query()
        .map(|value| value.as_str().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "/".to_string())
}

fn sanitize_next(next: Option<&str>) -> String {
    let Some(next) = next.filter(|next| !next.is_empty()) else {
        return "/".to_string();
    };
    if !next.starts_with('/')
        || next.starts_with("//")
        || next.starts_with("/_ghrm/login")
        || next.starts_with("/_ghrm/logout")
    {
        return "/".to_string();
    }
    next.to_string()
}

fn redirect(loc: &str) -> Response {
    Response::builder()
        .status(StatusCode::SEE_OTHER)
        .header(header::LOCATION, loc)
        .body(Body::empty())
        .unwrap()
}

fn session_cookie(name: &str, token: &str) -> String {
    format!(
        "{name}={token}; Path=/; Max-Age={}; HttpOnly; SameSite=Lax",
        SESSION_TTL.as_secs()
    )
}

fn clear_session_cookie(name: &str) -> String {
    format!("{name}=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax")
}

fn expires_at() -> u64 {
    now_secs() + SESSION_TTL.as_secs()
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn session_token(auth: &AuthState, expires: u64) -> String {
    let sig = sign(auth, expires);
    format!("{expires}.{sig}")
}

fn verify_token(auth: &AuthState, token: &str) -> bool {
    let Some((exp, sig)) = token.split_once('.') else {
        return false;
    };
    let Ok(expires) = exp.parse::<u64>() else {
        return false;
    };
    if expires <= now_secs() {
        return false;
    }
    sign(auth, expires) == sig
}

fn sign(auth: &AuthState, expires: u64) -> String {
    let mut mac = HmacSha256::new_from_slice(auth.password.as_bytes()).expect("hmac key");
    mac.update(auth.username.as_bytes());
    mac.update(b":");
    mac.update(expires.to_string().as_bytes());
    hex_lower(&mac.finalize().into_bytes())
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn login_html(username: &str, next: &str, invalid: bool) -> String {
    let user = html_escape::encode_text(username);
    let next = html_escape::encode_double_quoted_attribute(next);
    let error = if invalid {
        "<p class=\"ghrm-login-error\">Invalid username or password</p>"
    } else {
        ""
    };

    format!(
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>Sign in</title><style>body{{margin:0;font-family:-apple-system,BlinkMacSystemFont,\"Segoe UI\",\"Noto Sans\",Helvetica,Arial,sans-serif;background:#f6f8fa;color:#1f2328}}main{{min-height:100vh;display:grid;place-items:center;padding:24px}}form{{width:min(100%,360px);padding:24px;border:1px solid #d0d7de;border-radius:8px;background:#fff;box-shadow:0 1px 2px rgba(31,35,40,.04)}}h1{{margin:0 0 8px;font-size:20px}}p{{margin:0 0 16px;color:#636c76;font-size:14px;line-height:1.5}}label{{display:block;margin:0 0 6px;font-size:14px;font-weight:600}}input{{width:100%;height:36px;margin:0 0 14px;padding:0 10px;border:1px solid #d0d7de;border-radius:6px;font:inherit}}button{{width:100%;height:36px;border:1px solid #1f2328;border-radius:6px;background:#1f2328;color:#fff;font:inherit;font-weight:600;cursor:pointer}}.ghrm-login-error{{margin:0 0 14px;color:#d1242f}}</style></head><body><main><form method=\"post\" action=\"/_ghrm/login\"><h1>Sign in</h1><p>Use the configured account for {user}</p>{error}<input type=\"hidden\" name=\"next\" value=\"{next}\"><label for=\"username\">Username</label><input id=\"username\" name=\"username\" type=\"text\" autocomplete=\"username\" required><label for=\"password\">Password</label><input id=\"password\" name=\"password\" type=\"password\" autocomplete=\"current-password\" required><button type=\"submit\">Sign in</button></form></main></body></html>"
    )
}

fn not_found() -> Response {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::empty())
        .unwrap()
}
