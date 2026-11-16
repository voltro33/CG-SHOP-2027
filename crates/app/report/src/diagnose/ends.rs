//! Where each lane of a tour ends, and what its ends cost.
//!
//! A lane has two ends: where the cutter leaves it after its last step, and
//! where it entered it before its first. An end is classified by the step
//! the cutter did not take there, one more step along the lane past its
//! last position (or, at the start, one step backwards before its first):
//!
//! - edge: that step would sweep no region cell, the lane stops at the
//!   lawn's edge
//! - inner: it would sweep region cells, but only ones this tour sweeps
//!   itself (a headland, a lane of its own, a shift between lanes)
//! - border: it would sweep region cells this tour never sweeps, so another
//!   tour sweeps them: the lane stops where two cutters' areas meet
//!
//! The waste of an end is that of the lane's steps within a cutter span of
//! it (each step goes to the nearer end, as for overshoot at lane ends),
//! plus half of each turn: the dead steps between two consecutive lanes,
//! when fewer than four spans (longer ones are travel). A step's waste is
//! its length less what it mows new and less what it re-mows of another
//! tour's cells, which is counted as cross-tour overlap instead.

use mow_core::Point;

use super::lanes::{step_at, straight_runs};

/// The kinds of lane ends, in the order they are reported.
pub const END_KINDS: [&str; 3] = ["edge", "inner", "border"];

/// What the step a lane did not take would have swept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndKind {
    Edge = 0,
    Inner = 1,
    Border = 2,
}

/// Lane ends per [`END_KINDS`]: how many, and the waste they cost.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LaneEnds {
    pub counts: [u64; 3],
    pub waste: [f64; 3],
}

impl LaneEnds {
    pub fn plus(&self, other: &Self) -> Self {
        Self {
            counts: std::array::from_fn(|i| self.counts[i] + other.counts[i]),
            waste: std::array::from_fn(|i| self.waste[i] + other.waste[i]),
        }
    }

    fn book(&mut self, kind: EndKind, waste: f64) {
        self.waste[kind as usize] += waste;
    }
}

/// The lane ends of one tour. `new[i]` and `waste[i]` belong to the step
/// from position `i` to the next one, the last step closing the tour.
/// `classify(at, direction)` tells what one more step from `at` by
/// `direction` would sweep.
pub fn lane_ends(
    positions: &[Point],
    new: &[usize],
    waste: &[f64],
    span: i64,
    classify: impl Fn(Point, (i32, i32)) -> EndKind,
) -> LaneEnds {
    let count = positions.len();
    let lanes: Vec<(usize, usize)> = straight_runs(positions)
        .into_iter()
        .filter(|&(first, length)| (first..first + length).any(|i| new[i] > 0))
        .collect();

    let kinds: Vec<(EndKind, EndKind)> = lanes
        .iter()
        .map(|&(first, length)| {
            let (dx, dy) = step_at(positions, first);
            let start = classify(positions[first], (-dx, -dy));
            let end = classify(positions[(first + length) % count], (dx, dy));

            (start, end)
        })
        .collect();

    let mut ends = LaneEnds::default();
    for (&lane, &(start, end)) in lanes.iter().zip(&kinds) {
        ends.counts[start as usize] += 1;
        ends.counts[end as usize] += 1;

        let (near_start, near_end) = window_waste(lane, waste, span);
        ends.book(start, near_start);
        ends.book(end, near_end);
    }

    for (j, &(first, length)) in lanes.iter().enumerate() {
        let next = (j + 1) % lanes.len();
        let half = turn_waste(first + length, lanes[next].0, waste, span) / 2.0;

        ends.book(kinds[j].1, half);
        ends.book(kinds[next].0, half);
    }

    ends
}

/// The waste of a lane's steps within a span of its start, and of its end.
fn window_waste((first, length): (usize, usize), waste: &[f64], span: i64) -> (f64, f64) {
    let mut near = (0.0, 0.0);

    for (offset, &step) in waste[first..first + length].iter().enumerate() {
        let from_start = offset as i64;
        let from_end = (length - 1) as i64 - from_start;
        if from_start.min(from_end) >= span {
            continue;
        }

        match from_start < from_end {
            true => near.0 += step,
            false => near.1 += step,
        }
    }

    near
}

/// The waste of the dead steps from step `from` up to step `to` (cyclic),
/// a turn between two lanes; nothing when they are travel instead.
fn turn_waste(from: usize, to: usize, waste: &[f64], span: i64) -> f64 {
    let count = waste.len();
    let gap = (to + count - from % count) % count;
    if gap as i64 >= 4 * span {
        return 0.0;
    }

    (from..from + gap).map(|i| waste[i % count]).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Right along y = 0 from x = 0 to 4, up one, back left, down one.
    fn loop_positions() -> Vec<Point> {
        let mut positions: Vec<Point> = (0..4).map(|x| Point::new(x, 0)).collect();
        positions.push(Point::new(4, 0));
        positions.extend((1..=4).rev().map(|x| Point::new(x, 1)));
        positions.push(Point::new(0, 1));

        positions
    }

    #[test]
    fn classifies_both_ends_of_each_lane() {
        let positions = loop_positions();
        let new = [1, 1, 1, 1, 0, 1, 1, 1, 1, 0];
        let waste = [0.0, 0.0, 0.0, 0.5, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];

        // past x = 4 is another tour's lawn, past x = 0 the lawn's edge
        let classify = |at: Point, (dx, _): (i32, i32)| match at.x + dx {
            x if x > 4 => EndKind::Border,
            x if x < 0 => EndKind::Edge,
            _ => EndKind::Inner,
        };
        let ends = lane_ends(&positions, &new, &waste, 1, classify);

        // the two vertical one-step runs are dead: two lanes, four ends
        assert_eq!(ends.counts, [2, 0, 2]);
        // border: the lower lane's last step and the two halves of the turn
        // up; edge: the two halves of the turn down
        assert_eq!(ends.waste, [1.0, 0.0, 1.5]);
    }

    #[test]
    fn long_dead_stretches_are_travel_not_turns() {
        assert_eq!(turn_waste(2, 6, &[1.0; 8], 1), 0.0);
        assert_eq!(turn_waste(2, 5, &[1.0; 8], 1), 3.0);
        assert_eq!(turn_waste(7, 1, &[1.0; 8], 1), 2.0);
    }

    #[test]
    fn steps_go_to_the_nearer_end() {
        let waste = [1.0, 2.0, 4.0, 8.0, 16.0];

        assert_eq!(window_waste((0, 5), &waste, 2), (3.0, 24.0));
        assert_eq!(window_waste((0, 3), &waste, 2), (1.0, 6.0));
    }
}
