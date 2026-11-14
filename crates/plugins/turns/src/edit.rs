//! One tentative edit of a tour as positions given up and positions taken
//! on, tried against the shared visit counts.
//!
//! A move lists the positions of the old way and of the new way; whatever
//! both list cancels out, so a move that keeps a long lane and changes its
//! end only touches the end. The edit is kept if every cell around the
//! positions given up stays swept, and reverted otherwise.

use mow_core::Point;
use mow_coverage::{CoverageError, VisitCounts};

/// The positions of one edit, reused from move to move.
#[derive(Debug, Default)]
pub struct Edit {
    removed: Vec<Point>,
    added: Vec<Point>,
    net_removed: Vec<Point>,
    net_added: Vec<Point>,
}

impl Edit {
    pub fn clear(&mut self) {
        self.removed.clear();
        self.added.clear();
    }

    /// Gives up the positions of the path through `corners`, all but the first.
    pub fn remove_path(&mut self, corners: &[Point]) {
        push_path(&mut self.removed, corners);
    }

    /// Takes on the positions of the path through `corners`, all but the first.
    pub fn add_path(&mut self, corners: &[Point]) {
        push_path(&mut self.added, corners);
    }

    pub fn remove_point(&mut self, p: Point) {
        self.removed.push(p);
    }

    pub fn add_point(&mut self, p: Point) {
        self.added.push(p);
    }

    /// Applies the edit to `counts` if every cell stays swept. True if kept;
    /// otherwise the counts are as they were.
    pub fn try_on(&mut self, counts: &mut VisitCounts) -> Result<bool, CoverageError> {
        self.cancel();

        counts.add_positions(self.net_added.iter().copied())?;
        counts.remove_positions(self.net_removed.iter().copied())?;
        if counts.all_swept_around(self.net_removed.iter().copied()) {
            return Ok(true);
        }

        counts.add_positions(self.net_removed.iter().copied())?;
        counts.remove_positions(self.net_added.iter().copied())?;
        Ok(false)
    }

    /// The two multisets without what they share.
    fn cancel(&mut self) {
        let shared = trim_ends(&self.removed, &self.added);
        let removed = &self.removed[shared.0..self.removed.len() - shared.1];
        let added = &self.added[shared.0..self.added.len() - shared.1];

        self.net_removed.clear();
        self.net_removed.extend_from_slice(removed);
        self.net_removed.sort_unstable();
        self.net_added.clear();
        self.net_added.extend_from_slice(added);
        self.net_added.sort_unstable();

        let (r, a) = difference(&self.net_removed, &self.net_added);
        self.net_removed = r;
        self.net_added = a;
    }
}

/// How many positions the two lists share at their start and at their end,
/// the cheap part of the cancelling: a kept lane is a shared run.
fn trim_ends(a: &[Point], b: &[Point]) -> (usize, usize) {
    let front = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let room = a.len().min(b.len()) - front;
    let back = a
        .iter()
        .rev()
        .zip(b.iter().rev())
        .take(room)
        .take_while(|(x, y)| x == y)
        .count();

    (front, back)
}

/// Sorted multisets `a - b` and `b - a`.
fn difference(a: &[Point], b: &[Point]) -> (Vec<Point>, Vec<Point>) {
    let (mut only_a, mut only_b) = (Vec::new(), Vec::new());
    let (mut i, mut j) = (0, 0);

    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => {
                only_a.push(a[i]);
                i += 1;
            }
            std::cmp::Ordering::Greater => {
                only_b.push(b[j]);
                j += 1;
            }
            std::cmp::Ordering::Equal => {
                i += 1;
                j += 1;
            }
        }
    }
    only_a.extend_from_slice(&a[i..]);
    only_b.extend_from_slice(&b[j..]);

    (only_a, only_b)
}

/// The positions of the path through `corners`, all but the first corner.
fn push_path(out: &mut Vec<Point>, corners: &[Point]) {
    for w in corners.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (dx, dy) = ((b.x - a.x).signum(), (b.y - a.y).signum());
        out.extend((1..=a.l1(b) as i32).map(|t| a.shifted(dx * t, dy * t)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pts(raw: &[(i32, i32)]) -> Vec<Point> {
        raw.iter().map(|&p| p.into()).collect()
    }

    #[test]
    fn a_path_lists_every_position_but_the_first() {
        let mut out = Vec::new();
        push_path(&mut out, &pts(&[(0, 0), (2, 0), (2, 1)]));

        assert_eq!(out, pts(&[(1, 0), (2, 0), (2, 1)]));
    }

    #[test]
    fn shared_positions_cancel() {
        let mut edit = Edit::default();
        edit.remove_path(&pts(&[(0, 0), (5, 0), (5, 1)]));
        edit.add_path(&pts(&[(0, 0), (4, 0), (4, 1), (5, 1)]));
        edit.cancel();

        assert_eq!(edit.net_removed, pts(&[(5, 0)]));
        assert_eq!(edit.net_added, pts(&[(4, 1)]));
    }

    #[test]
    fn ends_trim_without_overlap() {
        let a = pts(&[(1, 0), (2, 0)]);
        assert_eq!(trim_ends(&a, &a), (2, 0));
        assert_eq!(trim_ends(&a, &pts(&[(2, 0)])), (0, 1));
    }
}
