//! Optional text log for debugging: set `GIT_TABS_LOG=1` (file in the temp folder) or
//! `GIT_TABS_LOG=<path>` and every menu, dialog, toast and git command is appended to it.
//! Without the variable nothing is written.

use std::io::Write as _;
use std::sync::OnceLock;

static FILE: OnceLock<Option<std::path::PathBuf>> = OnceLock::new();

fn target() -> &'static Option<std::path::PathBuf> {
    FILE.get_or_init(|| match std::env::var("GIT_TABS_LOG") {
        Ok(v) if v.is_empty() || v == "0" => None,
        Ok(v) if v == "1" => Some(std::env::temp_dir().join("git-tabs-debug.log")),
        Ok(v) => Some(v.into()),
        Err(_) => None,
    })
}

/// `debug::log("menu", format!(...))`; the message is only built when logging is on.
pub fn log(kind: &str, msg: impl FnOnce() -> String) {
    let Some(path) = target() else { return };
    let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
    let line = format!("{}.{:03} {:<8} {}\n", ms / 1000, ms % 1000, kind, msg().replace('\n', "\n"));
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = f.write_all(line.as_bytes());
    }
}
