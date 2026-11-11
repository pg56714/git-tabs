//! Everything git-tabs does, behind one `dispatch(cmd, args)` entry point that both the
//! Tauri app and the browser dev server call.

pub mod config;
pub mod identity;
pub mod ops;
pub mod parse;
pub mod patch;
pub mod runner;

use config::{AppConfig, Identity};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub struct Ctx {
    pub config_dir: PathBuf,
    lock: Mutex<()>,
}

impl Ctx {
    pub fn new(config_dir: PathBuf) -> Self {
        let cfg = config::load(&config_dir);
        runner::set_git_exe(&cfg.git_path);
        Self { config_dir, lock: Mutex::new(()) }
    }
}

fn arg<T: DeserializeOwned>(v: &Value) -> Result<T, String> {
    serde_json::from_value(v.clone()).map_err(|e| format!("參數錯誤：{e}"))
}

fn ok<T: serde::Serialize>(v: T) -> Result<Value, String> {
    serde_json::to_value(v).map_err(|e| e.to_string())
}

#[derive(Deserialize)]
struct RepoArg {
    repo: String,
}

fn repo_path(a: &Value) -> Result<PathBuf, String> {
    let r: RepoArg = arg(a)?;
    let p = PathBuf::from(&r.repo);
    if !p.is_dir() {
        return Err(format!("找不到資料夾 {}", r.repo));
    }
    Ok(p)
}

fn s<'a>(a: &'a Value, k: &str) -> &'a str {
    a.get(k).and_then(Value::as_str).unwrap_or("")
}

fn opt<'a>(a: &'a Value, k: &str) -> Option<&'a str> {
    a.get(k).and_then(Value::as_str).filter(|s| !s.is_empty())
}

fn b(a: &Value, k: &str) -> bool {
    a.get(k).and_then(Value::as_bool).unwrap_or(false)
}

fn n(a: &Value, k: &str, d: usize) -> usize {
    a.get(k).and_then(Value::as_u64).map(|x| x as usize).unwrap_or(d)
}

fn strings(a: &Value, k: &str) -> Vec<String> {
    a.get(k).and_then(Value::as_array).map(|v| v.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default()
}

pub fn dispatch(ctx: &Ctx, cmd: &str, a: Value) -> Result<Value, String> {
    match cmd {
        // ----- app settings -----
        "config_get" => ok(config::load(&ctx.config_dir)),
        "config_save" => {
            let _g = ctx.lock.lock().unwrap();
            let mut cfg: AppConfig = arg(&a["config"])?;
            // The list of lines we wrote into the global gitconfig is ours to keep.
            cfg.managed = config::load(&ctx.config_dir).managed;
            runner::set_git_exe(&cfg.git_path);
            config::save(&ctx.config_dir, &cfg)?;
            ok(true)
        }
        "identities_apply" => {
            let _g = ctx.lock.lock().unwrap();
            let mut cfg = config::load(&ctx.config_dir);
            let ids: Vec<Identity> = arg(&a["identities"])?;
            let added = identity::apply(&ctx.config_dir, &ids, &cfg.managed)?;
            cfg.identities = ids;
            cfg.managed = added;
            config::save(&ctx.config_dir, &cfg)?;
            ok(cfg.managed.len())
        }
        "identity_pin" => {
            let repo = repo_path(&a)?;
            let id: Option<Identity> = a.get("identity").filter(|v| !v.is_null()).map(arg).transpose()?;
            identity::pin(&repo, id.as_ref())?;
            ok(true)
        }
        "ssh_test" => ok(identity::ssh_test(s(&a, "key"), s(&a, "host"))),
        "ssh_keygen" => ok(identity::keygen(s(&a, "path"), s(&a, "email"), s(&a, "passphrase"))?),
        "identities_detect" => {
            let home = PathBuf::from(std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).unwrap_or_default());
            let cfg = config::load(&ctx.config_dir);
            let ours: Vec<String> = cfg.managed.iter().map(|m| m.value.clone()).collect();
            ok(identity::detect(&home, &ours))
        }
        "home_dir" => ok(std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).unwrap_or_default()),
        "git_version" => {
            let o = runner::run(None, &["--version"], None, false);
            if o.ok() { ok(o.stdout.trim()) } else { Err(o.message()) }
        }
        "command_log" => ok(runner::log_since(a.get("since").and_then(Value::as_u64).unwrap_or(0))),

        // ----- repos on disk -----
        "repo_resolve" => ok(ops::toplevel(Path::new(s(&a, "path")))?.display().to_string()),
        "scan_dir" => ok(ops::scan(Path::new(s(&a, "path")))),
        "clone" => ok(ops::clone(s(&a, "url"), Path::new(s(&a, "dir")))?),
        "open_folder" => open_folder(Path::new(s(&a, "path"))),
        "open_terminal" => open_terminal(Path::new(s(&a, "path"))),
        "open_file" => open_file(Path::new(s(&a, "path"))),

        // ----- reading -----
        "summary" => ok(ops::summary(&repo_path(&a)?)?),
        "snapshot" => ok(ops::snapshot(&repo_path(&a)?)?),
        "status" => ok(ops::status(&repo_path(&a)?)?),
        "diff_file" if b(&a, "conflicted") => ok(ops::conflict_view(&repo_path(&a)?, s(&a, "path"))),
        "diff_file" => ok(ops::diff_file(&repo_path(&a)?, s(&a, "path"), b(&a, "staged"), b(&a, "untracked"))?),
        "log" => ok(ops::log(&repo_path(&a)?, n(&a, "skip", 0), n(&a, "limit", 300), a.get("all").and_then(Value::as_bool).unwrap_or(true), s(&a, "query"), opt(&a, "path"))?),
        "commit_detail" => ok(ops::commit_detail(&repo_path(&a)?, s(&a, "sha"))?),
        "commit_file_diff" => ok(ops::commit_file_diff(&repo_path(&a)?, s(&a, "sha"), s(&a, "path"), opt(&a, "old_path"))?),
        "refs" => ok(ops::refs(&repo_path(&a)?)?),
        "range_files" => {
            let repo = repo_path(&a)?;
            let (base, head) = (s(&a, "base"), s(&a, "head"));
            ok(json!({ "files": ops::range_files(&repo, base, head)?, "commits": ops::range_commits(&repo, base, head)? }))
        }
        "range_file_diff" => ok(ops::range_file_diff(&repo_path(&a)?, s(&a, "base"), s(&a, "head"), s(&a, "path"), opt(&a, "old_path"))?),
        "reflog" => ok(ops::reflog(&repo_path(&a)?, n(&a, "limit", 200))?),
        "blame" => ok(ops::blame(&repo_path(&a)?, s(&a, "path"), opt(&a, "rev"))?),
        "last_message" => ok(ops::last_message(&repo_path(&a)?)?),

        // ----- changing things -----
        "stage" => ok(ops::stage(&repo_path(&a)?, &strings(&a, "paths"))?),
        "unstage" => ok(ops::unstage(&repo_path(&a)?, &strings(&a, "paths"))?),
        "discard" => ok(ops::discard(&repo_path(&a)?, &strings(&a, "paths"))?),
        "apply_lines" => {
            let action: ops::LineAction = arg(&a["action"])?;
            let lines: Vec<(usize, usize)> = arg(&a["lines"])?;
            ok(ops::apply_lines(&repo_path(&a)?, s(&a, "path"), action, &lines)?)
        }
        "commit" => ok(ops::commit(&repo_path(&a)?, s(&a, "message"), b(&a, "amend"))?),
        "fetch" => ok(ops::fetch(&repo_path(&a)?)?),
        "pull" => ok(ops::pull(&repo_path(&a)?, s(&a, "mode"))?),
        "push" => ok(ops::push(&repo_path(&a)?, b(&a, "force"))?),
        "switch" => ok(ops::switch(&repo_path(&a)?, s(&a, "name"))?),
        "create_branch" => ok(ops::create_branch(&repo_path(&a)?, s(&a, "name"), opt(&a, "from"), b(&a, "checkout"))?),
        "delete_branch" => ok(ops::delete_branch(&repo_path(&a)?, s(&a, "name"), b(&a, "force"))?),
        "tag" => ok(ops::tag(&repo_path(&a)?, s(&a, "name"), s(&a, "sha"), s(&a, "message"))?),
        "stash_push" => ok(ops::stash_push(&repo_path(&a)?, s(&a, "message"), b(&a, "untracked"))?),
        "op" => ok(ops::simple(&repo_path(&a)?, s(&a, "op"), s(&a, "arg"))?),
        _ => Err(format!("未知的指令 {cmd}")),
    }
}

fn spawn(program: &str, args: &[&str], dir: Option<&Path>) -> Result<(), String> {
    let mut c = std::process::Command::new(program);
    c.args(args);
    if let Some(d) = dir {
        c.current_dir(d);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    c.spawn().map(|_| ()).map_err(|e| e.to_string())
}

fn open_folder(p: &Path) -> Result<Value, String> {
    #[cfg(windows)]
    // Explorer falls back to Documents when the path has forward slashes (git prints C:/…).
    spawn("explorer", &[&p.display().to_string().replace('/', "\\")], None)?;
    #[cfg(target_os = "macos")]
    spawn("open", &[&p.display().to_string()], None)?;
    #[cfg(all(unix, not(target_os = "macos")))]
    spawn("xdg-open", &[&p.display().to_string()], None)?;
    ok(true)
}

fn open_terminal(p: &Path) -> Result<Value, String> {
    let d = p.display().to_string();
    #[cfg(windows)]
    {
        // Backslashes, and also the working directory, so the shell starts in the repo.
        let d = d.replace('/', "\\");
        if spawn("wt", &["-d", &d], Some(p)).is_err() {
            spawn("cmd", &["/c", "start", "", "cmd", "/K"], Some(p))?;
        }
    }
    #[cfg(target_os = "macos")]
    spawn("open", &["-a", "Terminal", &d], None)?;
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = &d;
        // Debian/Ubuntu's alternative, then the common desktops' own terminals.
        let found = ["x-terminal-emulator", "gnome-terminal", "konsole", "xfce4-terminal", "xterm"].iter().any(|t| has(t) && spawn(t, &[], Some(p)).is_ok());
        if !found {
            return Err("找不到終端機程式".into());
        }
    }
    ok(true)
}

/// Whether a program is on PATH.
fn has(program: &str) -> bool {
    let finder = if cfg!(windows) { "where" } else { "which" };
    let mut c = std::process::Command::new(finder);
    c.arg(program).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    c.status().map(|s| s.success()).unwrap_or(false)
}

/// Opens a file in VS Code when it is installed, else with the system's default app.
fn open_file(p: &Path) -> Result<Value, String> {
    let d = p.display().to_string();
    if has("code") {
        // On Windows `code` is a .cmd script, so it has to go through cmd.
        #[cfg(windows)]
        return spawn("cmd", &["/c", "code", &d], None).and_then(|_| ok(true));
        #[cfg(not(windows))]
        return spawn("code", &[&d], None).and_then(|_| ok(true));
    }
    #[cfg(windows)]
    spawn("explorer", &[&d], None)?;
    #[cfg(target_os = "macos")]
    spawn("open", &[&d], None)?;
    #[cfg(all(unix, not(target_os = "macos")))]
    spawn("xdg-open", &[&d], None)?;
    ok(true)
}
