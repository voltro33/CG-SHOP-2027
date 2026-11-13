//! The order being improved, as a cycle of stops. An open order gets one
//! extra dummy stop whose links cost nothing, so moves never special case the
//! two ends of a path.
//!
//! Positions and segment ids are kept in maps both ways, plus the entry and
//! exit point of every position, so a move reads only the few points it
//! needs. Every position a move writes is logged in a journal, which lets the
//! search undo or keep a kick without copying the whole order.

use mow_core::{Closure, Order, Point, Segment, Visit};

/// The ends of the dummy stop. Links to it cost nothing.
const NOWHERE: Point = Point::new(i32::MIN, i32::MIN);

pub(crate) struct Tour {
    // ends of every stop by id, the dummy last
    starts: Vec<Point>,
    finishes: Vec<Point>,

    // by position
    pub(crate) seg_at: Vec<u32>,
    pub(crate) rev_at: Vec<bool>,
    pub(crate) ent: Vec<Point>,
    pub(crate) ext: Vec<Point>,

    // by id
    pub(crate) pos_of: Vec<u32>,

    /// Link length of the current order.
    pub(crate) cost: i64,

    pub(crate) journal: Journal,
}

/// The positions written since the journal was last cleared. When more were
/// written than it holds, it only remembers that it overflowed.
pub(crate) struct Journal {
    pub(crate) positions: Vec<u32>,
    pub(crate) overflow: bool,
}

impl Journal {
    fn push(&mut self, position: u32) {
        if self.positions.len() < self.positions.capacity() {
            self.positions.push(position);
        } else {
            self.overflow = true;
        }
    }

    pub(crate) fn clear(&mut self) {
        self.positions.clear();
        self.overflow = false;
    }
}

impl Tour {
    /// The order as a cycle, with a dummy stop after the last visit when open.
    pub(crate) fn new(segments: &[Segment], order: &Order, closure: Closure) -> Self {
        let mut starts: Vec<Point> = segments.iter().map(|s| s.start()).collect();
        let mut finishes: Vec<Point> = segments.iter().map(|s| s.end()).collect();
        let mut visits = order.visits().to_vec();

        if closure == Closure::Open {
            starts.push(NOWHERE);
            finishes.push(NOWHERE);
            visits.push(Visit {
                segment: segments.len() as u32,
                reversed: false,
            });
        }

        let m = visits.len();
        let mut tour = Self {
            starts,
            finishes,
            seg_at: vec![0; m],
            rev_at: vec![false; m],
            ent: vec![NOWHERE; m],
            ext: vec![NOWHERE; m],
            pos_of: vec![0; m],
            cost: 0,
            journal: Journal {
                positions: Vec::with_capacity(2 * m + 64),
                overflow: false,
            },
        };

        for (p, v) in visits.iter().enumerate() {
            tour.write(p, v.segment, v.reversed);
        }
        tour.cost = tour.full_cost();
        tour.journal.clear();

        tour
    }

    /// Stops in the cycle, the dummy included.
    pub(crate) fn len(&self) -> usize {
        self.seg_at.len()
    }

    pub(crate) fn next(&self, p: usize) -> usize {
        if p + 1 == self.len() {
            0
        } else {
            p + 1
        }
    }

    pub(crate) fn prev(&self, p: usize) -> usize {
        if p == 0 {
            self.len() - 1
        } else {
            p - 1
        }
    }

    /// Puts stop `segment` at position `p`, driven backwards if `reversed`.
    pub(crate) fn write(&mut self, p: usize, segment: u32, reversed: bool) {
        let (a, b) = (self.starts[segment as usize], self.finishes[segment as usize]);
        let (entry, exit) = if reversed { (b, a) } else { (a, b) };

        self.seg_at[p] = segment;
        self.rev_at[p] = reversed;
        self.ent[p] = entry;
        self.ext[p] = exit;
        self.pos_of[segment as usize] = p as u32;
        self.journal.push(p as u32);
    }

    /// The link from the exit of position `p` to the entry of the next one.
    pub(crate) fn link(&self, p: usize) -> i64 {
        dist(self.ext[p], self.ent[self.next(p)])
    }

    pub(crate) fn full_cost(&self) -> i64 {
        (0..self.len()).map(|p| self.link(p)).sum()
    }

    /// Reverses the `len` positions from `first` on, cyclically, and flips
    /// each stop. On a cycle that is the same as reversing the rest, so the
    /// shorter side is the one rewritten.
    pub(crate) fn reverse(&mut self, first: usize, len: usize) {
        let m = self.len();
        let (mut a, len) = if 2 * len > m {
            ((first + len) % m, m - len)
        } else {
            (first, len)
        };
        let mut b = (a + len + m - 1) % m;

        for _ in 0..len / 2 {
            let (sa, ra) = (self.seg_at[a], self.rev_at[a]);
            let (sb, rb) = (self.seg_at[b], self.rev_at[b]);
            self.write(a, sb, !rb);
            self.write(b, sa, !ra);

            a = self.next(a);
            b = self.prev(b);
        }

        if len % 2 == 1 {
            self.write(a, self.seg_at[a], !self.rev_at[a]);
        }
    }

    /// Moves the stop at `p` to just after position `slot`, flipped if `flip`.
    /// Shifts whichever side of the cycle between them is shorter.
    pub(crate) fn move_one(&mut self, p: usize, slot: usize, flip: bool) {
        let m = self.len();
        let (segment, reversed) = (self.seg_at[p], self.rev_at[p] ^ flip);
        let forward = (slot + m - p) % m;

        let mut at = p;
        if forward <= m - 1 - forward {
            for _ in 0..forward {
                let from = self.next(at);
                self.write(at, self.seg_at[from], self.rev_at[from]);
                at = from;
            }
        } else {
            for _ in 0..m - 1 - forward {
                let from = self.prev(at);
                self.write(at, self.seg_at[from], self.rev_at[from]);
                at = from;
            }
        }

        self.write(at, segment, reversed);
    }

    /// The visits, starting after the dummy stop if there is one, else at
    /// position 0.
    pub(crate) fn order(&self, segment_count: usize) -> Vec<Visit> {
        let m = self.len();
        let start = if m > segment_count {
            self.next(self.pos_of[segment_count] as usize)
        } else {
            0
        };

        (0..m)
            .map(|i| (start + i) % m)
            .filter(|&p| (self.seg_at[p] as usize) < segment_count)
            .map(|p| Visit {
                segment: self.seg_at[p],
                reversed: self.rev_at[p],
            })
            .collect()
    }
}

/// Taxicab distance, zero to and from the dummy stop.
pub(crate) fn dist(a: Point, b: Point) -> i64 {
    if a == NOWHERE || b == NOWHERE {
        return 0;
    }

    a.l1(b) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lanes(count: i32) -> Vec<Segment> {
        (0..count)
            .map(|y| Segment::horizontal(2 * y, 0, 10).unwrap())
            .collect()
    }

    fn ids(tour: &Tour) -> Vec<(u32, bool)> {
        tour.seg_at
            .iter()
            .copied()
            .zip(tour.rev_at.iter().copied())
            .collect()
    }

    fn consistent(tour: &Tour) {
        for p in 0..tour.len() {
            assert_eq!(tour.pos_of[tour.seg_at[p] as usize] as usize, p);
        }
    }

    #[test]
    fn open_orders_cost_as_mow_core_says() {
        let segments = lanes(4);
        let order = Order::identity(4);

        let open = Tour::new(&segments, &order, Closure::Open);
        let closed = Tour::new(&segments, &order, Closure::Closed);

        assert_eq!(open.len(), 5);
        assert_eq!(open.cost as u64, order.link_length(&segments, Closure::Open));
        assert_eq!(closed.cost as u64, order.link_length(&segments, Closure::Closed));
    }

    #[test]
    fn reverse_flips_and_takes_the_shorter_side() {
        let segments = lanes(5);
        let mut tour = Tour::new(&segments, &Order::identity(5), Closure::Closed);

        tour.reverse(1, 2);
        assert_eq!(
            ids(&tour),
            vec![(0, false), (2, true), (1, true), (3, false), (4, false)]
        );

        // reversing 4 of 5 rewrites the one left instead: the same cycle
        tour.journal.clear();
        tour.reverse(1, 4);
        assert_eq!(tour.journal.positions, vec![0]);
        assert_eq!(ids(&tour)[0], (0, true));
        consistent(&tour);
    }

    #[test]
    fn move_one_shifts_either_way() {
        let segments = lanes(6);
        let mut tour = Tour::new(&segments, &Order::identity(6), Closure::Closed);

        tour.move_one(1, 3, true);
        let order: Vec<u32> = tour.seg_at.clone();
        assert_eq!(order, vec![0, 2, 3, 1, 4, 5]);
        assert!(tour.rev_at[3]);

        tour.move_one(4, 0, false);
        let cycle = tour.order(6);
        let next_of = |s: u32| {
            let i = cycle.iter().position(|v| v.segment == s).unwrap();
            cycle[(i + 1) % 6].segment
        };
        assert_eq!(next_of(0), 4);
        assert_eq!(next_of(4), 2);
        consistent(&tour);
    }

    #[test]
    fn open_orders_read_back_after_the_dummy() {
        let segments = lanes(3);
        let mut tour = Tour::new(&segments, &Order::identity(3), Closure::Open);

        tour.reverse(2, 2);
        let visits = tour.order(3);

        assert_eq!(visits.len(), 3);
        assert_eq!((visits[0].segment, visits[0].reversed), (2, true));
        assert_eq!(dist(NOWHERE, Point::new(3, 4)), 0);
    }

    #[test]
    fn the_journal_overflows_without_growing() {
        let segments = lanes(3);
        let mut tour = Tour::new(&segments, &Order::identity(3), Closure::Closed);
        let capacity = tour.journal.positions.capacity();

        for _ in 0..=capacity {
            tour.reverse(0, 1);
        }

        assert!(tour.journal.overflow);
        assert_eq!(tour.journal.positions.capacity(), capacity);
    }
}
