//! End-to-end checks against real throwaway repos. The global gitconfig is redirected to a
//! temp file so these tests never touch the user's own settings.

use gitcore::{Ctx, dispatch};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

fn isolate() -> &'static PathBuf {
    static HOME: OnceLock<PathBuf> = OnceLock::new();
    HOME.get_or_init(|| {
        let d = tempfile::tempdir().unwrap().keep();
        let g = d.join("gitconfig");
        std::fs::write(&g, "[user]\n\tname = Test\n\temail = test@example.com\n[init]\n\tdefaultBranch = main\n").unwrap();
        // SAFETY: set once, before any test spawns git.
        unsafe {
            std::env::set_var("GIT_CONFIG_GLOBAL", &g);
            std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
        }
        d
    })
}

fn git(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git").args(args).current_dir(dir).output().unwrap();
    assert!(o.status.success(), "git {:?}: {}", args, String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).into_owned()
}

/// The tests share one global gitconfig, and one of them rewrites it, so run them one at a time.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static M: std::sync::Mutex<()> = std::sync::Mutex::new(());
    M.lock().unwrap_or_else(|e| e.into_inner())
}

fn repo() -> (tempfile::TempDir, Ctx) {
    isolate();
    let d = tempfile::tempdir().unwrap();
    git(d.path(), &["init", "-q", "-b", "main"]);
    std::fs::write(d.path().join("a.txt"), "one\ntwo\nthree\nfour\nfive\n").unwrap();
    git(d.path(), &["add", "."]);
    git(d.path(), &["commit", "-q", "-m", "first"]);
    let cfg = tempfile::tempdir().unwrap().keep();
    (d, Ctx::new(cfg))
}

fn call(ctx: &Ctx, cmd: &str, a: Value) -> Value {
    dispatch(ctx, cmd, a).unwrap_or_else(|e| panic!("{cmd}: {e}"))
}

fn r(d: &tempfile::TempDir) -> String {
    d.path().display().to_string()
}

#[test]
fn status_stage_commit_and_log() {
    let _s = serial();
    let (d, ctx) = repo();
    std::fs::write(d.path().join("a.txt"), "one\nTWO\nthree\nfour\nfive\n").unwrap();
    std::fs::write(d.path().join("新.txt"), "hi\n").unwrap();
    let st = call(&ctx, "status", json!({"repo": r(&d)}));
    assert_eq!(st.as_array().unwrap().len(), 2);
    let diff = call(&ctx, "diff_file", json!({"repo": r(&d), "path": "a.txt"}));
    assert_eq!(diff["hunks"][0]["lines"].as_array().unwrap().iter().filter(|l| l["t"] != " ").count(), 2);
    let u = call(&ctx, "diff_file", json!({"repo": r(&d), "path": "新.txt", "untracked": true}));
    assert_eq!(u["hunks"][0]["lines"][0]["text"], "hi");
    call(&ctx, "stage", json!({"repo": r(&d), "paths": ["a.txt", "新.txt"]}));
    call(&ctx, "commit", json!({"repo": r(&d), "message": "second"}));
    let log = call(&ctx, "log", json!({"repo": r(&d)}));
    assert_eq!(log[0]["subject"], "second");
    assert_eq!(log[0]["refs"][0]["kind"], "head");
    let det = call(&ctx, "commit_detail", json!({"repo": r(&d), "sha": log[0]["sha"]}));
    assert_eq!(det["files"].as_array().unwrap().len(), 2);
    let first = call(&ctx, "commit_detail", json!({"repo": r(&d), "sha": log[1]["sha"]}));
    assert_eq!(first["files"][0]["path"], "a.txt");
    let fd = call(&ctx, "commit_file_diff", json!({"repo": r(&d), "sha": log[1]["sha"], "path": "a.txt"}));
    assert_eq!(fd["hunks"][0]["lines"].as_array().unwrap().len(), 5);
    let s = call(&ctx, "summary", json!({"repo": r(&d)}));
    assert_eq!(s["branch"], "main");
    assert_eq!(s["changes"], 0);
}

#[test]
fn stage_and_unstage_single_lines() {
    let _s = serial();
    let (d, ctx) = repo();
    std::fs::write(d.path().join("a.txt"), "one\nTWO\nthree\nfour\nFIVE\nsix\n").unwrap();
    let diff = call(&ctx, "diff_file", json!({"repo": r(&d), "path": "a.txt"}));
    // Pick only the lines that turn "two" into "TWO".
    let lines: Vec<(usize, usize)> = diff["hunks"][0]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .filter(|(_, l)| l["text"] == "two" || l["text"] == "TWO")
        .map(|(i, _)| (0, i))
        .collect();
    assert_eq!(lines.len(), 2);
    call(&ctx, "apply_lines", json!({"repo": r(&d), "path": "a.txt", "action": "stage", "lines": lines}));
    assert_eq!(git(d.path(), &["show", ":a.txt"]), "one\nTWO\nthree\nfour\nfive\n");
    // Unstage it again from the staged diff.
    let staged = call(&ctx, "diff_file", json!({"repo": r(&d), "path": "a.txt", "staged": true}));
    let all: Vec<(usize, usize)> = (0..staged["hunks"][0]["lines"].as_array().unwrap().len()).map(|i| (0, i)).collect();
    call(&ctx, "apply_lines", json!({"repo": r(&d), "path": "a.txt", "action": "unstage", "lines": all}));
    assert_eq!(git(d.path(), &["show", ":a.txt"]), "one\ntwo\nthree\nfour\nfive\n");
    // Discard only the "six" line from the worktree.
    let diff = call(&ctx, "diff_file", json!({"repo": r(&d), "path": "a.txt"}));
    let six: Vec<(usize, usize)> = diff["hunks"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .flat_map(|(hi, h)| h["lines"].as_array().unwrap().iter().enumerate().filter(|(_, l)| l["text"] == "six").map(move |(li, _)| (hi, li)).collect::<Vec<_>>())
        .collect();
    call(&ctx, "apply_lines", json!({"repo": r(&d), "path": "a.txt", "action": "discard", "lines": six}));
    assert_eq!(std::fs::read_to_string(d.path().join("a.txt")).unwrap(), "one\nTWO\nthree\nfour\nFIVE\n");
}

#[test]
fn stage_lines_of_untracked_file() {
    let _s = serial();
    let (d, ctx) = repo();
    std::fs::write(d.path().join("n.txt"), "x\ny\n").unwrap();
    call(&ctx, "apply_lines", json!({"repo": r(&d), "path": "n.txt", "action": "stage", "lines": [[0, 0]]}));
    assert_eq!(git(d.path(), &["show", ":n.txt"]), "x\n");
}

#[test]
fn branches_range_stash_reflog_and_ops() {
    let _s = serial();
    let (d, ctx) = repo();
    call(&ctx, "create_branch", json!({"repo": r(&d), "name": "feat/x", "checkout": true}));
    std::fs::write(d.path().join("b.txt"), "b\n").unwrap();
    git(d.path(), &["add", "."]);
    git(d.path(), &["commit", "-q", "-m", "add b"]);
    let refs = call(&ctx, "refs", json!({"repo": r(&d)}));
    assert_eq!(refs["default_branch"], "main");
    assert!(refs["branches"].as_array().unwrap().iter().any(|b| b["name"] == "feat/x" && b["current"] == true));
    let range = call(&ctx, "range_files", json!({"repo": r(&d), "base": "main", "head": "feat/x"}));
    assert_eq!(range["files"][0]["path"], "b.txt");
    assert_eq!(range["commits"].as_array().unwrap().len(), 1);
    let rd = call(&ctx, "range_file_diff", json!({"repo": r(&d), "base": "main", "head": "feat/x", "path": "b.txt"}));
    assert!(rd["new_file"].as_bool().unwrap());

    std::fs::write(d.path().join("a.txt"), "changed\n").unwrap();
    call(&ctx, "stash_push", json!({"repo": r(&d), "message": "wip"}));
    let refs = call(&ctx, "refs", json!({"repo": r(&d)}));
    assert_eq!(refs["stashes"][0]["index"], 0);
    call(&ctx, "op", json!({"repo": r(&d), "op": "stash-pop", "arg": "0"}));
    assert_eq!(std::fs::read_to_string(d.path().join("a.txt")).unwrap(), "changed\n");
    call(&ctx, "discard", json!({"repo": r(&d), "paths": ["a.txt"]}));

    call(&ctx, "switch", json!({"repo": r(&d), "name": "main"}));
    call(&ctx, "op", json!({"repo": r(&d), "op": "merge-no-ff", "arg": "feat/x"}));
    let log = call(&ctx, "log", json!({"repo": r(&d)}));
    assert_eq!(log[0]["parents"].as_array().unwrap().len(), 2);
    let merge = call(&ctx, "commit_detail", json!({"repo": r(&d), "sha": log[0]["sha"]}));
    assert_eq!(merge["files"][0]["path"], "b.txt");
    let rl = call(&ctx, "reflog", json!({"repo": r(&d)}));
    assert!(rl.as_array().unwrap().len() >= 3);
    let hits = call(&ctx, "log", json!({"repo": r(&d), "query": "ADD B"}));
    assert_eq!(hits.as_array().unwrap().len(), 1);
    call(&ctx, "op", json!({"repo": r(&d), "op": "reset-hard", "arg": "HEAD~1"}));
    assert!(!d.path().join("b.txt").exists());
    let blame = call(&ctx, "blame", json!({"repo": r(&d), "path": "a.txt"}));
    assert_eq!(blame.as_array().unwrap().len(), 5);
}

#[test]
fn conflict_state_and_abort() {
    let _s = serial();
    let (d, ctx) = repo();
    git(d.path(), &["switch", "-q", "-c", "other"]);
    std::fs::write(d.path().join("a.txt"), "other\n").unwrap();
    git(d.path(), &["commit", "-qam", "other"]);
    git(d.path(), &["switch", "-q", "main"]);
    std::fs::write(d.path().join("a.txt"), "mine\n").unwrap();
    git(d.path(), &["commit", "-qam", "mine"]);
    assert!(dispatch(&ctx, "op", json!({"repo": r(&d), "op": "merge", "arg": "other"})).is_err());
    let s = call(&ctx, "summary", json!({"repo": r(&d)}));
    assert_eq!(s["state"], "merge");
    assert_eq!(s["conflicts"], 1);
    call(&ctx, "op", json!({"repo": r(&d), "op": "use-theirs", "arg": "a.txt"}));
    call(&ctx, "op", json!({"repo": r(&d), "op": "continue"}));
    let s = call(&ctx, "summary", json!({"repo": r(&d)}));
    assert!(s["state"].is_null());
    assert_eq!(std::fs::read_to_string(d.path().join("a.txt")).unwrap(), "other\n");
}

#[test]
fn identities_write_include_if_and_pin() {
    let _s = serial();
    let (d, ctx) = repo();
    let g = isolate().join("gitconfig");
    let ids = json!([
        {"id": "p", "name": "個人", "user": "Sam", "email": "sam@me.com", "key": "C:\\k\\id_p", "rules": [{"t": "remote", "v": "github.com/sam/*"}]},
        {"id": "e", "name": "EMU", "user": "Sam", "email": "sam@corp.com", "key": "C:\\k\\id_e", "rules": [{"t": "dir", "v": "D:\\work"}]}
    ]);
    call(&ctx, "identities_apply", json!({"identities": ids, "default": "p"}));
    let text = std::fs::read_to_string(&g).unwrap();
    assert!(text.contains("hasconfig:remote.*.url:git@github.com:sam/**"));
    assert!(text.contains("gitdir/i:D:/work/"));
    // A repo whose remote matches the "p" rule picks up p's email through the include.
    git(d.path(), &["remote", "add", "origin", "git@github.com:sam/x.git"]);
    assert_eq!(git(d.path(), &["config", "user.email"]).trim(), "sam@me.com");
    // Re-applying replaces our lines instead of piling them up.
    call(&ctx, "identities_apply", json!({"identities": ids, "default": "p"}));
    let again = std::fs::read_to_string(&g).unwrap();
    assert_eq!(again.matches("git@github.com:sam/**").count(), 1);
    // Pinning writes the repo's own config, which wins over the include.
    call(&ctx, "identity_pin", json!({"repo": r(&d), "identity": ids[1]}));
    assert_eq!(git(d.path(), &["config", "user.email"]).trim(), "sam@corp.com");
    assert!(git(d.path(), &["config", "--local", "core.sshCommand"]).contains("C:/k/id_e"));
    call(&ctx, "identity_pin", json!({"repo": r(&d), "identity": null}));
    assert_eq!(git(d.path(), &["config", "user.email"]).trim(), "sam@me.com");
    // Removing every identity leaves the global config as it was.
    call(&ctx, "identities_apply", json!({"identities": [], "default": null}));
    let after = std::fs::read_to_string(&g).unwrap();
    assert!(!after.contains("includeIf"));
    assert!(!after.contains("include"), "{after}");
}
