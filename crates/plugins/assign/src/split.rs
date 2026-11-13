//! Tour splitting: cut one long walk into at most `k` consecutive pieces so
//! the longest piece, closed, is as short as possible (port of `split.py`).
//!
//! A piece over positions `i..=j` of the walk costs
//!
//! ```text
//!     (j - i)  +  |P[j] - P[i]|_1
//!     walking     driving back to the start
//! ```
//!
//! Binary search on the target cost, and greedily make every piece as long
//! as the target allows: the Frederickson-Hecht-Kim idea for min-max k-TSP,
//! on an already ordered walk.
//!
//! Python works on the dense positions. Here a position index is an arc
//! length along the corners, so a walk of millions of positions costs only
//! a few binary searches per piece. The pieces are the same.

use mow_core::{Point, SolveError, Splitter, Walk};

/// Splits a walk into at most `k` pieces, see the module docs. Label `split`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SplitWalk;

impl Splitter for SplitWalk {
    fn label(&self) -> String {
        "split".to_owned()
    }

    fn split(&self, walk: &Walk, k: usize) -> Result<Vec<Walk>, SolveError> {
        if k <= 1 {
            return Ok(vec![walk.clone()]);
        }

        let arc = Arc::new(walk.corners());
        let pieces = piece_bounds(&arc, k)
            .into_iter()
            .map(|(first, last)| arc.piece(first, last))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(pieces)
    }
}

/// The pieces [`SplitWalk`] cuts, as first and last position index (both
/// included), the way `split_walk` slices its positions.
pub fn split_bounds(walk: &Walk, k: usize) -> Vec<(u64, u64)> {
    let arc = Arc::new(walk.corners());

    if k <= 1 {
        return vec![(0, arc.last())];
    }

    piece_bounds(&arc, k)
}

fn piece_bounds(arc: &Arc<'_>, k: usize) -> Vec<(u64, u64)> {
    let mut low = 0;
    let mut high = arc.closed_cost(0, arc.last());

    // one piece over everything always fits its own cost
    let mut best = greedy(arc, k, high).unwrap_or_else(|| vec![(0, arc.last())]);

    while low < high {
        let target = low + (high - low) / 2;

        match greedy(arc, k, target) {
            Some(pieces) => {
                best = pieces;
                high = target;
            }
            None => low = target + 1,
        }
    }

    best
}

/// Pieces as long as `target` allows, or `None` if `k` are not enough.
fn greedy(arc: &Arc<'_>, k: usize, target: u64) -> Option<Vec<(u64, u64)>> {
    let last = arc.last();
    let mut pieces = Vec::with_capacity(k);
    let mut start = 0;

    while start <= last {
        if pieces.len() == k {
            return None;
        }

        let end = farthest_end(arc, start, target);
        pieces.push((start, end));
        start = end + 1;
    }

    Some(pieces)
}

/// The last position whose piece from `start` costs at most `target`.
///
/// Every unit step adds one to the walking part and moves the distance home
/// by one either way, so the cost never falls as the end moves on: binary
/// search finds the last end that fits.
fn farthest_end(arc: &Arc<'_>, start: u64, target: u64) -> u64 {
    let mut fits = start;
    let mut too_far = start.saturating_add(target).min(arc.last()) + 1;

    while too_far - fits > 1 {
        let middle = fits + (too_far - fits) / 2;

        if arc.closed_cost(start, middle) <= target {
            fits = middle;
        } else {
            too_far = middle;
        }
    }

    fits
}

/// A walk's corners plus the arc length at each, so position `s` (the
/// `s`-th unit step point) is found by binary search.
struct Arc<'a> {
    corners: &'a [Point],
    at: Vec<u64>,
}

impl<'a> Arc<'a> {
    fn new(corners: &'a [Point]) -> Self {
        let mut at = Vec::with_capacity(corners.len());
        let mut length = 0;

        at.push(0);
        for pair in corners.windows(2) {
            length += pair[0].l1(pair[1]);
            at.push(length);
        }

        Self { corners, at }
    }

    /// The index of the last position.
    fn last(&self) -> u64 {
        self.at[self.at.len() - 1]
    }

    /// The corner at or before position `s`.
    fn corner_before(&self, s: u64) -> usize {
        self.at.partition_point(|&a| a <= s) - 1
    }

    fn position(&self, s: u64) -> Point {
        let i = self.corner_before(s);
        let from = self.corners[i];

        let Some(&to) = self.corners.get(i + 1) else {
            return from;
        };

        let t = (s - self.at[i]) as i32;
        from.shifted((to.x - from.x).signum() * t, (to.y - from.y).signum() * t)
    }

    fn closed_cost(&self, first: u64, last: u64) -> u64 {
        (last - first) + self.position(first).l1(self.position(last))
    }

    /// Positions `first..=last` as a walk of their own.
    fn piece(&self, first: u64, last: u64) -> Result<Walk, SolveError> {
        let inner = self.corner_before(first) + 1..self.corner_before(last) + 1;

        let mut corners = Vec::with_capacity(inner.len() + 2);
        corners.push(self.position(first));
        corners.extend(inner.filter(|&i| self.at[i] < last).map(|i| self.corners[i]));
        corners.push(self.position(last));

        Ok(Walk::new(corners)?.normalized())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn walk(raw: &[(i32, i32)]) -> Walk {
        Walk::new(raw.iter().map(|&p| p.into()).collect()).unwrap()
    }

    /// Python's `split_walk` on dense positions, the reference.
    fn dense_bounds(positions: &[Point], k: usize) -> Vec<(u64, u64)> {
        let cost = |i: usize, j: usize| (j - i) as u64 + positions[i].l1(positions[j]);
        let n = positions.len();

        let greedy = |target: u64| {
            let mut pieces = Vec::new();
            let mut start = 0;
            while start < n {
                if pieces.len() == k {
                    return None;
                }
                let stop = (n - 1).min(start + target as usize);
                let end = (start..=stop)
                    .filter(|&j| cost(start, j) <= target)
                    .max()
                    .unwrap();
                pieces.push((start as u64, end as u64));
                start = end + 1;
            }
            Some(pieces)
        };

        let (mut low, mut high) = (0, cost(0, n - 1));
        let mut best = greedy(high).unwrap();
        while low < high {
            let target = (low + high) / 2;
            match greedy(target) {
                Some(p) => (best, high) = (p, target),
                None => low = target + 1,
            }
        }
        best
    }

    #[test]
    fn matches_the_dense_reference_on_a_winding_walk() {
        let w = walk(&[
            (0, 0),
            (9, 0),
            (9, 3),
            (2, 3),
            (2, 1),
            (5, 1),
            (5, 8),
            (0, 8),
            (3, 8),
            (3, 2),
        ]);
        let positions: Vec<Point> = w.positions().collect();

        for k in 1..=5 {
            assert_eq!(split_bounds(&w, k), dense_bounds(&positions, k.max(1)), "k = {k}");
        }
    }

    #[test]
    fn pieces_hold_every_position_once() {
        let w = walk(&[(0, 0), (6, 0), (6, 4), (0, 4), (0, 1), (4, 1)]);
        let all: Vec<Point> = w.positions().collect();

        let pieces = SplitWalk.split(&w, 3).unwrap();
        let joined: Vec<Point> = pieces
            .iter()
            .flat_map(|p| p.positions().collect::<Vec<_>>())
            .collect();

        assert_eq!(pieces.len(), 3);
        assert_eq!(joined, all);
    }

    #[test]
    fn short_walks_and_one_cutter() {
        let point = walk(&[(2, 2)]);
        assert_eq!(SplitWalk.split(&point, 3).unwrap(), vec![point.clone()]);

        let line = walk(&[(0, 0), (4, 0)]);
        assert_eq!(SplitWalk.split(&line, 1).unwrap(), vec![line.clone()]);
        assert_eq!(split_bounds(&line, 0), vec![(0, 4)]);
    }
}
