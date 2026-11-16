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
        std::fs::write(&g, "[user]\n\tname = Test\n\temail = test@example.com\n[init]\n\tdefaultBranch = main\n[protocol \"file\"]\n\tallow = always\n").unwrap();
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

/// The fingerprint of the diff the user would be looking at.
fn fp(d: &tempfile::TempDir, path: &str, staged: bool, untracked: bool) -> String {
    gitcore::ops::diff_file(d.path(), path, staged, untracked).unwrap().fingerprint()
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
    call(&ctx, "apply_lines", json!({"repo": r(&d), "path": "a.txt", "action": "stage", "lines": lines, "diff_hash": fp(&d, "a.txt", false, false)}));
    assert_eq!(git(d.path(), &["show", ":a.txt"]), "one\nTWO\nthree\nfour\nfive\n");
    // Unstage it again from the staged diff.
    let staged = call(&ctx, "diff_file", json!({"repo": r(&d), "path": "a.txt", "staged": true}));
    let all: Vec<(usize, usize)> = (0..staged["hunks"][0]["lines"].as_array().unwrap().len()).map(|i| (0, i)).collect();
    call(&ctx, "apply_lines", json!({"repo": r(&d), "path": "a.txt", "action": "unstage", "lines": all, "diff_hash": fp(&d, "a.txt", true, false)}));
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
    call(&ctx, "apply_lines", json!({"repo": r(&d), "path": "a.txt", "action": "discard", "lines": six, "diff_hash": fp(&d, "a.txt", false, false)}));
    assert_eq!(std::fs::read_to_string(d.path().join("a.txt")).unwrap(), "one\nTWO\nthree\nfour\nFIVE\n");
}

#[test]
fn stage_lines_of_untracked_file() {
    let _s = serial();
    let (d, ctx) = repo();
    std::fs::write(d.path().join("n.txt"), "x\ny\n").unwrap();
    call(&ctx, "apply_lines", json!({"repo": r(&d), "path": "n.txt", "action": "stage", "lines": [[0, 0]], "diff_hash": fp(&d, "n.txt", false, true)}));
    assert_eq!(git(d.path(), &["show", ":n.txt"]), "x\n");
}

#[test]
fn branches_stash_reflog_and_ops() {
    let _s = serial();
    let (d, ctx) = repo();
    call(&ctx, "create_branch", json!({"repo": r(&d), "name": "feat/x", "checkout": true}));
    std::fs::write(d.path().join("b.txt"), "b\n").unwrap();
    git(d.path(), &["add", "."]);
    git(d.path(), &["commit", "-q", "-m", "add b"]);
    let refs = call(&ctx, "refs", json!({"repo": r(&d)}));
    assert_eq!(refs["default_branch"], "main");
    assert!(refs["branches"].as_array().unwrap().iter().any(|b| b["name"] == "feat/x" && b["current"] == true));

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

/// Puts the shared global gitconfig back when a test ends, also when it fails, so a fixture
/// never leaks into the other tests.
struct RestoreGlobal(PathBuf, String);

impl Drop for RestoreGlobal {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.0, &self.1);
    }
}

fn keep_global() -> RestoreGlobal {
    let g = isolate().join("gitconfig");
    let before = std::fs::read_to_string(&g).unwrap();
    RestoreGlobal(g, before)
}

/// Every value of `key` in the shared global gitconfig.
fn global_all(key: &str) -> Vec<String> {
    let o = Command::new("git").args(["config", "--global", "--get-all", key]).output().unwrap();
    String::from_utf8_lossy(&o.stdout).lines().map(String::from).collect()
}

/// How many include / includeIf lines in the global gitconfig point into the app's identities folder.
fn lines_into_identities(cfg_dir: &Path) -> usize {
    let o = Command::new("git").args(["config", "--global", "--get-regexp", r"^include(if\..*)?\.path$"]).output().unwrap();
    let dir = format!("{}/", cfg_dir.join("identities").display().to_string().replace('\\', "/").to_lowercase());
    String::from_utf8_lossy(&o.stdout).lines().filter(|l| l.split_once(' ').is_some_and(|(_, v)| v.replace('\\', "/").to_lowercase().starts_with(&dir))).count()
}

/// The user's own includes, which must survive every clean-up.
const MINE: &str = "~/.gitconfig-git-tabs-test-mine";
const MINE_IF: &str = "includeIf.gitdir/i:D:/git-tabs-test-mine/.path";

/// What versions with a default identity could leave behind: three copies of the default
/// identity's include (one spelt with backslashes in upper case), one whose file is gone, and the
/// user's own includes in between, among them a neighbour folder whose name starts the same.
/// Returns (the default identity's include, the neighbour).
fn leave_old_includes(cfg_dir: &Path) -> (String, String) {
    let ids = cfg_dir.join("identities");
    std::fs::create_dir_all(&ids).unwrap();
    std::fs::write(ids.join("idpersonal.gitconfig"), "[user]\n\temail = personal@example.com\n").unwrap();
    let base = cfg_dir.display().to_string().replace('\\', "/");
    let ours = format!("{base}/identities/idpersonal.gitconfig");
    let shouty = ours.to_uppercase().replace('/', "\\");
    let gone = format!("{base}/identities/idgone.gitconfig");
    let neighbour = format!("{base}/identities-old/x.gitconfig");
    for v in [ours.as_str(), MINE, ours.as_str(), shouty.as_str(), gone.as_str(), neighbour.as_str()] {
        git(cfg_dir, &["config", "--global", "--add", "include.path", v]);
    }
    git(cfg_dir, &["config", "--global", MINE_IF, "~/git-tabs-test-mine.gitconfig"]);
    (ours, neighbour)
}

#[test]
fn tidy_takes_out_the_old_default_identity_includes_only() {
    use gitcore::config::{AppConfig, Managed};
    let _s = serial();
    let (d, ctx) = repo();
    let _g = keep_global();
    let (ours, neighbour) = leave_old_includes(&ctx.config_dir);
    // `managed` as an older version could leave it: the default include and one rule.
    let rule = Managed { key: "includeIf.gitdir/i:D:/x/.path".into(), value: ours.clone() };
    let cfg = AppConfig { managed: vec![Managed { key: "include.path".into(), value: ours.clone() }, rule.clone()], ..Default::default() };
    gitcore::config::save(&ctx.config_dir, &cfg).unwrap();
    // The bug: the include wins in every repo that has no email of its own.
    assert_eq!(git(d.path(), &["config", "user.email"]).trim(), "personal@example.com");

    let removed = call(&ctx, "identities_tidy", json!({}));
    assert_eq!(removed.as_array().unwrap().len(), 4, "three copies of the default include and the one whose file is gone: {removed}");
    assert_eq!(lines_into_identities(&ctx.config_dir), 0);
    assert_eq!(global_all("include.path"), vec![MINE.to_string(), neighbour], "the user's own includes stay, in order");
    assert_eq!(global_all(MINE_IF), vec!["~/git-tabs-test-mine.gitconfig"]);
    assert_eq!(git(d.path(), &["config", "user.email"]).trim(), "test@example.com");
    assert!(gitcore::config::load(&ctx.config_dir).managed == vec![rule], "only the include.path entry leaves managed");
    // Nothing is left for the next start, so it has nothing to report.
    assert_eq!(call(&ctx, "identities_tidy", json!({})).as_array().unwrap().len(), 0);
}

#[test]
fn apply_cleans_up_lines_it_lost_track_of_and_writes_each_once() {
    let _s = serial();
    let (d, ctx) = repo();
    let _g = keep_global();
    let (_, neighbour) = leave_old_includes(&ctx.config_dir);
    git(d.path(), &["remote", "add", "origin", "git@github.com:sam/x.git"]);
    // The same rule twice, which the settings screen allows.
    let ids = json!([{"id": "p", "name": "個人", "user": "Sam", "email": "sam@me.com", "key": "C:\\k\\id_p",
        "rules": [{"t": "remote", "v": "github.com/sam/*"}, {"t": "remote", "v": "github.com/sam/*"}, {"t": "dir", "v": "D:\\work"}]}]);
    for round in 0..3 {
        call(&ctx, "identities_apply", json!({"identities": ids}));
        assert_eq!(global_all("includeIf.hasconfig:remote.*.url:git@github.com:sam/**.path").len(), 1, "round {round}");
        assert_eq!(global_all("includeIf.gitdir/i:D:/work/.path").len(), 1, "round {round}");
        assert_eq!(lines_into_identities(&ctx.config_dir), 4, "round {round}: three remote conditions and one folder, no include.path");
        assert_eq!(global_all("include.path"), vec![MINE.to_string(), neighbour.clone()], "round {round}");
        assert_eq!(global_all(MINE_IF).len(), 1, "round {round}");
        assert_eq!(git(d.path(), &["config", "user.email"]).trim(), "sam@me.com", "round {round}");
        let mut cfg = gitcore::config::load(&ctx.config_dir);
        assert_eq!(cfg.managed.len(), 4, "round {round}: managed has no copies either");
        // What the settings screen used to do: save its stale copy over `managed`.
        cfg.managed.clear();
        gitcore::config::save(&ctx.config_dir, &cfg).unwrap();
    }
    call(&ctx, "identities_apply", json!({"identities": []}));
    assert_eq!(lines_into_identities(&ctx.config_dir), 0);
    assert_eq!(global_all("include.path"), vec![MINE.to_string(), neighbour]);
    assert_eq!(global_all(MINE_IF).len(), 1);
}

#[test]
fn detects_keys_ssh_hosts_and_includes() {
    let _s = serial();
    isolate();
    let home = tempfile::tempdir().unwrap();
    let ssh = home.path().join(".ssh");
    std::fs::create_dir_all(&ssh).unwrap();
    std::fs::write(ssh.join("id_ed25519"), "x").unwrap();
    std::fs::write(ssh.join("id_ed25519.pub"), "ssh-ed25519 AAAA me@home.com\n").unwrap();
    std::fs::write(ssh.join("id_work"), "x").unwrap();
    std::fs::write(ssh.join("id_work.pub"), "ssh-ed25519 BBBB\n").unwrap();
    std::fs::write(ssh.join("known_hosts"), "").unwrap();
    std::fs::write(ssh.join("config"), "Host github-work\n  HostName github.com\n  IdentityFile ~/.ssh/id_work\n\nHost *\n  AddKeysToAgent yes\n").unwrap();
    let inc = home.path().join("work.gitconfig");
    std::fs::write(&inc, "[user]\n\temail = me@corp.com\n[core]\n\tsshCommand = ssh -i \"C:/k/id_work\" -o IdentitiesOnly=yes\n").unwrap();
    let g = isolate().join("gitconfig");
    let before = std::fs::read_to_string(&g).unwrap();
    git(home.path(), &["config", "--global", "includeIf.gitdir/i:D:/work/.path", &inc.display().to_string()]);
    let f = gitcore::identity::detect(home.path(), &[]);
    std::fs::write(&g, before).unwrap();
    assert_eq!(f.email, "test@example.com");
    assert_eq!(f.keys.len(), 2);
    assert_eq!(f.keys[0].comment, "me@home.com");
    let work = f.keys.iter().find(|k| k.path.ends_with("id_work")).unwrap();
    assert_eq!(work.hosts, vec![("github-work".to_string(), "github.com".to_string())]);
    assert_eq!(f.includes.len(), 1);
    assert_eq!(f.includes[0].condition, "gitdir/i:D:/work/");
    assert_eq!(f.includes[0].email, "me@corp.com");
    assert_eq!(f.includes[0].key, "C:/k/id_work");
}

#[test]
fn worktree_for_an_existing_branch_and_for_a_new_one() {
    let _s = serial();
    let (d, _ctx) = repo();
    git(d.path(), &["branch", "feat/old"]);
    let out = tempfile::tempdir().unwrap();
    let (a, b) = (out.path().join("old"), out.path().join("new"));
    gitcore::ops::worktree_add(d.path(), &a.display().to_string(), "feat/old").unwrap();
    gitcore::ops::worktree_add(d.path(), &b.display().to_string(), "feat/new").unwrap();
    assert_eq!(git(&a, &["branch", "--show-current"]).trim(), "feat/old");
    assert_eq!(git(&b, &["branch", "--show-current"]).trim(), "feat/new", "an unknown name is created from HEAD");
    let all = gitcore::ops::refs(d.path()).unwrap();
    assert_eq!(all.worktrees.len(), 3);
    // the branch of the main worktree is taken: git refuses, and the error comes back
    assert!(gitcore::ops::worktree_add(d.path(), &out.path().join("dup").display().to_string(), "main").is_err());
}

#[test]
fn rename_a_branch_and_manage_remotes() {
    let _s = serial();
    let (d, _ctx) = repo();
    let bare = tempfile::tempdir().unwrap();
    git(bare.path(), &["init", "-q", "--bare"]);
    let url = bare.path().display().to_string();

    git(d.path(), &["branch", "feat/old"]);
    gitcore::ops::rename_branch(d.path(), "feat/old", "feat/new").unwrap();
    let names = |d: &Path| gitcore::ops::refs(d).unwrap().refs.branches.iter().map(|b| b.name.clone()).collect::<Vec<_>>();
    assert!(names(d.path()).contains(&"feat/new".to_string()) && !names(d.path()).contains(&"feat/old".to_string()));
    assert!(gitcore::ops::rename_branch(d.path(), "feat/new", "main").is_err(), "the name is taken");

    gitcore::ops::remote_add(d.path(), "origin", "https://example.invalid/x.git").unwrap();
    gitcore::ops::remote_set_url(d.path(), "origin", &url).unwrap();
    let all = gitcore::ops::refs(d.path()).unwrap();
    assert_eq!(all.remote_list.len(), 1, "listed although nothing was fetched from it yet");
    assert_eq!((all.remote_list[0].name.as_str(), all.remote_list[0].url.as_str()), ("origin", url.as_str()));

    git(d.path(), &["push", "-q", "origin", "feat/new"]);
    assert!(gitcore::ops::refs(d.path()).unwrap().refs.remotes.iter().any(|b| b.name == "origin/feat/new"));
    gitcore::ops::delete_remote_branch(d.path(), "origin/feat/new").unwrap();
    assert!(git(d.path(), &["ls-remote", "--heads", "origin"]).trim().is_empty(), "gone on the remote");
    assert!(!gitcore::ops::refs(d.path()).unwrap().refs.remotes.iter().any(|b| b.name == "origin/feat/new"));
    assert!(names(d.path()).contains(&"feat/new".to_string()), "the local branch stays");

    gitcore::ops::remote_remove(d.path(), "origin").unwrap();
    assert!(gitcore::ops::refs(d.path()).unwrap().remote_list.is_empty());
}

#[test]
fn submodules_are_listed_and_can_be_fetched() {
    let _s = serial();
    let (lib, _c1) = repo();
    let (d, _c2) = repo();
    // (the shared test gitconfig allows the file protocol, which git refuses for submodules by default)
    git(d.path(), &["submodule", "add", "-q", &lib.path().display().to_string(), "libs/the lib"]);
    git(d.path(), &["commit", "-q", "-m", "add the lib"]);
    let subs = gitcore::ops::refs(d.path()).unwrap().submodules;
    assert_eq!(subs.len(), 1);
    assert_eq!((subs[0].path.as_str(), subs[0].state), ("libs/the lib", "ok"), "a path with a space, without git's description");
    assert_eq!(subs[0].sha.len(), 40);

    // a plain clone has the submodule's folder but nothing in it
    let fresh = tempfile::tempdir().unwrap();
    git(fresh.path(), &["clone", "-q", &d.path().display().to_string(), "copy"]);
    let copy = fresh.path().join("copy");
    assert_eq!(gitcore::ops::refs(&copy).unwrap().submodules[0].state, "uninit");
    gitcore::ops::submodule_update(&copy, "libs/the lib").unwrap();
    assert_eq!(gitcore::ops::refs(&copy).unwrap().submodules[0].state, "ok");
    assert!(copy.join("libs/the lib/a.txt").is_file());

    assert!(gitcore::ops::refs(lib.path()).unwrap().submodules.is_empty(), "a repo without .gitmodules is not even asked");
}

#[test]
fn files_of_a_commit_and_what_they_held() {
    use gitcore::ops::{FileText, file_at, tree};
    let _s = serial();
    let (d, _ctx) = repo();
    std::fs::create_dir_all(d.path().join("src/deep")).unwrap();
    std::fs::write(d.path().join("src/deep/新 檔.txt"), "第一行\nsecond\n").unwrap();
    std::fs::write(d.path().join("src/blob.bin"), [1u8, 0, 2, 0]).unwrap();
    git(d.path(), &["add", "."]);
    git(d.path(), &["commit", "-q", "-m", "more files"]);
    std::fs::write(d.path().join("a.txt"), "changed later\n").unwrap();
    git(d.path(), &["commit", "-q", "-am", "later"]);

    // the tree of the older commit, also after the files moved on
    assert_eq!(tree(d.path(), "HEAD~1").unwrap(), ["a.txt", "src/blob.bin", "src/deep/新 檔.txt"]);
    assert_eq!(tree(d.path(), "HEAD~2").unwrap(), ["a.txt"]);
    assert_eq!(file_at(d.path(), "HEAD~1", "src/deep/新 檔.txt").unwrap(), FileText::Text("第一行\nsecond\n".into()));
    assert_eq!(file_at(d.path(), "HEAD~1", "a.txt").unwrap(), FileText::Text("one\ntwo\nthree\nfour\nfive\n".into()), "as it was then, not as it is now");
    assert_eq!(file_at(d.path(), "HEAD", "src/blob.bin").unwrap(), FileText::Binary);
    assert!(file_at(d.path(), "HEAD", "no/such/file").is_err());
}

#[test]
fn lines_picked_from_an_older_diff_are_refused() {
    let _s = serial();
    let (d, ctx) = repo();
    std::fs::write(d.path().join("a.txt"), "ONE\ntwo\nthree\nfour\nFIVE\n").unwrap();
    let shown = fp(&d, "a.txt", false, false);
    // Some of it gets staged; the picture the user still has is of the old diff.
    call(&ctx, "apply_lines", json!({"repo": r(&d), "path": "a.txt", "action": "stage", "lines": [[0, 0], [0, 1]], "diff_hash": shown}));
    let err = dispatch(&ctx, "apply_lines", json!({"repo": r(&d), "path": "a.txt", "action": "discard", "lines": [[0, 1], [0, 2]], "diff_hash": shown})).unwrap_err();
    assert!(err.contains("重新整理"), "{err}");
    assert_eq!(std::fs::read_to_string(d.path().join("a.txt")).unwrap(), "ONE\ntwo\nthree\nfour\nFIVE\n", "nothing was thrown away");
    // No fingerprint at all is no fingerprint.
    assert!(dispatch(&ctx, "apply_lines", json!({"repo": r(&d), "path": "a.txt", "action": "discard", "lines": [[0, 0]]})).is_err());
}

#[test]
fn discarding_lines_of_an_untracked_file_says_no() {
    let _s = serial();
    let (d, ctx) = repo();
    std::fs::write(d.path().join("n.txt"), "x\ny\n").unwrap();
    let err = dispatch(&ctx, "apply_lines", json!({"repo": r(&d), "path": "n.txt", "action": "discard", "lines": [[0, 0]], "diff_hash": fp(&d, "n.txt", false, true)})).unwrap_err();
    assert!(err.contains("未追蹤"), "{err}");
    assert!(d.path().join("n.txt").exists());
}

#[test]
fn lines_of_a_crlf_file_can_be_staged_and_discarded() {
    let _s = serial();
    let (d, ctx) = repo();
    git(d.path(), &["config", "core.autocrlf", "false"]);
    std::fs::write(d.path().join("w.txt"), "a\r\nb\r\nc\r\n").unwrap();
    git(d.path(), &["add", "w.txt"]);
    git(d.path(), &["commit", "-qm", "crlf"]);
    std::fs::write(d.path().join("w.txt"), "a\r\nB\r\nc\r\nd\r\n").unwrap();
    let diff = call(&ctx, "diff_file", json!({"repo": r(&d), "path": "w.txt"}));
    let pick = |texts: &[&str]| -> Vec<(usize, usize)> {
        diff["hunks"][0]["lines"].as_array().unwrap().iter().enumerate().filter(|(_, l)| l["t"] != " " && texts.contains(&l["text"].as_str().unwrap())).map(|(i, _)| (0, i)).collect()
    };
    let b = pick(&["b", "B"]);
    assert_eq!(b.len(), 2);
    call(&ctx, "apply_lines", json!({"repo": r(&d), "path": "w.txt", "action": "stage", "lines": b, "diff_hash": fp(&d, "w.txt", false, false)}));
    assert_eq!(git(d.path(), &["show", ":w.txt"]), "a\r\nB\r\nc\r\n");
    // And "d" thrown away from the worktree.
    let diff = call(&ctx, "diff_file", json!({"repo": r(&d), "path": "w.txt"}));
    let dd: Vec<(usize, usize)> = diff["hunks"][0]["lines"].as_array().unwrap().iter().enumerate().filter(|(_, l)| l["text"] == "d").map(|(i, _)| (0, i)).collect();
    call(&ctx, "apply_lines", json!({"repo": r(&d), "path": "w.txt", "action": "discard", "lines": dd, "diff_hash": fp(&d, "w.txt", false, false)}));
    assert_eq!(std::fs::read(d.path().join("w.txt")).unwrap(), b"a\r\nB\r\nc\r\n");
}

/// While rebasing, git's "ours" is the upstream and "theirs" is the user's own commit.
#[test]
fn mine_and_theirs_swap_while_rebasing() {
    let _s = serial();
    let (d, ctx) = repo();
    git(d.path(), &["switch", "-q", "-c", "feature"]);
    std::fs::write(d.path().join("a.txt"), "feature\n").unwrap();
    git(d.path(), &["commit", "-qam", "feature"]);
    git(d.path(), &["switch", "-q", "main"]);
    std::fs::write(d.path().join("a.txt"), "upstream\n").unwrap();
    git(d.path(), &["commit", "-qam", "upstream"]);
    let conflict = |op: &str| {
        git(d.path(), &["switch", "-q", "feature"]);
        let o = Command::new("git").args(["rebase", "main"]).current_dir(d.path()).output().unwrap();
        assert!(!o.status.success());
        let view = call(&ctx, "diff_file", json!({"repo": r(&d), "path": "a.txt", "conflicted": true}));
        assert!(view["hunks"][0]["header"].as_str().unwrap().contains("rebase"), "{view}");
        call(&ctx, "op", json!({"repo": r(&d), "op": op, "arg": "a.txt"}));
        let got = std::fs::read_to_string(d.path().join("a.txt")).unwrap();
        git(d.path(), &["rebase", "--abort"]);
        got
    };
    assert_eq!(conflict("use-ours"), "feature\n", "\"mine\" is the commit being replayed");
    assert_eq!(conflict("use-theirs"), "upstream\n");
    // A plain merge keeps the usual meaning.
    git(d.path(), &["switch", "-q", "feature"]);
    assert!(dispatch(&ctx, "op", json!({"repo": r(&d), "op": "merge", "arg": "main"})).is_err());
    call(&ctx, "op", json!({"repo": r(&d), "op": "use-ours", "arg": "a.txt"}));
    assert_eq!(std::fs::read_to_string(d.path().join("a.txt")).unwrap(), "feature\n");
}

/// A colleague's commit that the background fetch already brought in must still stop a force push.
#[test]
fn force_push_refuses_a_commit_it_never_took_in() {
    let _s = serial();
    let (d, ctx) = repo();
    let remote = tempfile::tempdir().unwrap();
    git(remote.path(), &["init", "-q", "--bare", "-b", "main"]);
    git(d.path(), &["remote", "add", "origin", &remote.path().display().to_string()]);
    git(d.path(), &["push", "-q", "-u", "origin", "main"]);
    let other = tempfile::tempdir().unwrap();
    git(other.path(), &["clone", "-q", &remote.path().display().to_string(), "."]);
    std::fs::write(other.path().join("c.txt"), "colleague\n").unwrap();
    git(other.path(), &["add", "."]);
    git(other.path(), &["commit", "-qm", "colleague"]);
    git(other.path(), &["push", "-q"]);
    // What the app's 10-minute fetch does.
    call(&ctx, "fetch", json!({"repo": r(&d)}));
    git(d.path(), &["commit", "-q", "--amend", "-m", "rewritten"]);
    assert!(dispatch(&ctx, "push", json!({"repo": r(&d), "force": true})).is_err(), "the colleague's commit would be lost");
    assert!(git(remote.path(), &["log", "--format=%s", "main"]).contains("colleague"));
    // Taken in first, the same force push goes through.
    git(d.path(), &["pull", "-q", "--rebase"]);
    call(&ctx, "push", json!({"repo": r(&d), "force": true}));
}
