//! Bridges gitcore's parsed diffs and the GPUI `Diff` component.
//!
//! * [`to_patch`] turns a gitcore `FileDiff` into unified-patch text for `DiffFile::parse`.
//! * [`selected_pairs`] turns a selection in the diff view (a path, a side and line numbers)
//!   back into the `(hunk, line)` pairs `ops::apply_lines` takes.

use gitcore::parse::FileDiff;
use gpui_kit::component::diff::DiffSide;

/// Unified patch text for one file, in the shape `git diff` prints it.
pub fn to_patch(d: &FileDiff) -> String {
    let new = &d.path;
    let old = d.old_path.as_deref().unwrap_or(new);
    let mut s = format!("diff --git a/{old} b/{new}\n");
    if d.binary {
        s.push_str(&format!("Binary files a/{old} and b/{new} differ\n"));
        return s;
    }
    if d.new_file {
        s.push_str("new file mode 100644\n");
    } else if d.deleted {
        s.push_str("deleted file mode 100644\n");
    } else if old != new {
        s.push_str(&format!("rename from {old}\nrename to {new}\n"));
    }
    if d.hunks.is_empty() {
        return s;
    }
    s.push_str(&if d.new_file { "--- /dev/null\n".to_string() } else { format!("--- a/{old}\n") });
    s.push_str(&if d.deleted { "+++ /dev/null\n".to_string() } else { format!("+++ b/{new}\n") });
    for h in &d.hunks {
        s.push_str(&h.header);
        s.push('\n');
        for l in &h.lines {
            s.push(l.t);
            s.push_str(&l.text);
            s.push('\n');
            if l.no_eol {
                s.push_str("\\ No newline at end of file\n");
            }
        }
    }
    s
}

struct Row {
    hunk: usize,
    line: usize,
    changed: bool,
    old_no: Option<usize>,
    new_no: Option<usize>,
}

fn rows(d: &FileDiff) -> Vec<Row> {
    let mut out = Vec::new();
    for (hi, h) in d.hunks.iter().enumerate() {
        let (mut o, mut n) = (h.old_start as usize, h.new_start as usize);
        for (li, l) in h.lines.iter().enumerate() {
            let (old_no, new_no) = match l.t {
                '+' => (None, Some(n)),
                '-' => (Some(o), None),
                _ => (Some(o), Some(n)),
            };
            if l.t != '+' {
                o += 1;
            }
            if l.t != '-' {
                n += 1;
            }
            out.push(Row { hunk: hi, line: li, changed: l.t != ' ', old_no, new_no });
        }
    }
    out
}

fn find(rows: &[Row], side: DiffSide, no: usize) -> Option<usize> {
    rows.iter().position(|r| match side {
        DiffSide::Original => r.old_no == Some(no),
        DiffSide::Modified => r.new_no == Some(no),
    })
}

/// The changed lines between two positions of the diff view, both ends included. The two
/// ends may be on different sides (a drag across a split view); they are ordered by where
/// they fall in the unified diff, so the result is the same either way round.
pub fn selected_pairs(d: &FileDiff, start: (DiffSide, usize), end: (DiffSide, usize)) -> Vec<(usize, usize)> {
    let rows = rows(d);
    let (Some(a), Some(b)) = (find(&rows, start.0, start.1), find(&rows, end.0, end.1)) else {
        return Vec::new();
    };
    let (lo, hi) = (a.min(b), a.max(b));
    rows[lo..=hi].iter().filter(|r| r.changed).map(|r| (r.hunk, r.line)).collect()
}

/// Every changed line of one hunk.
pub fn hunk_pairs(d: &FileDiff, hunk: usize) -> Vec<(usize, usize)> {
    d.hunks
        .get(hunk)
        .map(|h| h.lines.iter().enumerate().filter(|(_, l)| l.t != ' ').map(|(i, _)| (hunk, i)).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gitcore::parse::parse_diff;
    use gpui_kit::component::diff::DiffFile;

    const RAW: &str = "diff --git a/a.txt b/a.txt\nindex 111..222 100644\n--- a/a.txt\n+++ b/a.txt\n@@ -1,4 +1,5 @@\n one\n-two\n+TWO\n+two-b\n three\n four\n@@ -10,3 +11,3 @@ ctx\n ten\n-eleven\n+ELEVEN\n twelve\n";

    fn diff() -> FileDiff {
        parse_diff(RAW).remove(0)
    }

    #[test]
    fn patch_roundtrips_through_the_gpui_parser() {
        let d = diff();
        let patch = to_patch(&d);
        let files = DiffFile::parse(&patch).expect("gpui parses our patch");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path().as_ref(), "a.txt");
        assert_eq!(files[0].additions(), 3);
        assert_eq!(files[0].deletions(), 2);
        // and gitcore parses it back to the same thing
        let again = parse_diff(&patch).remove(0);
        assert_eq!(again.hunks.len(), 2);
        assert_eq!(again.hunks[0].lines.len(), d.hunks[0].lines.len());
    }

    #[test]
    fn single_added_line_on_the_modified_side() {
        // new line 3 is "+two-b": hunk 0, line index 3
        let got = selected_pairs(&diff(), (DiffSide::Modified, 3), (DiffSide::Modified, 3));
        assert_eq!(got, vec![(0, 3)]);
    }

    #[test]
    fn single_removed_line_on_the_original_side() {
        // old line 2 is "-two": hunk 0, line index 1
        let got = selected_pairs(&diff(), (DiffSide::Original, 2), (DiffSide::Original, 2));
        assert_eq!(got, vec![(0, 1)]);
    }

    #[test]
    fn context_lines_are_never_selected() {
        let got = selected_pairs(&diff(), (DiffSide::Modified, 1), (DiffSide::Modified, 1));
        assert!(got.is_empty());
    }

    #[test]
    fn range_across_sides_and_in_either_direction() {
        // from the removed "two" (old 2) to the added "two-b" (new 3)
        let fwd = selected_pairs(&diff(), (DiffSide::Original, 2), (DiffSide::Modified, 3));
        let back = selected_pairs(&diff(), (DiffSide::Modified, 3), (DiffSide::Original, 2));
        assert_eq!(fwd, vec![(0, 1), (0, 2), (0, 3)]);
        assert_eq!(fwd, back);
    }

    #[test]
    fn range_spanning_two_hunks_uses_each_hunks_own_indices() {
        let got = selected_pairs(&diff(), (DiffSide::Modified, 3), (DiffSide::Modified, 12));
        assert_eq!(got, vec![(0, 3), (1, 1), (1, 2)]);
    }

    #[test]
    fn a_whole_hunk() {
        assert_eq!(hunk_pairs(&diff(), 1), vec![(1, 1), (1, 2)]);
    }

    #[test]
    fn new_and_deleted_files_get_dev_null_headers() {
        let raw = "diff --git a/n.txt b/n.txt\nnew file mode 100644\nindex 000..111\n--- /dev/null\n+++ b/n.txt\n@@ -0,0 +1,2 @@\n+a\n+b\n";
        let d = parse_diff(raw).remove(0);
        let p = to_patch(&d);
        assert!(p.contains("--- /dev/null") && p.contains("new file mode"));
        assert_eq!(DiffFile::parse(&p).unwrap()[0].additions(), 2);
    }
}
