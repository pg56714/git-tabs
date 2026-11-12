//! Repository operations, one function per thing the UI can ask for.

use crate::parse::{self, ChangedFile, Commit, FileDiff, Refs, ReflogEntry, Stash, StatusEntry, Worktree};
use crate::patch;
use crate::runner::{read, run, write, write_stdin};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};
use std::path::{Path, PathBuf};

const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

pub fn toplevel(path: &Path) -> Result<PathBuf, String> {
    let s = read(path, &["rev-parse", "--show-toplevel"]).map_err(|_| format!("{} 不是 git repo", path.display()))?;
    Ok(PathBuf::from(s.trim()))
}

/// The .git folder never moves, so it is looked up once per repo.
fn git_dir(repo: &Path) -> Option<PathBuf> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, PathBuf>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(d) = cache.lock().unwrap().get(repo) {
        return Some(d.clone());
    }
    let d = PathBuf::from(read(repo, &["rev-parse", "--absolute-git-dir"]).ok()?.trim());
    cache.lock().unwrap().insert(repo.to_path_buf(), d.clone());
    Some(d)
}

// ---------- summary / status ----------

#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub path: String,
    pub name: String,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub changes: usize,
    pub conflicts: usize,
    pub remote_url: Option<String>,
    /// "merge", "rebase", "cherry-pick", "revert" or None
    pub state: Option<&'static str>,
}

pub fn summary(repo: &Path) -> Result<Summary, String> {
    let (st, remote_url) = std::thread::scope(|sc| {
        let u = sc.spawn(|| remote_url(repo));
        (status_raw(repo), u.join().ok().flatten())
    });
    let (b, files) = st?;
    Ok(make_summary(repo, b, &files, remote_url))
}

fn make_summary(repo: &Path, b: parse::BranchInfo, files: &[StatusEntry], remote_url: Option<String>) -> Summary {
    let state = git_dir(repo).and_then(|g| {
        if g.join("rebase-merge").exists() || g.join("rebase-apply").exists() {
            Some("rebase")
        } else if g.join("MERGE_HEAD").exists() {
            Some("merge")
        } else if g.join("CHERRY_PICK_HEAD").exists() {
            Some("cherry-pick")
        } else if g.join("REVERT_HEAD").exists() {
            Some("revert")
        } else {
            None
        }
    });
    Summary {
        path: repo.display().to_string(),
        name: repo.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        branch: b.head,
        head: b.oid,
        upstream: b.upstream,
        ahead: b.ahead,
        behind: b.behind,
        changes: files.len(),
        conflicts: files.iter().filter(|f| f.conflicted).count(),
        remote_url,
        state,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub summary: Summary,
    pub status: Vec<StatusEntry>,
    pub refs: RefsAll,
}

/// Everything a refresh needs, with the git calls running in parallel (each spawn costs
/// tens of milliseconds on Windows).
pub fn snapshot(repo: &Path) -> Result<Snapshot, String> {
    std::thread::scope(|sc| {
        let u = sc.spawn(|| remote_url(repo));
        let rf = sc.spawn(|| refs(repo));
        let st = status_raw(repo);
        let url = u.join().ok().flatten();
        let refs = rf.join().map_err(|_| "refs 讀取失敗".to_string())??;
        let (b, files) = st?;
        Ok(Snapshot { summary: make_summary(repo, b, &files, url), status: files, refs })
    })
}

/// URL of "origin", or of the first remote when there is no origin.
pub fn remote_url(repo: &Path) -> Option<String> {
    let out = read(repo, &["config", "--get-regexp", r"^remote\..*\.url$"]).ok()?;
    let urls: Vec<(&str, &str)> = out.lines().filter_map(|l| l.split_once(' ')).collect();
    urls.iter().find(|(k, _)| *k == "remote.origin.url").or_else(|| urls.first()).map(|(_, v)| v.trim().to_string())
}

fn status_raw(repo: &Path) -> Result<(parse::BranchInfo, Vec<StatusEntry>), String> {
    let out = read(repo, &["status", "--porcelain=v2", "--branch", "-z", "--untracked-files=all"])?;
    Ok(parse::parse_status(&out))
}

pub fn status(repo: &Path) -> Result<Vec<StatusEntry>, String> {
    Ok(status_raw(repo)?.1)
}

// ---------- diffs ----------

fn one(mut v: Vec<FileDiff>, path: &str) -> FileDiff {
    if v.is_empty() {
        FileDiff { path: path.into(), ..Default::default() }
    } else {
        v.swap_remove(0)
    }
}

/// Diff of one changed file. `staged` = index vs HEAD, otherwise worktree vs index.
pub fn diff_file(repo: &Path, path: &str, staged: bool, untracked: bool) -> Result<FileDiff, String> {
    if untracked {
        return Ok(new_file_diff(&repo.join(path), path));
    }
    let mut args = vec!["diff", "--no-color", "--no-ext-diff", "-M"];
    if staged {
        args.push("--cached");
    }
    args.extend(["--", path]);
    Ok(one(parse::parse_diff(&read(repo, &args)?), path))
}

/// A conflicted file as one block: our side (HEAD) as removed lines, theirs as added lines,
/// so the conflict regions stand out. git's own `--cc` diff is hard to read.
pub fn conflict_view(repo: &Path, path: &str) -> FileDiff {
    let mut d = FileDiff { path: path.into(), ..Default::default() };
    let Ok(text) = std::fs::read_to_string(repo.join(path)) else {
        d.binary = true;
        return d;
    };
    #[derive(PartialEq)]
    enum Side {
        Both,
        Ours,
        Base,
        Theirs,
    }
    let mut side = Side::Both;
    let mut lines = Vec::new();
    for l in text.lines() {
        let (t, next) = if l.starts_with("<<<<<<<") {
            (' ', Side::Ours)
        } else if l.starts_with("|||||||") && side == Side::Ours {
            (' ', Side::Base)
        } else if l.starts_with("=======") && side != Side::Both {
            (' ', Side::Theirs)
        } else if l.starts_with(">>>>>>>") && side == Side::Theirs {
            (' ', Side::Both)
        } else {
            (match side { Side::Ours => '-', Side::Theirs => '+', _ => ' ' }, side)
        };
        lines.push(parse::DiffLine { t, text: l.to_string(), no_eol: false });
        side = next;
    }
    let n = lines.len() as u32;
    d.hunks.push(parse::Hunk { header: "@@ 衝突：紅色是我的版本（HEAD），綠色是對方的版本 @@".into(), old_start: 1, old_lines: n, new_start: 1, new_lines: n, lines });
    d
}

/// An untracked file shown as all-added lines (cheaper than spawning `git diff --no-index`).
fn new_file_diff(full: &Path, path: &str) -> FileDiff {
    let mut d = FileDiff { path: path.into(), new_file: true, ..Default::default() };
    let Ok(bytes) = std::fs::read(full) else { return d };
    if bytes.len() > 8 * 1024 * 1024 || bytes[..bytes.len().min(8000)].contains(&0) {
        d.binary = true;
        return d;
    }
    let text = String::from_utf8_lossy(&bytes);
    if text.is_empty() {
        return d;
    }
    let no_eol = !text.ends_with('\n');
    let mut lines: Vec<parse::DiffLine> = text.lines().map(|l| parse::DiffLine { t: '+', text: l.to_string(), no_eol: false }).collect();
    if let Some(l) = lines.last_mut() {
        l.no_eol = no_eol;
    }
    let n = lines.len() as u32;
    d.hunks.push(parse::Hunk { header: format!("@@ -0,0 +1,{n} @@"), old_start: 0, old_lines: 0, new_start: 1, new_lines: n, lines });
    d
}

pub fn commit_file_diff(repo: &Path, sha: &str, path: &str, old_path: Option<&str>) -> Result<FileDiff, String> {
    let parent = first_parent(repo, sha);
    let base = parent.as_deref().unwrap_or(EMPTY_TREE);
    let mut args = vec!["diff", "--no-color", "--no-ext-diff", "-M", base, sha, "--"];
    if let Some(o) = old_path {
        args.push(o);
    }
    args.push(path);
    Ok(one(parse::parse_diff(&read(repo, &args)?), path))
}

pub fn range_file_diff(repo: &Path, base: &str, head: &str, path: &str, old_path: Option<&str>) -> Result<FileDiff, String> {
    let range = format!("{base}...{head}");
    let mut args = vec!["diff", "--no-color", "--no-ext-diff", "-M", range.as_str(), "--"];
    if let Some(o) = old_path {
        args.push(o);
    }
    args.push(path);
    Ok(one(parse::parse_diff(&read(repo, &args)?), path))
}

fn first_parent(repo: &Path, sha: &str) -> Option<String> {
    let s = read(repo, &["rev-list", "--parents", "-n", "1", sha]).ok()?;
    s.split_whitespace().nth(1).map(String::from)
}

fn changed_files(repo: &Path, a: &str, b: Option<&str>) -> Result<Vec<ChangedFile>, String> {
    let mut ns = vec!["diff", "--name-status", "-z", "-M", a];
    let mut num = vec!["diff", "--numstat", "-z", "-M", a];
    if let Some(b) = b {
        ns.push(b);
        num.push(b);
    }
    let mut files = parse::parse_name_status(&read(repo, &ns)?);
    parse::apply_numstat(&mut files, &read(repo, &num)?);
    Ok(files)
}

// ---------- staging ----------

pub fn stage(repo: &Path, paths: &[String]) -> Result<(), String> {
    let mut args = vec!["add", "-A", "--"];
    args.extend(paths.iter().map(String::as_str));
    write(repo, &args).map(|_| ())
}

pub fn unstage(repo: &Path, paths: &[String]) -> Result<(), String> {
    let has_head = read(repo, &["rev-parse", "--verify", "-q", "HEAD"]).is_ok();
    let mut args = if has_head { vec!["restore", "--staged", "--"] } else { vec!["rm", "--cached", "-r", "-q", "--"] };
    args.extend(paths.iter().map(String::as_str));
    write(repo, &args).map(|_| ())
}

/// Throws away worktree changes. Untracked files are deleted.
pub fn discard(repo: &Path, paths: &[String]) -> Result<(), String> {
    let st = status(repo)?;
    let (untracked, tracked): (Vec<&String>, Vec<&String>) = paths.iter().partition(|p| st.iter().any(|s| s.untracked && &s.path == *p));
    if !tracked.is_empty() {
        let mut args = vec!["restore", "--worktree", "--"];
        args.extend(tracked.iter().map(|s| s.as_str()));
        write(repo, &args)?;
    }
    if !untracked.is_empty() {
        let mut args = vec!["clean", "-f", "-q", "--"];
        args.extend(untracked.iter().map(|s| s.as_str()));
        write(repo, &args)?;
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LineAction {
    Stage,
    Unstage,
    Discard,
}

/// Stages, unstages or discards only the selected (hunk, line) pairs of one file.
pub fn apply_lines(repo: &Path, path: &str, action: LineAction, lines: &[(usize, usize)]) -> Result<(), String> {
    let untracked = status(repo)?.iter().any(|s| s.untracked && s.path == path);
    if untracked && action == LineAction::Stage {
        write(repo, &["add", "-N", "--", path])?;
    }
    let mut diff = diff_file(repo, path, action == LineAction::Unstage, false)?;
    // A partial patch edits the file in place; a "new file"/"deleted" header would make
    // git apply want the whole file created or removed.
    diff.new_file = false;
    diff.deleted = false;
    let sel: HashSet<(usize, usize)> = lines.iter().copied().collect();
    let mode = if action == LineAction::Stage { patch::Mode::Forward } else { patch::Mode::Reverse };
    let Some(p) = patch::build(&diff, &sel, mode) else { return Ok(()) };
    let args: &[&str] = match action {
        LineAction::Stage => &["apply", "--cached", "--recount", "--whitespace=nowarn", "-"],
        LineAction::Unstage => &["apply", "--cached", "-R", "--recount", "--whitespace=nowarn", "-"],
        LineAction::Discard => &["apply", "-R", "--recount", "--whitespace=nowarn", "-"],
    };
    write_stdin(repo, args, p.as_bytes()).map(|_| ())
}

pub fn commit(repo: &Path, message: &str, amend: bool) -> Result<String, String> {
    let mut args = vec!["commit", "-m", message];
    if amend {
        args.push("--amend");
    }
    write(repo, &args)?;
    Ok(read(repo, &["rev-parse", "HEAD"])?.trim().to_string())
}

pub fn last_message(repo: &Path) -> Result<String, String> {
    read(repo, &["log", "-1", "--format=%B"]).map(|s| s.trim_end().to_string())
}

// ---------- remote ----------

pub fn fetch(repo: &Path) -> Result<String, String> {
    write(repo, &["fetch", "--all", "--prune"])
}

/// mode: "ff" (fast-forward only), "rebase" or "merge"
pub fn pull(repo: &Path, mode: &str) -> Result<String, String> {
    let flag = match mode {
        "rebase" => "--rebase",
        "merge" => "--no-rebase",
        _ => "--ff-only",
    };
    write(repo, &["pull", flag])
}

pub fn push(repo: &Path, force: bool) -> Result<String, String> {
    let has_upstream = read(repo, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]).is_ok();
    let mut args: Vec<String> = vec!["push".into()];
    if force {
        args.push("--force-with-lease".into());
    }
    if !has_upstream {
        let remotes = read(repo, &["remote"])?;
        let remote = remotes.lines().find(|r| *r == "origin").or_else(|| remotes.lines().next()).ok_or("這個 repo 沒有設定 remote")?;
        args.extend(["-u".into(), remote.into(), "HEAD".into()]);
    }
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    write(repo, &a)
}

// ---------- history ----------

pub fn log(repo: &Path, skip: usize, limit: usize, all: bool, query: &str, path: Option<&str>) -> Result<Vec<Commit>, String> {
    if read(repo, &["rev-parse", "--verify", "-q", "HEAD"]).is_err() && !all {
        return Ok(vec![]);
    }
    let skip_s = format!("--skip={skip}");
    let n = format!("-n{limit}");
    let fmt = format!("--format={}", parse::LOG_FORMAT);
    let mut args: Vec<String> = vec!["log".into(), "--decorate=full".into(), "--date-order".into(), skip_s, n, fmt];
    if all {
        args.extend(["--branches".into(), "--remotes".into(), "--tags".into()]);
    }
    args.push("HEAD".into());
    let q = query.trim();
    if !q.is_empty() {
        if q.len() >= 4 && q.chars().all(|c| c.is_ascii_hexdigit()) {
            // A hash prefix: show that commit if it exists.
            if let Ok(full) = read(repo, &["rev-parse", "--verify", "-q", &format!("{q}^{{commit}}")]) {
                let out = read(repo, &["log", "--decorate=full", "-n1", &format!("--format={}", parse::LOG_FORMAT), full.trim()])?;
                return Ok(parse::parse_log(&out));
            }
        }
        args.extend(["-i".into(), "--fixed-strings".into(), format!("--grep={q}")]);
    }
    if let Some(p) = path {
        args.extend(["--follow".into(), "--".into(), p.into()]);
    }
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    let o = run(Some(repo), &a, None, false);
    if !o.ok() {
        // An empty repo has no HEAD yet.
        return if o.stderr.contains("unknown revision") || o.stderr.contains("bad default revision") { Ok(vec![]) } else { Err(o.message()) };
    }
    Ok(parse::parse_log(&o.stdout))
}

#[derive(Debug, Clone, Serialize)]
pub struct CommitDetail {
    pub sha: String,
    pub parents: Vec<String>,
    pub author: String,
    pub email: String,
    pub time: i64,
    pub committer: String,
    pub commit_time: i64,
    pub refs: Vec<parse::RefLabel>,
    pub message: String,
    pub files: Vec<ChangedFile>,
}

pub fn commit_detail(repo: &Path, sha: &str) -> Result<CommitDetail, String> {
    let out = read(repo, &["show", "-s", "--decorate=full", "--format=%H%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%cn%x1f%ct%x1f%D%x1f%B", sha])?;
    let f: Vec<&str> = out.splitn(9, '\x1f').collect();
    if f.len() < 9 {
        return Err("讀不到這個 commit".into());
    }
    let parents: Vec<String> = f[1].split(' ').filter(|s| !s.is_empty()).map(String::from).collect();
    let base = parents.first().cloned().unwrap_or_else(|| EMPTY_TREE.into());
    let files = changed_files(repo, &base, Some(f[0]))?;
    Ok(CommitDetail {
        sha: f[0].into(),
        parents,
        author: f[2].into(),
        email: f[3].into(),
        time: f[4].parse().unwrap_or(0),
        committer: f[5].into(),
        commit_time: f[6].parse().unwrap_or(0),
        refs: parse::parse_decorations(f[7]),
        message: f[8].trim_end().into(),
        files,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct RefsAll {
    #[serde(flatten)]
    pub refs: Refs,
    pub stashes: Vec<Stash>,
    pub worktrees: Vec<Worktree>,
    pub default_branch: Option<String>,
}

pub fn refs(repo: &Path) -> Result<RefsAll, String> {
    std::thread::scope(|sc| {
        let stash = sc.spawn(|| read(repo, &["stash", "list", "--format=%gd%x1f%H%x1f%s"]).unwrap_or_default());
        let wt = sc.spawn(|| read(repo, &["worktree", "list", "--porcelain"]).unwrap_or_default());
        let head = sc.spawn(|| read(repo, &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"]).ok());
        let fmt = format!("--format={}", parse::REF_FORMAT);
        let r = parse::parse_refs(&read(repo, &["for-each-ref", &fmt, "refs/heads", "refs/remotes", "refs/tags"])?);
        let stashes = parse::parse_stashes(&stash.join().unwrap_or_default());
        let worktrees = parse::parse_worktrees(&wt.join().unwrap_or_default());
        let default_branch = default_branch(head.join().ok().flatten(), &r);
        Ok(RefsAll { refs: r, stashes, worktrees, default_branch })
    })
}

/// main/master (or whatever origin/HEAD points at).
pub fn default_branch(origin_head: Option<String>, r: &Refs) -> Option<String> {
    if let Some(s) = origin_head {
        let name = s.trim().trim_start_matches("origin/").to_string();
        if r.branches.iter().any(|b| b.name == name) {
            return Some(name);
        }
        return Some(format!("origin/{name}"));
    }
    ["main", "master", "trunk", "develop"].iter().find(|n| r.branches.iter().any(|b| &b.name == *n)).map(|s| s.to_string())
}

pub fn range_files(repo: &Path, base: &str, head: &str) -> Result<Vec<ChangedFile>, String> {
    changed_files(repo, &format!("{base}...{head}"), None)
}

pub fn range_commits(repo: &Path, base: &str, head: &str) -> Result<Vec<Commit>, String> {
    let fmt = format!("--format={}", parse::LOG_FORMAT);
    Ok(parse::parse_log(&read(repo, &["log", "--decorate=full", &fmt, &format!("{base}..{head}")])?))
}

pub fn reflog(repo: &Path, limit: usize) -> Result<Vec<ReflogEntry>, String> {
    let n = format!("-n{limit}");
    Ok(parse::parse_reflog(&read(repo, &["reflog", &n, "--format=%gd%x1f%H%x1f%ct%x1f%gs"]).unwrap_or_default()))
}

// ---------- branches & history edits ----------

/// Switches to a local branch, or creates a tracking branch for "origin/x".
pub fn switch(repo: &Path, name: &str) -> Result<String, String> {
    let local = read(repo, &["rev-parse", "--verify", "-q", &format!("refs/heads/{name}")]).is_ok();
    if local {
        return write(repo, &["switch", name]);
    }
    let remote = read(repo, &["rev-parse", "--verify", "-q", &format!("refs/remotes/{name}")]).is_ok();
    if remote {
        return write(repo, &["switch", "--track", name]);
    }
    Err(format!("找不到分支 {name}"))
}

pub fn create_branch(repo: &Path, name: &str, from: Option<&str>, checkout: bool) -> Result<String, String> {
    let mut args = if checkout { vec!["switch", "-c", name] } else { vec!["branch", name] };
    if let Some(f) = from.filter(|f| !f.is_empty()) {
        args.push(f);
    }
    write(repo, &args)
}

pub fn delete_branch(repo: &Path, name: &str, force: bool) -> Result<String, String> {
    write(repo, &["branch", if force { "-D" } else { "-d" }, name])
}

/// Adds a worktree for `branch` in `dir`. A branch that exists (here or on a remote) is checked
/// out there; any other name becomes a new branch from HEAD.
pub fn worktree_add(repo: &Path, dir: &str, branch: &str) -> Result<String, String> {
    let local = read(repo, &["rev-parse", "--verify", "-q", &format!("refs/heads/{branch}")]).is_ok();
    let remote = || read(repo, &["for-each-ref", "--format=%(refname)", &format!("refs/remotes/*/{branch}")]).is_ok_and(|o| !o.trim().is_empty());
    if local || remote() { write(repo, &["worktree", "add", dir, branch]) } else { write(repo, &["worktree", "add", "-b", branch, dir]) }
}

pub fn simple(repo: &Path, op: &str, arg: &str) -> Result<String, String> {
    match op {
        "merge" => write(repo, &["merge", "--no-edit", arg]),
        "merge-no-ff" => write(repo, &["merge", "--no-ff", "--no-edit", arg]),
        "rebase" => write(repo, &["rebase", arg]),
        "cherry-pick" => write(repo, &["cherry-pick", arg]),
        "revert" => write(repo, &["revert", "--no-edit", arg]),
        "checkout" => write(repo, &["switch", "--detach", arg]),
        "reset-soft" => write(repo, &["reset", "--soft", arg]),
        "reset-mixed" => write(repo, &["reset", "--mixed", arg]),
        "reset-hard" => write(repo, &["reset", "--hard", arg]),
        "abort" => abort_or_continue(repo, "--abort"),
        "continue" => abort_or_continue(repo, "--continue"),
        "use-ours" => write(repo, &["checkout", "--ours", "--", arg]).and_then(|_| write(repo, &["add", "--", arg])),
        "use-theirs" => write(repo, &["checkout", "--theirs", "--", arg]).and_then(|_| write(repo, &["add", "--", arg])),
        "mark-resolved" => write(repo, &["add", "--", arg]),
        "stash-apply" => write(repo, &["stash", "apply", &format!("stash@{{{arg}}}")]),
        "stash-pop" => write(repo, &["stash", "pop", &format!("stash@{{{arg}}}")]),
        "stash-drop" => write(repo, &["stash", "drop", &format!("stash@{{{arg}}}")]),
        "tag-delete" => write(repo, &["tag", "-d", arg]),
        "push-tag" => write(repo, &["push", "origin", &format!("refs/tags/{arg}")]),
        "worktree-remove" => write(repo, &["worktree", "remove", arg]),
        _ => Err(format!("不支援的操作 {op}")),
    }
}

fn abort_or_continue(repo: &Path, flag: &str) -> Result<String, String> {
    let st = summary(repo)?.state;
    match st {
        Some("merge") if flag == "--continue" => write(repo, &["commit", "--no-edit"]),
        Some(s) => write(repo, &[s, flag]),
        None => Err("目前沒有進行中的 merge / rebase".into()),
    }
}

pub fn tag(repo: &Path, name: &str, sha: &str, message: &str) -> Result<String, String> {
    if message.trim().is_empty() { write(repo, &["tag", name, sha]) } else { write(repo, &["tag", "-a", name, "-m", message, sha]) }
}

pub fn stash_push(repo: &Path, message: &str, include_untracked: bool) -> Result<String, String> {
    let mut args = vec!["stash", "push"];
    if include_untracked {
        args.push("-u");
    }
    if !message.trim().is_empty() {
        args.extend(["-m", message]);
    }
    write(repo, &args)
}

// ---------- blame ----------

#[derive(Debug, Clone, Serialize)]
pub struct BlameLine {
    pub sha: String,
    pub author: String,
    pub time: i64,
    pub summary: String,
    pub line: u32,
    pub text: String,
}

pub fn blame(repo: &Path, path: &str, rev: Option<&str>) -> Result<Vec<BlameLine>, String> {
    let mut args = vec!["blame", "--line-porcelain"];
    if let Some(r) = rev {
        args.push(r);
    }
    args.extend(["--", path]);
    let out = read(repo, &args)?;
    let mut v = Vec::new();
    let mut cur = BlameLine { sha: String::new(), author: String::new(), time: 0, summary: String::new(), line: 0, text: String::new() };
    for l in out.lines() {
        if let Some(t) = l.strip_prefix('\t') {
            cur.text = t.into();
            v.push(std::mem::replace(&mut cur, BlameLine { sha: String::new(), author: String::new(), time: 0, summary: String::new(), line: 0, text: String::new() }));
        } else if let Some(a) = l.strip_prefix("author ") {
            cur.author = a.into();
        } else if let Some(a) = l.strip_prefix("author-time ") {
            cur.time = a.parse().unwrap_or(0);
        } else if let Some(a) = l.strip_prefix("summary ") {
            cur.summary = a.into();
        } else if cur.sha.is_empty() {
            let mut p = l.split(' ');
            if let (Some(sha), Some(_), Some(n)) = (p.next(), p.next(), p.next()) {
                if sha.len() >= 40 {
                    cur.sha = sha.into();
                    cur.line = n.parse().unwrap_or(0);
                }
            }
        }
    }
    Ok(v)
}

// ---------- repos on disk ----------

/// Folders right under `dir` (and `dir` itself) that are git repos.
pub fn scan(dir: &Path) -> Vec<String> {
    let mut v = Vec::new();
    if dir.join(".git").exists() {
        v.push(dir.display().to_string());
    }
    if let Ok(rd) = std::fs::read_dir(dir) {
        let mut subs: Vec<PathBuf> = rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir() && p.join(".git").exists()).collect();
        subs.sort();
        v.extend(subs.into_iter().map(|p| p.display().to_string()));
    }
    v
}

pub fn clone(url: &str, dir: &Path) -> Result<String, String> {
    let parent = dir.parent().ok_or("資料夾路徑不對")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let d = dir.display().to_string();
    let o = run(Some(parent), &["clone", "--progress", url, &d], None, true);
    if o.ok() { Ok(d) } else { Err(o.message()) }
}
