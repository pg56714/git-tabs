//! Runs the system `git` (and `ssh`) and keeps a log of the commands that change something.

use serde::Serialize;
use std::collections::VecDeque;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone, Serialize)]
pub struct LogEntry {
    pub id: u64,
    pub repo: String,
    pub cmd: String,
    pub ms: u64,
    pub ok: bool,
    pub output: String,
    pub at: u64,
}

struct LogBuf {
    next: u64,
    items: VecDeque<LogEntry>,
}

const LOG_CAP: usize = 500;

fn log_buf() -> &'static Mutex<LogBuf> {
    static LOG: OnceLock<Mutex<LogBuf>> = OnceLock::new();
    LOG.get_or_init(|| Mutex::new(LogBuf { next: 1, items: VecDeque::new() }))
}

fn exe_cell() -> &'static Mutex<String> {
    static EXE: OnceLock<Mutex<String>> = OnceLock::new();
    EXE.get_or_init(|| Mutex::new("git".into()))
}

pub fn set_git_exe(path: &str) {
    let p = path.trim();
    *exe_cell().lock().unwrap() = if p.is_empty() { "git".into() } else { p.into() };
}

pub fn git_exe() -> String {
    exe_cell().lock().unwrap().clone()
}

pub fn log_since(id: u64) -> Vec<LogEntry> {
    log_buf().lock().unwrap().items.iter().filter(|e| e.id > id).cloned().collect()
}

pub fn record(repo: &str, cmd: String, ms: u64, ok: bool, output: &str) {
    let mut b = log_buf().lock().unwrap();
    let id = b.next;
    b.next += 1;
    let at = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    let output: String = output.trim().chars().take(4000).collect();
    b.items.push_back(LogEntry { id, repo: repo.into(), cmd, ms, ok, output, at });
    while b.items.len() > LOG_CAP {
        b.items.pop_front();
    }
}

pub struct Out {
    pub stdout: String,
    pub stderr: String,
    pub code: i32,
}

impl Out {
    pub fn ok(&self) -> bool {
        self.code == 0
    }
    /// stderr if present, else stdout: what to show the user when a command fails.
    pub fn message(&self) -> String {
        let e = self.stderr.trim();
        if e.is_empty() { self.stdout.trim().to_string() } else { e.to_string() }
    }
}

pub fn command(program: &str) -> Command {
    let mut c = Command::new(program);
    // Never wait on a prompt nobody can answer; editors accept the default message.
    c.env("GIT_TERMINAL_PROMPT", "0").env("GIT_EDITOR", "true").env("GIT_SEQUENCE_EDITOR", "true").env("LC_ALL", "C");
    // `git status` otherwise rewrites .git/index to refresh stat info, which the file watcher
    // sees as a change and answers with another refresh.
    c.env("GIT_OPTIONAL_LOCKS", "0");
    c.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

fn quote(a: &str) -> String {
    if a.is_empty() || a.contains(|c: char| c.is_whitespace() || c == '"') { format!("\"{}\"", a.replace('"', "\\\"")) } else { a.to_string() }
}

/// Runs git in `repo`. `record` puts the command in the visible command log.
pub fn run(repo: Option<&Path>, args: &[&str], stdin: Option<&[u8]>, record_it: bool) -> Out {
    let exe = git_exe();
    let mut c = command(&exe);
    c.args(["-c", "core.quotepath=false", "-c", "color.ui=false", "-c", "core.pager=cat"]);
    if let Some(r) = repo {
        c.current_dir(r);
    }
    c.args(args);
    if stdin.is_some() {
        c.stdin(Stdio::piped());
    }
    let t = Instant::now();
    let out = match c.spawn() {
        Ok(mut child) => {
            if let (Some(input), Some(mut pipe)) = (stdin, child.stdin.take()) {
                let _ = pipe.write_all(input);
            }
            match child.wait_with_output() {
                Ok(o) => Out {
                    stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
                    stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
                    code: o.status.code().unwrap_or(-1),
                },
                Err(e) => Out { stdout: String::new(), stderr: e.to_string(), code: -1 },
            }
        }
        Err(e) => Out { stdout: String::new(), stderr: format!("找不到 git（{exe}）：{e}"), code: -1 },
    };
    if record_it {
        let cmd = std::iter::once("git".to_string()).chain(args.iter().map(|a| quote(a))).collect::<Vec<_>>().join(" ");
        let repo_s = repo.map(|r| r.display().to_string()).unwrap_or_default();
        let text = if out.ok() { format!("{}{}", out.stdout, out.stderr) } else { out.message() };
        record(&repo_s, cmd, t.elapsed().as_millis() as u64, out.ok(), &text);
    }
    out
}

/// Read-only git call; returns stdout or the error message.
pub fn read(repo: &Path, args: &[&str]) -> Result<String, String> {
    let o = run(Some(repo), args, None, false);
    if o.ok() { Ok(o.stdout) } else { Err(o.message()) }
}

/// Git call that changes something; shows up in the command log.
pub fn write(repo: &Path, args: &[&str]) -> Result<String, String> {
    let o = run(Some(repo), args, None, true);
    if o.ok() { Ok(format!("{}{}", o.stdout, o.stderr)) } else { Err(o.message()) }
}

pub fn write_stdin(repo: &Path, args: &[&str], input: &[u8]) -> Result<String, String> {
    let o = run(Some(repo), args, Some(input), true);
    if o.ok() { Ok(o.stdout) } else { Err(o.message()) }
}
