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
