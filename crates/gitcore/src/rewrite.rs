//! Rewriting one commit of the checked-out branch: a new message, folding it into the commit
//! before it, or dropping it.
//!
//! This is `git rebase -i` with nobody at the keyboard. Git asks an "editor" to edit the todo
//! list and the commit message; that editor is this same program (see [`editor_main`]), told
//! through the environment which line to change and which message to write.

use std::path::Path;

use crate::runner::{read, write_env};

pub enum Rewrite {
    /// Replace the commit's message.
    Reword(String),
    /// Fold the commit into the one before it, keeping both messages.
    Squash,
    Drop,
}

const SHA: &str = "GIT_TABS_TODO_SHA";
const VERB: &str = "GIT_TABS_TODO_VERB";
const MSG: &str = "GIT_TABS_MSG_FILE";

/// The todo list with the line of `sha` switched from `pick` to `verb`. None when the commit has
/// no `pick` line of its own: a merge has none, and neither has a commit outside the rebased range.
pub fn edit_todo(todo: &str, sha: &str, verb: &str) -> Option<String> {
    let mut found = false;
    let lines: Vec<String> = todo
        .lines()
        .map(|l| {
            let mut words = l.splitn(3, ' ');
            match (words.next(), words.next()) {
                // The todo abbreviates the commit; it is the same commit when the full name starts with it.
                (Some("pick" | "p"), Some(abbrev)) if !found && abbrev.len() >= 4 && sha.starts_with(abbrev) => {
                    found = true;
                    format!("{verb} {abbrev}{}", words.next().map(|rest| format!(" {rest}")).unwrap_or_default())
                }
                _ => l.to_string(),
            }
        })
        .collect();
    found.then(|| lines.join("\n") + "\n")
}

/// The part git runs as its editor. `--rebase-todo <file>` edits the todo list, `--commit-msg <file>`
/// writes the prepared message (or leaves git's own, when none was prepared). Returns the exit
/// code, or None when the arguments ask for neither and the program should start normally.
pub fn editor_main(args: &[String]) -> Option<i32> {
    let (mode, file) = match args {
        [_, mode, file] if mode == "--rebase-todo" || mode == "--commit-msg" => (mode.as_str(), Path::new(file)),
        _ => return None,
    };
    let done = if mode == "--rebase-todo" {
        let (sha, verb) = (std::env::var(SHA).unwrap_or_default(), std::env::var(VERB).unwrap_or_default());
        std::fs::read_to_string(file).ok().and_then(|todo| edit_todo(&todo, &sha, &verb)).is_some_and(|new| std::fs::write(file, new).is_ok())
    } else {
        match std::env::var(MSG).ok().filter(|m| !m.is_empty()) {
            Some(prepared) => std::fs::copy(prepared, file).is_ok(),
            None => true,
        }
    };
    // A failing editor makes git give the rebase up before it changed anything.
    Some(if done { 0 } else { 1 })
}

/// How git is told to run `exe`: one shell word, with forward slashes also on Windows.
fn editor(exe: &Path, mode: &str) -> String {
    format!("\"{}\" {mode}", exe.display().to_string().replace('\\', "/"))
}

/// Rewrites `sha`, a commit of the checked-out branch, and replays the commits after it.
/// `exe` is this program, which git calls back as its editor. Uncommitted changes are stashed
/// and put back. A conflict while replaying leaves the rebase open, to be resolved and continued
/// like any other; every other failure leaves the branch as it was.
pub fn rewrite(repo: &Path, exe: &Path, sha: &str, how: &Rewrite) -> Result<String, String> {
    let full = read(repo, &["rev-parse", "--verify", "-q", &format!("{sha}^{{commit}}")]).map_err(|_| "找不到這個 commit".to_string())?.trim().to_string();
    let parent_of = |rev: &str| read(repo, &["rev-parse", "--verify", "-q", &format!("{rev}^")]).ok().map(|s| s.trim().to_string());
    // The oldest commit that changes: the commit itself, or the one it is folded into.
    let first = match how {
        Rewrite::Squash => parent_of(&full).ok_or_else(|| "這是第一個 commit，前面沒有可以併入的 commit".to_string())?,
        _ => full.clone(),
    };
    let (verb, prepared) = match how {
        Rewrite::Reword(msg) => {
            let f = std::env::temp_dir().join(format!("git-tabs-msg-{}.txt", std::process::id()));
            std::fs::write(&f, msg.trim_end().to_string() + "\n").map_err(|e| e.to_string())?;
            ("reword", Some(f))
        }
        Rewrite::Squash => ("squash", None),
        Rewrite::Drop => ("drop", None),
    };
    // Everything from `first` up is replayed, onto what lies below it; the very first commit has nothing below.
    let base = parent_of(&first);
    let mut args = vec!["-c", "rebase.autoStash=true", "rebase", "-i", "--rebase-merges"];
    args.push(base.as_deref().unwrap_or("--root"));
    let (todo_editor, msg_editor) = (editor(exe, "--rebase-todo"), editor(exe, "--commit-msg"));
    let msg_file = prepared.as_ref().map(|f| f.display().to_string()).unwrap_or_default();
    let env = [("GIT_SEQUENCE_EDITOR", todo_editor.as_str()), ("GIT_EDITOR", msg_editor.as_str()), (SHA, full.as_str()), (VERB, verb), (MSG, msg_file.as_str())];
    let out = write_env(repo, &args, &env);
    if let Some(f) = prepared {
        let _ = std::fs::remove_file(f);
    }
    if out.is_err() && rebasing(repo) && read(repo, &["diff", "--name-only", "--diff-filter=U"]).is_ok_and(|u| u.trim().is_empty()) {
        // Stopped, and not on a conflict the user could resolve: put everything back.
        let _ = write_env(repo, &["rebase", "--abort"], &[]);
    }
    out
}

fn rebasing(repo: &Path) -> bool {
    read(repo, &["rev-parse", "--git-path", "rebase-merge"]).is_ok_and(|p| {
        let p = Path::new(p.trim());
        if p.is_absolute() { p.is_dir() } else { repo.join(p).is_dir() }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_line_of_the_commit_gets_the_verb() {
        let todo = "pick 1111111 one\npick 2222222 two: with words\npick 3333333 three\n\n# Rebase 0000000..3333333 onto 0000000\n";
        let sha = "2222222aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        assert_eq!(edit_todo(todo, sha, "reword").unwrap(), "pick 1111111 one\nreword 2222222 two: with words\npick 3333333 three\n\n# Rebase 0000000..3333333 onto 0000000\n");
        assert!(edit_todo(todo, "9999999aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "drop").is_none(), "not in the list");
    }

    #[test]
    fn a_merge_has_no_line_to_change() {
        let todo = "label onto\npick 1111111 one\nlabel side\nreset onto\nmerge -C 2222222 side # Merge branch 'side'\n";
        assert!(edit_todo(todo, "2222222aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "reword").is_none());
        assert!(edit_todo(todo, "1111111aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "drop").unwrap().contains("drop 1111111 one"));
    }

    #[test]
    fn only_the_editor_flags_are_taken() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(editor_main(&a(&["git-tabs"])), None);
        assert_eq!(editor_main(&a(&["git-tabs", "--config-dir", "x"])), None);
        assert_eq!(editor_main(&a(&["git-tabs", "--rebase-todo", "/no/such/file"])), Some(1));
    }
}
