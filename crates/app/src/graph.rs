//! Commit graph lanes (ported from src/lib/graph.ts).
//!
//! Each column remembers the sha it is waiting for; a commit takes the column waiting for
//! it, then hands that column to its first parent.

#[derive(Clone, Debug, PartialEq)]
pub struct Seg {
    pub x1: usize,
    /// 0 = top of the row, 1 = middle (the dot)
    pub y1: u8,
    pub x2: usize,
    /// 1 = middle, 2 = bottom
    pub y2: u8,
    pub color: usize,
}

#[derive(Clone, Debug)]
pub struct GraphRow {
    pub col: usize,
    pub color: usize,
    pub segs: Vec<Seg>,
    pub width: usize,
    /// More than one parent: drawn as a hollow dot.
    pub merge: bool,
    /// Not reachable from HEAD (commits only another branch has): its text is dimmed.
    pub off_head: bool,
}

/// The lanes as they stand below the rows laid out so far, so that the next page of commits is
/// appended without laying the whole history out again.
#[derive(Clone, Default)]
pub struct Layout {
    /// Shared with the list that draws them; appending copies nothing while the list is not holding on.
    pub rows: std::sync::Arc<Vec<GraphRow>>,
    lanes: Vec<Option<String>>,
    colors: Vec<usize>,
    next_color: usize,
    /// HEAD's commit has been laid out; from there on `reach` holds what it reaches further down.
    head_seen: bool,
    reach: std::collections::HashSet<String>,
}

impl Layout {
    /// Appends rows for `commits`: (sha, parents), newest first, continuing where the rows so
    /// far ended. `head` is the checked-out commit.
    pub fn extend(&mut self, commits: &[(&str, &[String])], head: Option<&str>) {
        fn open(lanes: &mut Vec<Option<String>>, colors: &mut Vec<usize>) -> usize {
            match lanes.iter().position(Option::is_none) {
                Some(k) => k,
                None => {
                    lanes.push(None);
                    colors.push(0);
                    lanes.len() - 1
                }
            }
        }
        let (lanes, colors) = (&mut self.lanes, &mut self.colors);
        let rows = std::sync::Arc::make_mut(&mut self.rows);
        rows.reserve(commits.len());
        for (sha, parents) in commits {
            let col = match lanes.iter().position(|l| l.as_deref() == Some(*sha)) {
                Some(c) => c,
                None => {
                    let c = open(lanes, colors);
                    colors[c] = self.next_color;
                    self.next_color += 1;
                    c
                }
            };
            let color = colors[col];
            let mut segs = Vec::new();
            for (i, s) in lanes.iter().enumerate() {
                let Some(s) = s else { continue };
                // Every lane waiting for this commit bends into its dot; the others pass straight through.
                if s == sha {
                    segs.push(Seg { x1: i, y1: 0, x2: col, y2: 1, color: colors[i] });
                } else {
                    segs.push(Seg { x1: i, y1: 0, x2: i, y2: 2, color: colors[i] });
                }
            }
            for l in lanes.iter_mut() {
                if l.as_deref() == Some(*sha) {
                    *l = None;
                }
            }
            if let Some(first) = parents.first() {
                lanes[col] = Some(first.clone());
                segs.push(Seg { x1: col, y1: 1, x2: col, y2: 2, color });
            }
            for p in parents.iter().skip(1) {
                let k = match lanes.iter().position(|l| l.as_ref() == Some(p)) {
                    Some(k) => k,
                    None => {
                        let k = open(lanes, colors);
                        lanes[k] = Some(p.clone());
                        colors[k] = self.next_color;
                        self.next_color += 1;
                        k
                    }
                };
                segs.push(Seg { x1: col, y1: 1, x2: k, y2: 2, color: colors[k] });
            }
            while matches!(lanes.last(), Some(None)) {
                lanes.pop();
                colors.pop();
            }
            let width = segs.iter().map(|s| s.x1.max(s.x2) + 1).chain([col + 1, lanes.len()]).max().unwrap_or(1);
            // What HEAD reaches is found in one pass, because children come before their parents:
            // everything above HEAD's own commit is out of its reach, and below it a commit is
            // reached when a reached commit names it as a parent. Until HEAD's commit turns up
            // (it may be on a later page) nothing is marked.
            let off_head = if self.head_seen {
                !self.reach.remove(*sha)
            } else if head == Some(*sha) {
                self.head_seen = true;
                rows.iter_mut().for_each(|r| r.off_head = true);
                false
            } else {
                false
            };
            if self.head_seen && !off_head {
                self.reach.extend(parents.iter().cloned());
            }
            rows.push(GraphRow { col, color, segs, width, merge: parents.len() > 1, off_head });
        }
    }
}

/// Which commits no remote has yet: those no remote branch reaches. `remote_tips` are the commits
/// a remote branch points at. With no remote branch in sight nothing is marked, as there is
/// nothing to be ahead of.
pub fn unpushed(commits: &[(&str, &[String])], remote_tips: &std::collections::HashSet<&str>) -> Vec<bool> {
    if remote_tips.is_empty() {
        return vec![false; commits.len()];
    }
    // Children come before their parents, so one pass carries "a remote has this" downwards.
    let mut reached: std::collections::HashSet<&str> = std::collections::HashSet::new();
    commits
        .iter()
        .map(|(sha, parents)| {
            let pushed = reached.remove(*sha) | remote_tips.contains(*sha);
            if pushed {
                reached.extend(parents.iter().map(String::as_str));
            }
            !pushed
        })
        .collect()
}

/// Which commits a set of collapsed merges hides: what each of them brought in, that is, what
/// its other parents reach and its first parent does not.
pub fn hidden(commits: &[(&str, &[String])], collapsed: &std::collections::HashSet<String>) -> Vec<bool> {
    let at: std::collections::HashMap<&str, usize> = commits.iter().enumerate().map(|(i, (s, _))| (*s, i)).collect();
    let parents_of = |i: usize| commits[i].1.iter().filter_map(|p| at.get(p.as_str()).copied());
    let mut hide = vec![false; commits.len()];
    for (sha, parents) in commits.iter().filter(|(sha, parents)| parents.len() > 1 && collapsed.contains(*sha)) {
        let _ = sha;
        // What the first parent reaches stays, however the other parents get there too.
        let mut keep = vec![false; commits.len()];
        let mut todo: Vec<usize> = at.get(parents[0].as_str()).copied().into_iter().collect();
        while let Some(i) = todo.pop() {
            if !std::mem::replace(&mut keep[i], true) {
                todo.extend(parents_of(i));
            }
        }
        let mut seen = vec![false; commits.len()];
        let mut todo: Vec<usize> = parents[1..].iter().filter_map(|p| at.get(p.as_str()).copied()).collect();
        while let Some(i) = todo.pop() {
            if !keep[i] && !std::mem::replace(&mut seen[i], true) {
                hide[i] = true;
                todo.extend(parents_of(i));
            }
        }
    }
    hide
}

/// The rows that are left: for each, its place in `commits` and its parents among what is still
/// shown. A collapsed merge keeps its first parent only; a parent that got hidden is replaced by
/// the nearest shown commit down its first-parent line, so that no lane is left hanging.
pub fn shown(commits: &[(&str, &[String])], collapsed: &std::collections::HashSet<String>, hide: &[bool]) -> Vec<(usize, Vec<String>)> {
    let at: std::collections::HashMap<&str, usize> = commits.iter().enumerate().map(|(i, (s, _))| (*s, i)).collect();
    let resolve = |p: &String| -> Option<String> {
        let mut p = p;
        loop {
            match at.get(p.as_str()) {
                Some(&i) if hide[i] => p = commits[i].1.first()?,
                _ => return Some(p.clone()),
            }
        }
    };
    commits
        .iter()
        .enumerate()
        .filter(|(i, _)| !hide[*i])
        .map(|(i, (sha, parents))| {
            let own = if collapsed.contains(*sha) { &parents[..parents.len().min(1)] } else { &parents[..] };
            let mut ps: Vec<String> = Vec::new();
            for p in own.iter().filter_map(resolve) {
                if !ps.contains(&p) {
                    ps.push(p);
                }
            }
            (i, ps)
        })
        .collect()
}

/// Lays `commits` out in one go.
#[cfg(test)]
pub fn layout(commits: &[(&str, &[String])], head: Option<&str>) -> Vec<GraphRow> {
    let mut l = Layout::default();
    l.extend(commits, head);
    std::sync::Arc::try_unwrap(l.rows).unwrap_or_else(|rows| (*rows).clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(x: &str) -> String {
        x.to_string()
    }

    #[test]
    fn linear_history_stays_in_one_column() {
        let (pa, pb, pc) = (vec![s("b")], vec![s("c")], Vec::<String>::new());
        let rows = layout(&[("a", &pa), ("b", &pb), ("c", &pc)], None);
        assert!(rows.iter().all(|r| r.col == 0 && r.width == 1));
        assert!(rows.iter().all(|r| !r.merge && !r.off_head));
        assert_eq!(rows[0].segs, vec![Seg { x1: 0, y1: 1, x2: 0, y2: 2, color: 0 }]);
        assert!(rows[2].segs.iter().all(|g| g.y2 != 2 || g.y1 == 0), "the root has nothing below it");
    }

    #[test]
    fn merge_opens_a_second_lane_and_closes_it_again() {
        // m merges (a, b); both continue down to the root r
        let (pm, pa, pb, pr) = (vec![s("a"), s("b")], vec![s("r")], vec![s("r")], Vec::<String>::new());
        let rows = layout(&[("m", &pm), ("a", &pa), ("b", &pb), ("r", &pr)], None);
        assert!(rows[0].merge && !rows[1].merge);
        assert_eq!(rows[0].col, 0);
        assert!(rows[0].segs.iter().any(|g| g.x1 == 0 && g.x2 == 1 && g.y1 == 1), "merge edge to lane 1");
        assert_eq!(rows[1].col, 0);
        assert_eq!(rows[2].col, 1, "b waits in the second lane");
        assert_eq!(rows[3].col, 0, "r gathers both lanes back");
        assert!(rows[3].segs.iter().any(|g| g.x1 == 1 && g.x2 == 0 && g.y2 == 1), "lane 1 bends into r");
    }

    #[test]
    fn unknown_parent_gets_a_new_lane_and_new_color() {
        let (p1, p2) = (vec![s("x")], vec![s("y")]);
        let rows = layout(&[("a", &p1), ("b", &p2)], None);
        assert_eq!(rows[0].col, 0);
        assert_eq!(rows[1].col, 1);
        assert_ne!(rows[0].color, rows[1].color);
    }

    #[test]
    fn appending_pages_gives_the_same_rows_as_one_pass() {
        // two branches that merge, a side branch left unmerged, then older history
        let h: Vec<(&str, Vec<String>)> = vec![
            ("x2", vec![s("x1")]),
            ("m", vec![s("a2"), s("b2")]),
            ("x1", vec![s("a1")]),
            ("a2", vec![s("a1")]),
            ("b2", vec![s("b1")]),
            ("b1", vec![s("a1")]),
            ("a1", vec![s("r")]),
            ("r", vec![]),
        ];
        let all: Vec<(&str, &[String])> = h.iter().map(|(a, b)| (*a, b.as_slice())).collect();
        let key = |rows: &[GraphRow]| rows.iter().map(|r| format!("{} {} {} {} {} {:?}", r.col, r.color, r.width, r.merge, r.off_head, r.segs)).collect::<Vec<_>>();
        // HEAD on the first page, on a later page, and not loaded at all
        for head in [Some("m"), Some("b1"), Some("zz"), None] {
            let whole = layout(&all, head);
            for cut in 1..all.len() {
                let mut paged = Layout::default();
                paged.extend(&all[..cut], head);
                paged.extend(&all[cut..], head);
                assert_eq!(key(&paged.rows), key(&whole), "head {head:?}, page break after {cut}");
            }
        }
        let rows = layout(&all, Some("m"));
        let off: Vec<bool> = rows.iter().map(|r| r.off_head).collect();
        assert_eq!(off, [true, false, true, false, false, false, false, false], "x2 and x1 are not in m");
    }

    #[test]
    fn commits_no_remote_has_are_marked() {
        // main is two commits ahead of origin/main (at c); side was never pushed
        let h: Vec<(&str, Vec<String>)> = vec![("e", vec![s("d")]), ("x", vec![s("c")]), ("d", vec![s("c")]), ("c", vec![s("b")]), ("b", vec![])];
        let all: Vec<(&str, &[String])> = h.iter().map(|(a, b)| (*a, b.as_slice())).collect();
        let tips: std::collections::HashSet<&str> = ["c"].into();
        assert_eq!(unpushed(&all, &tips), [true, true, true, false, false]);
        assert!(unpushed(&all, &std::collections::HashSet::new()).iter().all(|u| !u), "no remote, nothing to be ahead of");
    }

    #[test]
    fn a_collapsed_merge_hides_what_it_brought_in() {
        // m merges side (b2, b1) into main (a2, a1); x1 was branched off b1 and is still open
        let h: Vec<(&str, Vec<String>)> = vec![
            ("x1", vec![s("b1")]),
            ("m", vec![s("a2"), s("b2")]),
            ("a2", vec![s("a1")]),
            ("b2", vec![s("b1")]),
            ("b1", vec![s("a1")]),
            ("a1", vec![]),
        ];
        let all: Vec<(&str, &[String])> = h.iter().map(|(a, b)| (*a, b.as_slice())).collect();
        let none = std::collections::HashSet::new();
        assert!(hidden(&all, &none).iter().all(|h| !h));
        assert_eq!(shown(&all, &none, &hidden(&all, &none)).len(), all.len());

        let folded: std::collections::HashSet<String> = [s("m")].into();
        let hide = hidden(&all, &folded);
        assert_eq!(hide, [false, false, false, true, true, false], "b2 and b1; a1 is main's too and stays");
        let rows = shown(&all, &folded, &hide);
        let got: Vec<(&str, Vec<String>)> = rows.iter().map(|(i, ps)| (all[*i].0, ps.clone())).collect();
        assert_eq!(got, [("x1", vec![s("a1")]), ("m", vec![s("a2")]), ("a2", vec![s("a1")]), ("a1", vec![])], "x1 now hangs on the nearest commit still shown");
        // and the rows lay out without a lane left waiting for a hidden commit
        let v: Vec<(&str, &[String])> = rows.iter().map(|(i, ps)| (all[*i].0, ps.as_slice())).collect();
        let g = layout(&v, None);
        assert_eq!(g.last().unwrap().width, 2);
        assert!(g.last().unwrap().segs.iter().all(|x| x.y2 != 2), "nothing continues below the root");
    }

    #[test]
    fn commits_head_does_not_reach_are_marked() {
        // x is another branch's tip; h is HEAD; both sit on the root r
        let (px_, ph, pr) = (vec![s("r")], vec![s("r")], Vec::<String>::new());
        let rows = layout(&[("x", &px_), ("h", &ph), ("r", &pr)], Some("h"));
        assert_eq!(rows.iter().map(|r| r.off_head).collect::<Vec<_>>(), vec![true, false, false]);
        // HEAD's commit is not loaded yet: nothing is marked
        let rows = layout(&[("x", &px_), ("r", &pr)], Some("h"));
        assert!(rows.iter().all(|r| !r.off_head));
    }
}
