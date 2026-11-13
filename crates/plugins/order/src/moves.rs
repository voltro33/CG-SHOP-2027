//! The improving moves, each limited to a segment's spatial neighbours:
//!
//! - **2-opt**: reverse a block, which flips every segment in it, so that the
//!   segment gets linked to its neighbour exit to exit or entry to entry.
//! - **Or-opt**: take one segment out and put it back right before or after a
//!   neighbour, driven whichever way is shorter.
//!
//! Each returns the stops whose surroundings changed, for the descent to look
//! at again, or `None` when no move pays off.

use crate::neighbours::Neighbours;
use crate::tour::{dist, Tour};

/// Up to six stop ids, without allocating.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Touched {
    ids: [u32; 6],
    len: usize,
}

impl Touched {
    pub(crate) fn push(&mut self, id: u32) {
        self.ids[self.len] = id;
        self.len += 1;
    }

    pub(crate) fn ids(&self) -> &[u32] {
        &self.ids[..self.len]
    }

    /// The stops at the given positions.
    pub(crate) fn at(tour: &Tour, positions: &[usize]) -> Self {
        let mut touched = Self::default();
        positions.iter().for_each(|&p| touched.push(tour.seg_at[p]));
        touched
    }
}

/// The first improving 2-opt move between `segment` and one of its
/// neighbours, applied.
pub(crate) fn two_opt(tour: &mut Tour, neighbours: &Neighbours, segment: u32) -> Option<Touched> {
    let m = tour.len();
    let i = tour.pos_of[segment as usize] as usize;

    for &other in neighbours.of(segment) {
        let j = tour.pos_of[other as usize] as usize;

        // link the two exits: reverse i+1 ..= j
        let (x, len) = (tour.next(i), (j + m - i) % m);
        if let Some(touched) = try_reverse(tour, x, len) {
            return Some(touched);
        }

        // link the two entries: reverse i ..= j-1
        if let Some(touched) = try_reverse(tour, i, len) {
            return Some(touched);
        }
    }

    None
}

/// Reverses the `len` positions from `x` on if that shortens the tour.
fn try_reverse(tour: &mut Tour, x: usize, len: usize) -> Option<Touched> {
    let m = tour.len();
    if len == 0 || len >= m {
        return None;
    }

    let before = tour.prev(x);
    let y = (x + len - 1) % m;
    let after = tour.next(y);

    let old = tour.link(before) + tour.link(y);
    let new = dist(tour.ext[before], tour.ext[y]) + dist(tour.ent[x], tour.ent[after]);
    if new >= old {
        return None;
    }

    tour.reverse(x, len);
    tour.cost += new - old;

    Some(Touched::at(tour, &[before, x, y, after]))
}

/// The best place to move `segment` to, next to one of its neighbours.
pub(crate) fn or_opt(tour: &mut Tour, neighbours: &Neighbours, segment: u32) -> Option<Touched> {
    let p = tour.pos_of[segment as usize] as usize;
    let (before, after) = (tour.prev(p), tour.next(p));
    if tour.next(after) == p {
        return None;
    }

    // what taking the segment out and closing the hole saves
    let (entry, exit) = (tour.ent[p], tour.ext[p]);
    let removed = tour.link(before) + tour.link(p) - dist(tour.ext[before], tour.ent[after]);

    let mut best: Option<(i64, usize, bool)> = None;
    for &other in neighbours.of(segment) {
        let q = tour.pos_of[other as usize] as usize;

        // right after the neighbour, or right before it
        for slot in [q, tour.prev(q)] {
            if slot == p || slot == before {
                continue;
            }

            let (left, right) = (tour.ext[slot], tour.ent[tour.next(slot)]);
            let broken = dist(left, right);

            for (flip, a, b) in [(false, entry, exit), (true, exit, entry)] {
                let gain = removed + broken - dist(left, a) - dist(b, right);
                if gain > best.map_or(0, |(g, _, _)| g) {
                    best = Some((gain, slot, flip));
                }
            }
        }
    }

    let (gain, slot, flip) = best?;
    let touched = Touched::at(tour, &[before, after, slot, tour.next(slot), p]);

    tour.move_one(p, slot, flip);
    tour.cost -= gain;

    Some(touched)
}

#[cfg(test)]
mod tests {
    use mow_core::{Closure, Order, Segment, Visit};

    use super::*;

    fn h(y: i32, from: i32, to: i32) -> Segment {
        Segment::horizontal(y, from, to).unwrap()
    }

    fn tour_of(segments: &[Segment], visits: &[(u32, bool)], closure: Closure) -> Tour {
        let visits = visits
            .iter()
            .map(|&(segment, reversed)| Visit { segment, reversed })
            .collect();
        Tour::new(segments, &Order::new(visits, segments.len()).unwrap(), closure)
    }

    #[test]
    fn two_opt_untangles_a_crossing() {
        // lanes at 0, 2, 4, 6 all driven forwards: every link crosses back
        let segments: Vec<Segment> = (0..4).map(|y| h(2 * y, 0, 10)).collect();
        let mut tour = tour_of(
            &segments,
            &[(0, false), (1, false), (2, false), (3, false)],
            Closure::Open,
        );
        let neighbours = Neighbours::build(&segments, 8);
        let start = tour.cost;

        let touched = two_opt(&mut tour, &neighbours, 0);

        assert!(touched.is_some());
        assert!(tour.cost < start);
        assert_eq!(tour.cost, tour.full_cost());
    }

    #[test]
    fn or_opt_moves_a_straggler_home() {
        // segment 1 sits far away in the order but next to 0 in space
        let segments = vec![h(0, 0, 10), h(2, 0, 10), h(50, 0, 10), h(52, 0, 10)];
        let mut tour = tour_of(
            &segments,
            &[(0, false), (2, true), (3, false), (1, true)],
            Closure::Open,
        );
        let neighbours = Neighbours::build(&segments, 1);
        let start = tour.cost;

        let touched = or_opt(&mut tour, &neighbours, 1);

        assert!(touched.is_some());
        assert!(tour.cost < start);
        assert_eq!(tour.cost, tour.full_cost());
    }

    #[test]
    fn no_move_on_an_optimal_zigzag() {
        let segments: Vec<Segment> = (0..4).map(|y| h(2 * y, 0, 10)).collect();
        let visits = [(0, false), (1, true), (2, false), (3, true)];
        let mut tour = tour_of(&segments, &visits, Closure::Open);
        let neighbours = Neighbours::build(&segments, 8);

        for s in 0..4 {
            assert!(two_opt(&mut tour, &neighbours, s).is_none());
            assert!(or_opt(&mut tour, &neighbours, s).is_none());
        }
        assert_eq!(tour.cost, 6);
    }

    #[test]
    fn touched_holds_six() {
        let mut touched = Touched::default();
        (0..6).for_each(|id| touched.push(id));

        assert_eq!(touched.ids(), &[0, 1, 2, 3, 4, 5]);
    }
}
