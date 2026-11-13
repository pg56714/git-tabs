//! Rewriting older commits end to end: a real `git rebase -i` that calls the built program back
//! as its editor. The global gitconfig is a temp file, so the user's own settings are not touched.

use std::path::Path;
use std::process::Command;

use gitcore::rewrite::{Rewrite, rewrite};

fn git(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git").args(args).current_dir(dir).output().unwrap();
    assert!(o.status.success(), "git {:?}: {}", args, String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn commit(dir: &Path, file: &str, msg: &str) -> String {
    std::fs::write(dir.join(file), format!("{msg}\n")).unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", msg]);
    git(dir, &["rev-parse", "HEAD"]).trim().to_string()
}

fn subjects(dir: &Path) -> Vec<String> {
    git(dir, &["log", "--format=%s"]).lines().map(String::from).collect()
}

#[test]
fn reword_squash_and_drop_older_commits() {
    let exe = Path::new(env!("CARGO_BIN_EXE_git-tabs"));
    let home = tempfile::tempdir().unwrap();
    let global = home.path().join("gitconfig");
    std::fs::write(&global, "[user]\n\tname = Test\n\temail = test@example.com\n[init]\n\tdefaultBranch = main\n").unwrap();
    // SAFETY: the only test in this binary, set before it starts any git.
    unsafe {
        std::env::set_var("GIT_CONFIG_GLOBAL", &global);
        std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
    }
    let d = tempfile::tempdir().unwrap();
    let d = d.path();
    git(d, &["init", "-q", "-b", "main"]);
    commit(d, "a.txt", "one");
    let two = commit(d, "b.txt", "two");
    commit(d, "c.txt", "three");
    commit(d, "d.txt", "four");
    // Work that is not committed is there again afterwards.
    std::fs::write(d.join("a.txt"), "edited, not committed\n").unwrap();

    // A new message for a commit in the middle; the commits above it are replayed.
    rewrite(d, exe, &two, &Rewrite::Reword("two, said better\n\nand why".into())).unwrap();
    assert_eq!(subjects(d), ["four", "three", "two, said better", "one"]);
    assert_eq!(git(d, &["log", "-1", "--format=%b", "HEAD~2"]).trim(), "and why");
    assert_eq!(std::fs::read_to_string(d.join("a.txt")).unwrap(), "edited, not committed\n");

    // "three" folded into the commit before it: one commit with both files and both messages.
    let three = git(d, &["rev-parse", "HEAD~1"]).trim().to_string();
    rewrite(d, exe, &three, &Rewrite::Squash).unwrap();
    assert_eq!(subjects(d), ["four", "two, said better", "one"]);
    let folded = git(d, &["log", "-1", "--format=%B", "HEAD~1"]);
    assert!(folded.contains("and why") && folded.contains("three"), "{folded}");
    assert_eq!(git(d, &["show", "--name-only", "--format=", "HEAD~1"]).split_whitespace().collect::<Vec<_>>(), ["b.txt", "c.txt"]);

    // The first commit has nothing before it to be folded into, and nothing is left half done.
    let one = git(d, &["rev-parse", "HEAD~2"]).trim().to_string();
    assert!(rewrite(d, exe, &one, &Rewrite::Squash).is_err());
    assert_eq!(subjects(d), ["four", "two, said better", "one"]);

    // Dropping the newest commit takes its file with it.
    let four = git(d, &["rev-parse", "HEAD"]).trim().to_string();
    rewrite(d, exe, &four, &Rewrite::Drop).unwrap();
    assert_eq!(subjects(d), ["two, said better", "one"]);
    assert!(!d.join("d.txt").exists());

    // Also the very first commit can get a new message.
    rewrite(d, exe, &one, &Rewrite::Reword("the beginning".into())).unwrap();
    assert_eq!(subjects(d), ["two, said better", "the beginning"]);

    // A merge has no line of its own in the todo list: refused, and the branch is as it was.
    git(d, &["stash", "-q"]);
    git(d, &["switch", "-q", "-c", "side", "HEAD~1"]);
    commit(d, "s.txt", "on the side");
    git(d, &["switch", "-q", "main"]);
    git(d, &["merge", "-q", "--no-ff", "-m", "merge the side", "side"]);
    let merge = git(d, &["rev-parse", "HEAD"]).trim().to_string();
    let before = subjects(d);
    assert!(rewrite(d, exe, &merge, &Rewrite::Reword("x".into())).is_err());
    assert_eq!(subjects(d), before);
    assert!(!d.join(".git/rebase-merge").exists(), "no rebase left open");
    assert_eq!(git(d, &["rev-parse", "HEAD"]).trim(), merge);
}
