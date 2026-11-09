//! Builds partial patches so single hunks or lines can be staged, unstaged or discarded.

use crate::parse::{FileDiff, Hunk};
use std::collections::HashSet;

/// Which way the patch will be applied.
#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    /// `git apply --cached`: stage selected lines of the worktree diff.
    Forward,
    /// `git apply [--cached] -R`: remove selected lines (unstage from the staged diff,
    /// or discard from the worktree diff).
    Reverse,
}

/// `selected` holds (hunk index, line index) pairs. Returns None when nothing is selected.
pub fn build(diff: &FileDiff, selected: &HashSet<(usize, usize)>, mode: Mode) -> Option<String> {
    let path = &diff.path;
    let old = diff.old_path.as_deref().unwrap_or(path);
    let mut body = String::new();
    let mut delta: i64 = 0;
    for (hi, h) in diff.hunks.iter().enumerate() {
        if !h.lines.iter().enumerate().any(|(li, l)| l.t != ' ' && selected.contains(&(hi, li))) {
            continue;
        }
        let (text, old_n, new_n) = hunk_body(h, hi, selected, mode);
        // A hunk with no old lines inserts *after* old_start, so the new side starts one later.
        let mut new_start = h.old_start as i64 + delta + if old_n == 0 { 1 } else { 0 };
        if new_n == 0 {
            new_start -= 1;
        }
        body.push_str(&format!("@@ -{},{} +{},{} @@\n", h.old_start, old_n, new_start.max(0), new_n));
        body.push_str(&text);
        delta += new_n as i64 - old_n as i64;
    }
    if body.is_empty() {
        return None;
    }
    let from = if diff.new_file { "/dev/null".to_string() } else { format!("a/{old}") };
    let to = if diff.deleted { "/dev/null".to_string() } else { format!("b/{path}") };
    Some(format!("diff --git a/{old} b/{path}\n--- {from}\n+++ {to}\n{body}"))
}

fn hunk_body(h: &Hunk, hi: usize, selected: &HashSet<(usize, usize)>, mode: Mode) -> (String, u32, u32) {
    let mut s = String::new();
    let (mut old_n, mut new_n) = (0u32, 0u32);
    for (li, l) in h.lines.iter().enumerate() {
        let sel = selected.contains(&(hi, li));
        // Unselected lines keep the side the patch is applied to unchanged.
        let t = match (l.t, sel, mode) {
            (' ', _, _) => ' ',
            (c, true, _) => c,
            ('-', false, Mode::Forward) => ' ',
            ('+', false, Mode::Forward) => continue,
            ('+', false, Mode::Reverse) => ' ',
            ('-', false, Mode::Reverse) => continue,
            _ => continue,
        };
        if t != '+' {
            old_n += 1;
        }
        if t != '-' {
            new_n += 1;
        }
        s.push(t);
        s.push_str(&l.text);
        s.push('\n');
        if l.no_eol {
            s.push_str("\\ No newline at end of file\n");
        }
    }
    (s, old_n, new_n)
}
