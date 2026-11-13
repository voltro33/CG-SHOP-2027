//! A saved order, kept in step with the tour through its journal: keeping or
//! undoing a kick costs as much as the kick wrote, not a whole copy.

use crate::tour::Tour;

pub(crate) struct Snapshot {
    seg_at: Vec<u32>,
    rev_at: Vec<bool>,
    pub(crate) cost: i64,
}

impl Snapshot {
    pub(crate) fn of(tour: &Tour) -> Self {
        Self {
            seg_at: tour.seg_at.clone(),
            rev_at: tour.rev_at.clone(),
            cost: tour.cost,
        }
    }

    /// Copies all of `other`, without allocating.
    pub(crate) fn copy_from(&mut self, other: &Self) {
        self.seg_at.copy_from_slice(&other.seg_at);
        self.rev_at.copy_from_slice(&other.rev_at);
        self.cost = other.cost;
    }

    /// Takes over what the tour wrote since its journal was cleared, and
    /// clears it.
    pub(crate) fn keep(&mut self, tour: &mut Tour) {
        if tour.journal.overflow {
            self.seg_at.copy_from_slice(&tour.seg_at);
            self.rev_at.copy_from_slice(&tour.rev_at);
        } else {
            for &p in &tour.journal.positions {
                self.seg_at[p as usize] = tour.seg_at[p as usize];
                self.rev_at[p as usize] = tour.rev_at[p as usize];
            }
        }

        self.cost = tour.cost;
        tour.journal.clear();
    }

    /// Puts the tour back to this order where it wrote since its journal was
    /// cleared, and clears it.
    pub(crate) fn undo(&self, tour: &mut Tour) {
        let positions = std::mem::take(&mut tour.journal.positions);
        let everything = tour.journal.overflow;

        if everything {
            for p in 0..self.seg_at.len() {
                tour.write(p, self.seg_at[p], self.rev_at[p]);
            }
        } else {
            for &p in &positions {
                tour.write(p as usize, self.seg_at[p as usize], self.rev_at[p as usize]);
            }
        }

        // writing logged the positions again, into the empty journal
        tour.journal.positions = positions;
        tour.journal.clear();
        tour.cost = self.cost;
    }

    /// Writes this order into the tour, all of it.
    pub(crate) fn restore(&self, tour: &mut Tour) {
        for p in 0..self.seg_at.len() {
            tour.write(p, self.seg_at[p], self.rev_at[p]);
        }

        tour.journal.clear();
        tour.cost = self.cost;
    }
}

#[cfg(test)]
mod tests {
    use mow_core::{Closure, Order, Segment};

    use super::*;

    fn tour(count: i32) -> Tour {
        let segments: Vec<Segment> = (0..count)
            .map(|y| Segment::horizontal(2 * y, 0, 10).unwrap())
            .collect();
        Tour::new(&segments, &Order::identity(count as usize), Closure::Closed)
    }

    #[test]
    fn undo_goes_back_and_keep_moves_on() {
        let mut tour = tour(8);
        let mut kept = Snapshot::of(&tour);
        let original = tour.seg_at.clone();

        tour.reverse(2, 3);
        tour.cost = tour.full_cost();
        kept.undo(&mut tour);
        assert_eq!(tour.seg_at, original);
        assert_eq!(tour.cost, tour.full_cost());

        tour.reverse(1, 2);
        tour.cost = tour.full_cost();
        kept.keep(&mut tour);
        tour.move_one(0, 5, true);
        kept.undo(&mut tour);
        assert_eq!(tour.seg_at[1], 2);
        assert_eq!(kept.cost, tour.cost);
        assert!(tour.journal.positions.is_empty());
    }

    #[test]
    fn overflowed_journals_fall_back_to_full_copies() {
        let mut tour = tour(3);
        let mut kept = Snapshot::of(&tour);
        let capacity = tour.journal.positions.capacity();

        for _ in 0..=capacity {
            tour.reverse(0, 1);
        }
        tour.cost = tour.full_cost();
        kept.keep(&mut tour);

        assert_eq!(kept.seg_at, tour.seg_at);
        assert_eq!(kept.rev_at, tour.rev_at);
        assert!(!tour.journal.overflow);
    }
}
