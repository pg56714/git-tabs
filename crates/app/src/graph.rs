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
}

/// `commits` is (sha, parents), newest first.
pub fn layout(commits: &[(&str, &[String])]) -> Vec<GraphRow> {
    let mut lanes: Vec<Option<String>> = Vec::new();
    let mut colors: Vec<usize> = Vec::new();
    let mut next_color = 0usize;
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
    let mut out = Vec::with_capacity(commits.len());
    for (sha, parents) in commits {
        let col = match lanes.iter().position(|l| l.as_deref() == Some(*sha)) {
            Some(c) => c,
            None => {
                let c = open(&mut lanes, &mut colors);
                colors[c] = next_color;
                next_color += 1;
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
                    let k = open(&mut lanes, &mut colors);
                    lanes[k] = Some(p.clone());
                    colors[k] = next_color;
                    next_color += 1;
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
        out.push(GraphRow { col, color, segs, width });
    }
    out
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
        let rows = layout(&[("a", &pa), ("b", &pb), ("c", &pc)]);
        assert!(rows.iter().all(|r| r.col == 0 && r.width == 1));
        assert_eq!(rows[0].segs, vec![Seg { x1: 0, y1: 1, x2: 0, y2: 2, color: 0 }]);
        assert!(rows[2].segs.iter().all(|g| g.y2 != 2 || g.y1 == 0), "the root has nothing below it");
    }

    #[test]
    fn merge_opens_a_second_lane_and_closes_it_again() {
        // m merges (a, b); both continue down to the root r
        let (pm, pa, pb, pr) = (vec![s("a"), s("b")], vec![s("r")], vec![s("r")], Vec::<String>::new());
        let rows = layout(&[("m", &pm), ("a", &pa), ("b", &pb), ("r", &pr)]);
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
        let rows = layout(&[("a", &p1), ("b", &p2)]);
        assert_eq!(rows[0].col, 0);
        assert_eq!(rows[1].col, 1);
        assert_ne!(rows[0].color, rows[1].color);
    }
}
