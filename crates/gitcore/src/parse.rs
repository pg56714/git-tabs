//! Parsers for git's machine-readable output.

use serde::Serialize;

// ---------- status --porcelain=v2 --branch -z ----------

#[derive(Debug, Default, Clone, Serialize)]
pub struct BranchInfo {
    pub oid: Option<String>,
    /// None when HEAD is detached.
    pub head: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusEntry {
    pub path: String,
    pub orig_path: Option<String>,
    /// Index (staged) state, '.' when unchanged.
    pub x: char,
    /// Worktree (unstaged) state, '.' when unchanged.
    pub y: char,
    pub untracked: bool,
    pub conflicted: bool,
}

pub fn parse_status(out: &str) -> (BranchInfo, Vec<StatusEntry>) {
    let mut b = BranchInfo::default();
    let mut files = Vec::new();
    let mut it = out.split('\0').filter(|s| !s.is_empty());
    while let Some(rec) = it.next() {
        if let Some(h) = rec.strip_prefix("# ") {
            let (k, v) = h.split_once(' ').unwrap_or((h, ""));
            match k {
                "branch.oid" => b.oid = (v != "(initial)").then(|| v.to_string()),
                "branch.head" => b.head = (v != "(detached)").then(|| v.to_string()),
                "branch.upstream" => b.upstream = Some(v.to_string()),
                "branch.ab" => {
                    for p in v.split(' ') {
                        if let Some(n) = p.strip_prefix('+') {
                            b.ahead = n.parse().unwrap_or(0);
                        } else if let Some(n) = p.strip_prefix('-') {
                            b.behind = n.parse().unwrap_or(0);
                        }
                    }
                }
                _ => {}
            }
            continue;
        }
        let kind = rec.as_bytes()[0];
        let xy = |s: &str| {
            let mut c = s.chars();
            (c.next().unwrap_or('.'), c.next().unwrap_or('.'))
        };
        match kind {
            b'1' => {
                let f: Vec<&str> = rec.splitn(9, ' ').collect();
                if f.len() == 9 {
                    let (x, y) = xy(f[1]);
                    files.push(StatusEntry { path: f[8].into(), orig_path: None, x, y, untracked: false, conflicted: false });
                }
            }
            b'2' => {
                let f: Vec<&str> = rec.splitn(10, ' ').collect();
                let orig = it.next().map(str::to_string);
                if f.len() == 10 {
                    let (x, y) = xy(f[1]);
                    files.push(StatusEntry { path: f[9].into(), orig_path: orig, x, y, untracked: false, conflicted: false });
                }
            }
            b'u' => {
                let f: Vec<&str> = rec.splitn(11, ' ').collect();
                if f.len() == 11 {
                    let (x, y) = xy(f[1]);
                    files.push(StatusEntry { path: f[10].into(), orig_path: None, x, y, untracked: false, conflicted: true });
                }
            }
            b'?' => files.push(StatusEntry { path: rec[2..].into(), orig_path: None, x: '.', y: '?', untracked: true, conflicted: false }),
            _ => {}
        }
    }
    (b, files)
}

// ---------- unified diff ----------

#[derive(Debug, Clone, Serialize)]
pub struct DiffLine {
    /// ' ', '+' or '-'
    pub t: char,
    pub text: String,
    /// Followed by "\ No newline at end of file".
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub no_eol: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Hunk {
    pub header: String,
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct FileDiff {
    pub path: String,
    pub old_path: Option<String>,
    pub binary: bool,
    pub new_file: bool,
    pub deleted: bool,
    pub hunks: Vec<Hunk>,
}

fn parse_range(s: &str) -> (u32, u32) {
    match s.split_once(',') {
        Some((a, b)) => (a.parse().unwrap_or(0), b.parse().unwrap_or(0)),
        None => (s.parse().unwrap_or(0), 1),
    }
}

/// "@@ -a,b +c,d @@ ctx" -> (a, b, c, d)
pub fn parse_hunk_header(h: &str) -> Option<(u32, u32, u32, u32)> {
    let rest = h.strip_prefix("@@ -")?;
    let end = rest.find(" @@")?;
    let (old, new) = rest[..end].split_once(" +")?;
    let (a, b) = parse_range(old);
    let (c, d) = parse_range(new);
    Some((a, b, c, d))
}

/// Parses `git diff` output for a single file. Returns None for empty output.
pub fn parse_diff(out: &str) -> Vec<FileDiff> {
    let mut files: Vec<FileDiff> = Vec::new();
    let mut cur: Option<FileDiff> = None;
    let mut in_hunk = false;
    for line in out.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.starts_with("diff --git ") {
            if let Some(f) = cur.take() {
                files.push(f);
            }
            cur = Some(FileDiff::default());
            in_hunk = false;
            continue;
        }
        let Some(f) = cur.as_mut() else { continue };
        if !in_hunk || line.starts_with("@@ ") {
            if line.starts_with("@@ ") {
                if let Some((a, b, c, d)) = parse_hunk_header(line) {
                    f.hunks.push(Hunk { header: line.into(), old_start: a, old_lines: b, new_start: c, new_lines: d, lines: vec![] });
                    in_hunk = true;
                }
            } else if let Some(p) = line.strip_prefix("--- ") {
                if p != "/dev/null" {
                    f.old_path = Some(p.strip_prefix("a/").unwrap_or(p).to_string());
                }
            } else if let Some(p) = line.strip_prefix("+++ ") {
                if p != "/dev/null" {
                    f.path = p.strip_prefix("b/").unwrap_or(p).to_string();
                }
            } else if line.starts_with("new file mode") {
                f.new_file = true;
            } else if line.starts_with("deleted file mode") {
                f.deleted = true;
            } else if let Some(p) = line.strip_prefix("rename from ") {
                f.old_path = Some(p.into());
            } else if let Some(p) = line.strip_prefix("rename to ") {
                f.path = p.into();
            } else if line.starts_with("Binary files ") || line == "GIT binary patch" {
                f.binary = true;
            }
            continue;
        }
        let h = f.hunks.last_mut().unwrap();
        match line.as_bytes().first() {
            Some(b'+') | Some(b'-') | Some(b' ') => {
                h.lines.push(DiffLine { t: line.as_bytes()[0] as char, text: line[1..].to_string(), no_eol: false });
            }
            Some(b'\\') => {
                if let Some(l) = h.lines.last_mut() {
                    l.no_eol = true;
                }
            }
            None => {
                // A blank line inside a hunk is a context line whose text is empty only if
                // the hunk still expects lines; otherwise it's the trailing newline.
                let used_old = h.lines.iter().filter(|l| l.t != '+').count() as u32;
                let used_new = h.lines.iter().filter(|l| l.t != '-').count() as u32;
                if used_old < h.old_lines || used_new < h.new_lines {
                    h.lines.push(DiffLine { t: ' ', text: String::new(), no_eol: false });
                }
            }
            _ => in_hunk = false,
        }
    }
    if let Some(f) = cur.take() {
        files.push(f);
    }
    for f in &mut files {
        if f.deleted && f.path.is_empty() {
            f.path = f.old_path.clone().unwrap_or_default();
        }
        if f.old_path.as_deref() == Some(f.path.as_str()) {
            f.old_path = None;
        }
    }
    files
}

// ---------- log ----------

pub const LOG_FORMAT: &str = "%H%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%D%x1f%s%x1e";

#[derive(Debug, Clone, Serialize)]
pub struct RefLabel {
    pub name: String,
    /// "head" (current branch), "branch", "remote", "tag", "detached"
    pub kind: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct Commit {
    pub sha: String,
    pub parents: Vec<String>,
    pub author: String,
    pub email: String,
    pub time: i64,
    pub refs: Vec<RefLabel>,
    pub subject: String,
}

/// Parses `%D` produced with `--decorate=full`.
pub fn parse_decorations(d: &str) -> Vec<RefLabel> {
    let mut out = Vec::new();
    for part in d.split(", ").map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(b) = part.strip_prefix("HEAD -> ") {
            out.push(RefLabel { name: b.trim_start_matches("refs/heads/").into(), kind: "head" });
        } else if part == "HEAD" {
            out.push(RefLabel { name: "HEAD".into(), kind: "detached" });
        } else if let Some(t) = part.strip_prefix("tag: ") {
            out.push(RefLabel { name: t.trim_start_matches("refs/tags/").into(), kind: "tag" });
        } else if let Some(b) = part.strip_prefix("refs/heads/") {
            out.push(RefLabel { name: b.into(), kind: "branch" });
        } else if let Some(r) = part.strip_prefix("refs/remotes/") {
            if !r.ends_with("/HEAD") {
                out.push(RefLabel { name: r.into(), kind: "remote" });
            }
        } else if let Some(s) = part.strip_prefix("refs/stash") {
            let _ = s;
        }
    }
    out
}

pub fn parse_log(out: &str) -> Vec<Commit> {
    out.split('\x1e')
        .map(|r| r.trim_start_matches(['\n', '\r']))
        .filter(|r| !r.is_empty())
        .filter_map(|r| {
            let f: Vec<&str> = r.splitn(7, '\x1f').collect();
            if f.len() < 7 {
                return None;
            }
            Some(Commit {
                sha: f[0].into(),
                parents: f[1].split(' ').filter(|s| !s.is_empty()).map(String::from).collect(),
                author: f[2].into(),
                email: f[3].into(),
                time: f[4].parse().unwrap_or(0),
                refs: parse_decorations(f[5]),
                subject: f[6].trim_end().into(),
            })
        })
        .collect()
}

// ---------- name-status / numstat (-z) ----------

#[derive(Debug, Clone, Serialize)]
pub struct ChangedFile {
    pub path: String,
    pub old_path: Option<String>,
    pub status: char,
    pub add: Option<u32>,
    pub del: Option<u32>,
}

pub fn parse_name_status(out: &str) -> Vec<ChangedFile> {
    let mut v = Vec::new();
    let mut it = out.split('\0').filter(|s| !s.is_empty());
    while let Some(code) = it.next() {
        let status = code.chars().next().unwrap_or('M');
        if status == 'R' || status == 'C' {
            let old = it.next().unwrap_or_default().to_string();
            let new = it.next().unwrap_or_default().to_string();
            v.push(ChangedFile { path: new, old_path: Some(old), status, add: None, del: None });
        } else {
            let p = it.next().unwrap_or_default().to_string();
            v.push(ChangedFile { path: p, old_path: None, status, add: None, del: None });
        }
    }
    v
}

/// Fills add/del counts from `--numstat -z` output (binary files stay None).
pub fn apply_numstat(files: &mut [ChangedFile], out: &str) {
    let mut it = out.split('\0');
    let mut i = 0;
    while let Some(rec) = it.next() {
        if rec.is_empty() {
            continue;
        }
        let mut f = rec.splitn(3, '\t');
        let (a, d, p) = (f.next().unwrap_or(""), f.next().unwrap_or(""), f.next().unwrap_or(""));
        let path = if p.is_empty() {
            let _old = it.next();
            it.next().unwrap_or_default().to_string()
        } else {
            p.to_string()
        };
        let idx = files.iter().position(|x| x.path == path).or_else(|| (i < files.len()).then_some(i));
        if let Some(k) = idx {
            files[k].add = a.parse().ok();
            files[k].del = d.parse().ok();
        }
        i += 1;
    }
}

// ---------- refs ----------

pub const REF_FORMAT: &str = "%(refname)%1f%(objectname)%1f%(upstream:short)%1f%(upstream:track,nobracket)%1f%(HEAD)%1f%(committerdate:unix)%1f%(*objectname)";

#[derive(Debug, Clone, Serialize)]
pub struct Branch {
    pub name: String,
    pub sha: String,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub gone: bool,
    pub current: bool,
    pub time: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Tag {
    pub name: String,
    pub sha: String,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct Refs {
    pub branches: Vec<Branch>,
    pub remotes: Vec<Branch>,
    pub tags: Vec<Tag>,
}

pub fn parse_track(t: &str) -> (u32, u32, bool) {
    let (mut a, mut b) = (0, 0);
    for part in t.split(", ") {
        if let Some(n) = part.strip_prefix("ahead ") {
            a = n.parse().unwrap_or(0);
        } else if let Some(n) = part.strip_prefix("behind ") {
            b = n.parse().unwrap_or(0);
        }
    }
    (a, b, t == "gone")
}

pub fn parse_refs(out: &str) -> Refs {
    let mut r = Refs::default();
    for line in out.lines().filter(|l| !l.is_empty()) {
        let f: Vec<&str> = line.split('\x1f').collect();
        if f.len() < 6 {
            continue;
        }
        let (refname, sha) = (f[0], f[1].to_string());
        let time = f[5].parse().unwrap_or(0);
        if let Some(n) = refname.strip_prefix("refs/heads/") {
            let (ahead, behind, gone) = parse_track(f[3]);
            r.branches.push(Branch { name: n.into(), sha, upstream: (!f[2].is_empty()).then(|| f[2].into()), ahead, behind, gone, current: f[4] == "*", time });
        } else if let Some(n) = refname.strip_prefix("refs/remotes/") {
            if !n.ends_with("/HEAD") {
                r.remotes.push(Branch { name: n.into(), sha, upstream: None, ahead: 0, behind: 0, gone: false, current: false, time });
            }
        } else if let Some(n) = refname.strip_prefix("refs/tags/") {
            // Annotated tags point at a tag object; show the commit it tags.
            let peeled = f.get(6).copied().unwrap_or("");
            r.tags.push(Tag { name: n.into(), sha: if peeled.is_empty() { sha } else { peeled.into() } });
        }
    }
    r
}

// ---------- worktree / stash / reflog ----------

#[derive(Debug, Clone, Serialize)]
pub struct Worktree {
    pub path: String,
    pub head: String,
    pub branch: Option<String>,
    pub bare: bool,
}

pub fn parse_worktrees(out: &str) -> Vec<Worktree> {
    let mut v = Vec::new();
    for block in out.replace("\r\n", "\n").split("\n\n") {
        let mut w = Worktree { path: String::new(), head: String::new(), branch: None, bare: false };
        for l in block.lines() {
            if let Some(p) = l.strip_prefix("worktree ") {
                w.path = p.into();
            } else if let Some(h) = l.strip_prefix("HEAD ") {
                w.head = h.into();
            } else if let Some(b) = l.strip_prefix("branch ") {
                w.branch = Some(b.trim_start_matches("refs/heads/").into());
            } else if l == "bare" {
                w.bare = true;
            }
        }
        if !w.path.is_empty() {
            v.push(w);
        }
    }
    v
}

#[derive(Debug, Clone, Serialize)]
pub struct Stash {
    pub index: u32,
    pub sha: String,
    pub message: String,
}

pub fn parse_stashes(out: &str) -> Vec<Stash> {
    out.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.splitn(3, '\x1f').collect();
            let idx = f.first()?.strip_prefix("stash@{")?.strip_suffix('}')?.parse().ok()?;
            Some(Stash { index: idx, sha: f.get(1)?.to_string(), message: f.get(2).unwrap_or(&"").to_string() })
        })
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct ReflogEntry {
    pub selector: String,
    pub sha: String,
    pub message: String,
    pub time: i64,
}

pub fn parse_reflog(out: &str) -> Vec<ReflogEntry> {
    out.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.splitn(4, '\x1f').collect();
            (f.len() == 4).then(|| ReflogEntry { selector: f[0].into(), sha: f[1].into(), time: f[2].parse().unwrap_or(0), message: f[3].into() })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_v2() {
        let s = "# branch.oid abc\0# branch.head main\0# branch.upstream origin/main\0# branch.ab +2 -1\0\
1 .M N... 100644 100644 100644 h1 h2 src/a b.rs\0\
2 R. N... 100644 100644 100644 h1 h2 R100 new.rs\0old.rs\0\
u UU N... 100644 100644 100644 100644 h1 h2 h3 conf.txt\0\
? 新檔案.txt\0";
        let (b, f) = parse_status(s);
        assert_eq!(b.head.as_deref(), Some("main"));
        assert_eq!((b.ahead, b.behind), (2, 1));
        assert_eq!(f.len(), 4);
        assert_eq!(f[0].path, "src/a b.rs");
        assert_eq!((f[0].x, f[0].y), ('.', 'M'));
        assert_eq!(f[1].orig_path.as_deref(), Some("old.rs"));
        assert!(f[2].conflicted);
        assert!(f[3].untracked && f[3].path == "新檔案.txt");
    }

    #[test]
    fn diff_with_no_eol_and_blank_context() {
        let d = "diff --git a/x.txt b/x.txt\nindex 1..2 100644\n--- a/x.txt\n+++ b/x.txt\n@@ -1,3 +1,3 @@ fn x\n a\n\n-b\n\\ No newline at end of file\n+c\n\\ No newline at end of file\n";
        let f = &parse_diff(d)[0];
        assert_eq!(f.path, "x.txt");
        let h = &f.hunks[0];
        assert_eq!(h.lines.len(), 4);
        assert_eq!(h.lines[1].text, "");
        assert!(h.lines[2].no_eol && h.lines[3].no_eol);
    }

    #[test]
    fn decorations() {
        let r = parse_decorations("HEAD -> refs/heads/feat/x, refs/remotes/origin/feat/x, refs/remotes/origin/HEAD, tag: refs/tags/v1, refs/heads/main");
        let k: Vec<_> = r.iter().map(|x| (x.name.as_str(), x.kind)).collect();
        assert_eq!(k, vec![("feat/x", "head"), ("origin/feat/x", "remote"), ("v1", "tag"), ("main", "branch")]);
    }

    #[test]
    fn name_status_and_numstat() {
        let mut f = parse_name_status("M\0a.rs\0R087\0old.rs\0new.rs\0A\0img.png\0");
        apply_numstat(&mut f, "3\t1\ta.rs\x002\t2\t\0old.rs\0new.rs\0-\t-\timg.png\0");
        assert_eq!(f[1].path, "new.rs");
        assert_eq!((f[0].add, f[0].del), (Some(3), Some(1)));
        assert_eq!((f[1].add, f[1].del), (Some(2), Some(2)));
        assert_eq!(f[2].add, None);
    }
}
