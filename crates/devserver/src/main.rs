//! Development only: serves the same commands as the desktop app over HTTP on 127.0.0.1,
//! so the UI can be opened and tested in a normal browser.
//!
//! Requests must carry `X-Git-Tabs: 1`; browsers won't send that header cross-origin
//! without a CORS preflight this server never approves, so other sites can't call it.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use gitcore::{Ctx, dispatch};
use serde_json::Value;
use std::sync::Arc;

/// `--config-dir <dir>`: where config.json lives (default: %TEMP%\git-tabs-dev)
/// `--git-global <file>`: use this file as the global gitconfig, so testing never touches yours
#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    if let Some(g) = flag("--git-global") {
        // SAFETY: set before any other thread starts.
        unsafe { std::env::set_var("GIT_CONFIG_GLOBAL", &g) };
        eprintln!("git-tabs devserver: global gitconfig {g}");
    }
    let dir: std::path::PathBuf = flag("--config-dir").or_else(|| std::env::var("GIT_TABS_CONFIG_DIR").ok()).map(Into::into).unwrap_or_else(|| std::env::temp_dir().join("git-tabs-dev"));
    eprintln!("git-tabs devserver: config {}", std::path::Path::new(&dir).display());
    let ctx = Arc::new(Ctx::new(dir));
    let app = Router::new().route("/api/{cmd}", post(call)).with_state(ctx);
    let port = std::env::var("GIT_TABS_DEV_PORT").unwrap_or_else(|_| "5319".into());
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}")).await.expect("bind");
    eprintln!("listening on http://127.0.0.1:{port}");
    axum::serve(listener, app).await.unwrap();
}

async fn call(State(ctx): State<Arc<Ctx>>, Path(cmd): Path<String>, headers: HeaderMap, Json(args): Json<Value>) -> (StatusCode, Json<Value>) {
    if headers.get("x-git-tabs").and_then(|v| v.to_str().ok()) != Some("1") {
        return (StatusCode::FORBIDDEN, Json(Value::String("missing header".into())));
    }
    match tokio::task::spawn_blocking(move || dispatch(&ctx, &cmd, args)).await {
        Ok(Ok(v)) => (StatusCode::OK, Json(v)),
        Ok(Err(e)) => (StatusCode::UNPROCESSABLE_ENTITY, Json(Value::String(e))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(Value::String(e.to_string()))),
    }
}
