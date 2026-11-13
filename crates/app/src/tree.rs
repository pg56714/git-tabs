//! File tree rows (ported from src/lib/tree.ts).

use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Debug)]
pub struct TreeFile {
    pub path: String,
    pub status: char,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    Dir { name: String, depth: usize, dir: String },
    File { path: String, status: char, name: String, depth: usize },
}

/// Same order as the tree: in each folder, sub-folders first, then files, both A→Z.
pub fn tree_sort<T: Clone>(files: &[T], path: impl Fn(&T) -> &str) -> Vec<T> {
    let mut keyed: Vec<(String, T)> = files
        .iter()
        .map(|f| {
            let parts: Vec<&str> = path(f).split('/').collect();
            let n = parts.len();
            let k = parts
                .iter()
                .enumerate()
                .map(|(i, x)| format!("{}{}", if i < n - 1 { '0' } else { '1' }, x.to_lowercase()))
                .collect::<Vec<_>>()
                .join("/");
            (k, f.clone())
        })
        .collect();
    keyed.sort_by(|a, b| a.0.cmp(&b.0));
    keyed.into_iter().map(|x| x.1).collect()
}

/// One visible line of the File Tree tab: a folder or a file of the commit.
#[derive(Clone, Debug, PartialEq)]
pub struct FtRow {
    pub name: String,
    pub path: String,
    pub depth: usize,
    pub dir: bool,
}

#[derive(Default)]
struct FtDir<'a> {
    dirs: std::collections::HashMap<&'a str, FtDir<'a>>,
    files: Vec<&'a str>,
}

/// The lines to show for `paths` (every file of a commit) while only the folders in `open` are
/// unfolded. In each folder: sub-folders first, then files, both A→Z whatever their case.
/// Only what is unfolded gets sorted, so a huge repo costs no more than what is on screen.
pub fn file_tree_rows(paths: &[String], open: &HashSet<String>) -> Vec<FtRow> {
    let mut root = FtDir::default();
    for p in paths {
        let mut parts: Vec<&str> = p.split('/').collect();
        let file = parts.pop().unwrap_or(p);
        let mut at = &mut root;
        for d in parts {
            at = at.dirs.entry(d).or_default();
        }
        at.files.push(file);
    }
    fn emit(dir: &FtDir, prefix: &str, depth: usize, open: &HashSet<String>, out: &mut Vec<FtRow>) {
        let by_name = |a: &&str, b: &&str| a.to_lowercase().cmp(&b.to_lowercase()).then(a.cmp(b));
        let mut dirs: Vec<&str> = dir.dirs.keys().copied().collect();
        dirs.sort_by(by_name);
        for name in dirs {
            let path = format!("{prefix}{name}");
            let unfolded = open.contains(&path);
            out.push(FtRow { name: name.to_string(), path: path.clone(), depth, dir: true });
            if unfolded {
                emit(&dir.dirs[name], &format!("{path}/"), depth + 1, open, out);
            }
        }
        let mut files = dir.files.clone();
        files.sort_by(by_name);
        out.extend(files.into_iter().map(|name| FtRow { name: name.to_string(), path: format!("{prefix}{name}"), depth, dir: false }));
    }
    let mut out = Vec::new();
    emit(&root, "", 0, open, &mut out);
    out
}

#[derive(Default)]
struct Node {
    // Files arrive sorted, so folders are first seen in display order; keep that order.
    order: Vec<String>,
    dirs: BTreeMap<String, Node>,
    files: Vec<TreeFile>,
}

/// Flattened rows for a file tree; single-child folder chains are merged ("apps/backend/app").
pub fn tree_rows(files: &[TreeFile], collapsed: &HashSet<String>) -> Vec<Row> {
    let mut root = Node::default();
    for f in tree_sort(files, |f| &f.path) {
        let parts: Vec<String> = f.path.split('/').map(String::from).collect();
        let mut n = &mut root;
        for p in &parts[..parts.len() - 1] {
            if !n.dirs.contains_key(p) {
                n.order.push(p.clone());
            }
            n = n.dirs.entry(p.clone()).or_default();
        }
        n.files.push(f);
    }
    let mut rows = Vec::new();
    fn walk(n: &Node, depth: usize, prefix: &str, collapsed: &HashSet<String>, rows: &mut Vec<Row>) {
        for key in &n.order {
            let mut c = &n.dirs[key];
            let (mut name, mut full) = (key.clone(), format!("{prefix}{key}"));
            while c.dirs.len() == 1 && c.files.is_empty() {
                let k = &c.order[0];
                name.push('/');
                name.push_str(k);
                full.push('/');
                full.push_str(k);
                c = &c.dirs[k];
            }
            rows.push(Row::Dir { name, depth, dir: full.clone() });
            if !collapsed.contains(&full) {
                walk(c, depth + 1, &format!("{full}/"), collapsed, rows);
            }
        }
        for f in &n.files {
            rows.push(Row::File { path: f.path.clone(), status: f.status, name: f.path.rsplit('/').next().unwrap_or(&f.path).to_string(), depth });
        }
    }
    walk(&root, 0, "", collapsed, &mut rows);
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_whole_tree_shows_only_what_is_unfolded() {
        let paths: Vec<String> = ["README.md", "Cargo.toml", ".gitignore", "src/main.rs", "src/views/mod.rs", "src/app.rs", "crates/x/lib.rs"].iter().map(|s| s.to_string()).collect();
        let line = |r: &FtRow| format!("{}{}{}", "  ".repeat(r.depth), r.name, if r.dir { "/" } else { "" });
        let folded: Vec<String> = file_tree_rows(&paths, &HashSet::new()).iter().map(line).collect();
        assert_eq!(folded, ["crates/", "src/", ".gitignore", "Cargo.toml", "README.md"], "folders first, then files, A to Z whatever the case");
        let open: HashSet<String> = ["src".to_string(), "src/views".to_string()].into();
        let rows = file_tree_rows(&paths, &open);
        assert_eq!(rows.iter().map(line).collect::<Vec<_>>(), ["crates/", "src/", "  views/", "    mod.rs", "  app.rs", "  main.rs", ".gitignore", "Cargo.toml", "README.md"]);
        assert_eq!(rows[3].path, "src/views/mod.rs");
        assert_eq!(rows[2].path, "src/views");
    }

    fn f(p: &str) -> TreeFile {
        TreeFile { path: p.into(), status: 'M' }
    }

    #[test]
    fn folders_before_files_and_case_insensitive() {
        let s = tree_sort(&[f("b.txt"), f("src/z.rs"), f("A.txt"), f("src/a.rs")], |x| &x.path);
        let p: Vec<_> = s.iter().map(|x| x.path.as_str()).collect();
        assert_eq!(p, ["src/a.rs", "src/z.rs", "A.txt", "b.txt"]);
    }

    #[test]
    fn single_child_chains_merge() {
        let rows = tree_rows(&[f("apps/backend/app/main.py"), f("README.md")], &HashSet::new());
        assert_eq!(rows[0], Row::Dir { name: "apps/backend/app".into(), depth: 0, dir: "apps/backend/app".into() });
        assert!(matches!(&rows[1], Row::File { name, depth: 1, .. } if name == "main.py"));
        assert!(matches!(&rows[2], Row::File { name, depth: 0, .. } if name == "README.md"));
    }

    #[test]
    fn collapsed_dirs_hide_their_files() {
        let mut c = HashSet::new();
        c.insert("src".to_string());
        let rows = tree_rows(&[f("src/a.rs"), f("src/b.rs"), f("x.rs")], &c);
        assert_eq!(rows.len(), 2);
    }
}
