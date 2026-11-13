// No console window behind the release build on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod actions;
mod app;
mod debug;
mod diffmap;
mod format;
mod graph;
mod i18n;
mod identity;
mod model;
mod modals;
mod overlays;
mod theme;
mod tree;
mod ui;
mod views;

use gpui_kit::{AppContext as _, WindowOptions};
use std::path::PathBuf;
use std::sync::Arc;

/// `--config-dir <dir>`  where config.json lives (default: the same folder the Tauri build used)
/// `--git-global <file>` use this file as the global gitconfig, so testing never touches yours
/// `--check <repo>`      print a repo's summary and changed files, then exit
fn main() {
    let args: Vec<String> = std::env::args().collect();
    // While a commit is being rewritten, git runs this same program as its editor (gitcore::rewrite).
    if let Some(code) = gitcore::rewrite::editor_main(&args) {
        std::process::exit(code);
    }
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    if let Some(g) = flag("--git-global") {
        // SAFETY: set before any other thread starts.
        unsafe { std::env::set_var("GIT_CONFIG_GLOBAL", &g) };
    }
    let dir: PathBuf = flag("--config-dir")
        .or_else(|| std::env::var("GIT_TABS_CONFIG_DIR").ok())
        .map(Into::into)
        // Tauri's app_config_dir for identifier `app.git-tabs.desktop`: same folder, same settings.
        .unwrap_or_else(|| dirs::config_dir().unwrap_or_else(std::env::temp_dir).join("app.git-tabs.desktop"));

    if let Some(repo) = flag("--check") {
        let repo = PathBuf::from(repo);
        let _ctx = gitcore::Ctx::new(dir);
        match gitcore::ops::summary(&repo).and_then(|s| gitcore::ops::status(&repo).map(|st| (s, st))) {
            Ok((s, st)) => {
                println!("{} on {:?} +{} -{}", s.name, s.branch, s.ahead, s.behind);
                for f in st {
                    println!("{}{} {}", f.x, f.y, f.path);
                }
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }

    crate::debug::log("start", || format!("git-tabs {} on {}", env!("CARGO_PKG_VERSION"), std::env::consts::OS));
    let place = app::load_window(&dir);
    let ctx = Arc::new(gitcore::Ctx::new(dir));
    gpui_kit::application().with_assets(ui::AppAssets).run(move |cx| {
        gpui_kit::init(cx);
        views::bind_keys(cx);
        let options = WindowOptions {
            window_bounds: Some(app::initial_window(place, cx)),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, move |window, cx| cx.new(|cx| app::GitTabs::new(ctx.clone(), window, cx)))
            .expect("failed to open the window");
        cx.activate(true);
    });
}
