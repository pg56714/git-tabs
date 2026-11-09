//! Desktop shell: one `call` command into gitcore, plus a file watcher that tells the UI
//! when a repo changed on disk.

use gitcore::{Ctx, dispatch};
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer, notify::RecommendedWatcher, notify::RecursiveMode};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{Emitter, Manager, State};

struct Watch(Mutex<Option<Debouncer<RecommendedWatcher>>>);

#[tauri::command]
async fn call(ctx: State<'_, Arc<Ctx>>, cmd: String, args: Value) -> Result<Value, String> {
    let ctx = ctx.inner().clone();
    tauri::async_runtime::spawn_blocking(move || dispatch(&ctx, &cmd, args)).await.map_err(|e| e.to_string())?
}

/// Replaces the set of watched repos. Emits "repo-changed" with the repo path.
#[tauri::command]
fn watch(app: tauri::AppHandle, state: State<'_, Watch>, repos: Vec<String>) -> Result<(), String> {
    let roots: Vec<PathBuf> = repos.iter().map(PathBuf::from).collect();
    let r2 = roots.clone();
    let handle = app.clone();
    let mut deb = new_debouncer(Duration::from_millis(400), move |res: DebounceEventResult| {
        let Ok(events) = res else { return };
        let mut hit: Vec<&PathBuf> = Vec::new();
        for e in &events {
            let p = &e.path;
            // Object writes and lock files churn constantly during git commands, and build
            // output folders (normally gitignored) churn during builds.
            let s = p.to_string_lossy().replace('\\', "/");
            const NOISY: [&str; 8] = ["/.git/objects", "/node_modules/", "/target/", "/.venv/", "/__pycache__/", "/dist/", "/.next/", "/.mypy_cache/"];
            if s.ends_with(".lock") || NOISY.iter().any(|n| s.contains(n)) {
                continue;
            }
            if let Some(root) = r2.iter().filter(|r| p.starts_with(r)).max_by_key(|r| r.as_os_str().len()) {
                if !hit.contains(&root) {
                    hit.push(root);
                }
            }
        }
        for root in hit {
            let _ = handle.emit("repo-changed", root.display().to_string());
        }
    })
    .map_err(|e| e.to_string())?;
    for r in &roots {
        let _ = deb.watcher().watch(r, RecursiveMode::Recursive);
    }
    *state.0.lock().unwrap() = Some(deb);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            app.manage(Arc::new(Ctx::new(dir)));
            app.manage(Watch(Mutex::new(None)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![call, watch])
        .run(tauri::generate_context!())
        .expect("error while running git-tabs");
}
