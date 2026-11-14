//! A small closed order over stops: the kept chains of a tour and the new
//! segments that re-cover the hole. Every stop can be driven either way, so
//! a chain of the old tour may also come back reversed (a closed walk mows
//! the same driven backwards).
//!
//! Cheapest insertion, then 2-opt and Or-opt until no move shortens the
//! links. The orders are tens of stops, so every move is checked against
//! every position; `mow-order`'s neighbour lists pay off only for thousands.

use mow_core::Point;

/// The ends of a stop driven forwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stop {
    pub entry: Point,
    pub exit: Point,
}

/// One stop of the order, and whether it is driven from `exit` to `entry`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    pub stop: usize,
    pub reversed: bool,
}

/// Descents stop after this many passes even if moves still help.
const MAX_PASSES: usize = 40;

#[derive(Debug, Clone)]
pub struct Cycle {
    stops: Vec<Stop>,
    order: Vec<Step>,
}

impl Cycle {
    /// The stops `0..stops.len()` in their own order, all forwards.
    pub fn new(stops: Vec<Stop>) -> Self {
        let order = (0..stops.len())
            .map(|stop| Step {
                stop,
                reversed: false,
            })
            .collect();

        Self { stops, order }
    }

    pub fn order(&self) -> &[Step] {
        &self.order
    }

    /// `(entry, exit)` of a step as driven.
    pub fn ends(&self, step: Step) -> (Point, Point) {
        let stop = self.stops[step.stop];
        match step.reversed {
            true => (stop.exit, stop.entry),
            false => (stop.entry, stop.exit),
        }
    }

    /// The length of the links, the way home included.
    pub fn link_length(&self) -> u64 {
        let n = self.order.len();
        (0..n).map(|i| self.link(i, (i + 1) % n)).sum()
    }

    /// Adds a stop where it lengthens the links least, either way round.
    pub fn insert(&mut self, stop: Stop) {
        self.stops.push(stop);
        let index = self.stops.len() - 1;

        let (_, at, reversed) = self.best_place(index);
        self.order.insert(
            at,
            Step {
                stop: index,
                reversed,
            },
        );
    }

    /// Where stop `index`, not in the order, lengthens the links least:
    /// `(added length, position to insert at, reversed)`.
    fn best_place(&self, index: usize) -> (i64, usize, bool) {
        let n = self.order.len();
        let mut best = (i64::MAX, 0, false);

        for i in 0..n {
            let (_, out) = self.ends(self.order[i]);
            let (back, _) = self.ends(self.order[(i + 1) % n]);
            let old = out.l1(back) as i64;

            for reversed in [false, true] {
                let (a, b) = self.ends(Step {
                    stop: index,
                    reversed,
                });
                let cost = (out.l1(a) + b.l1(back)) as i64 - old;
                if cost < best.0 {
                    best = (cost, i + 1, reversed);
                }
            }
        }

        best
    }

    /// 2-opt and Or-opt until a pass finds nothing. Returns the links saved.
    pub fn improve(&mut self) -> u64 {
        let before = self.link_length();

        for _ in 0..MAX_PASSES {
            let moved = self.two_opt_pass() | self.or_opt_pass();
            if !moved {
                break;
            }
        }

        before - self.link_length()
    }

    /// Rotates the order so that `stop` comes first, keeping its direction.
    pub fn rotate_to(&mut self, stop: usize) {
        if let Some(at) = self.order.iter().position(|s| s.stop == stop) {
            self.order.rotate_left(at);
        }
    }

    fn link(&self, from: usize, to: usize) -> u64 {
        let (_, out) = self.ends(self.order[from]);
        let (entry, _) = self.ends(self.order[to]);
        out.l1(entry)
    }

    /// Reverses the block `i..=j` wherever that shortens the links.
    fn two_opt_pass(&mut self) -> bool {
        let n = self.order.len();
        let mut moved = false;

        for i in 0..n {
            for j in i..n {
                if i == 0 && j + 1 == n {
                    continue;
                }

                let (prev, next) = ((i + n - 1) % n, (j + 1) % n);
                let (_, prev_out) = self.ends(self.order[prev]);
                let (first_in, _) = self.ends(self.order[i]);
                let (_, last_out) = self.ends(self.order[j]);
                let (next_in, _) = self.ends(self.order[next]);

                let old = prev_out.l1(first_in) + last_out.l1(next_in);
                let new = prev_out.l1(last_out) + first_in.l1(next_in);
                if new < old {
                    self.reverse(i, j);
                    moved = true;
                }
            }
        }

        moved
    }

    fn reverse(&mut self, i: usize, j: usize) {
        self.order[i..=j].reverse();
        for step in &mut self.order[i..=j] {
            step.reversed = !step.reversed;
        }
    }

    /// Moves single stops to the cheapest other place, either way round.
    fn or_opt_pass(&mut self) -> bool {
        let mut moved = false;

        let n = self.order.len();
        if n < 3 {
            return false;
        }

        for i in 0..n {
            let (prev, next) = ((i + n - 1) % n, (i + 1) % n);
            let (_, prev_out) = self.ends(self.order[prev]);
            let (entry, exit) = self.ends(self.order[i]);
            let (next_in, _) = self.ends(self.order[next]);
            let gain = (prev_out.l1(entry) + exit.l1(next_in)) as i64 - prev_out.l1(next_in) as i64;

            let step = self.order.remove(i);
            let (cost, at, reversed) = self.best_place(step.stop);

            if cost < gain {
                self.order.insert(
                    at,
                    Step {
                        stop: step.stop,
                        reversed,
                    },
                );
                moved = true;
            } else {
                self.order.insert(i, step);
            }
        }

        moved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stop(a: (i32, i32), b: (i32, i32)) -> Stop {
        Stop {
            entry: a.into(),
            exit: b.into(),
        }
    }

    #[test]
    fn insertion_picks_the_cheapest_gap_and_direction() {
        let mut cycle = Cycle::new(vec![stop((0, 0), (10, 0)), stop((0, 4), (10, 4))]);
        assert_eq!(cycle.link_length(), 28);

        // a lane between the two, best driven right to left after the first
        cycle.insert(stop((0, 2), (10, 2)));

        assert_eq!(
            cycle.order()[1],
            Step {
                stop: 2,
                reversed: true
            }
        );
        assert_eq!(cycle.link_length(), 2 + 2 + 14);
    }

    #[test]
    fn improve_untangles_a_crossed_order() {
        // four lanes stacked, visited 0, 2, 1, 3, all left to right
        let lanes: Vec<Stop> = [0, 2, 1, 3].iter().map(|&y| stop((0, y), (10, y))).collect();
        let mut cycle = Cycle::new(lanes);
        let before = cycle.link_length();

        let saved = cycle.improve();

        assert!(saved > 0);
        assert_eq!(cycle.link_length(), before - saved);
        // a snake over four lanes: three short links and the way home
        assert_eq!(cycle.link_length(), 3 + 3);
    }

    #[test]
    fn rotation_keeps_the_links() {
        let mut cycle = Cycle::new(vec![
            stop((0, 0), (1, 0)),
            stop((5, 5), (6, 5)),
            stop((2, 2), (2, 3)),
        ]);
        let before = cycle.link_length();

        cycle.rotate_to(2);

        assert_eq!(cycle.order()[0].stop, 2);
        assert_eq!(cycle.link_length(), before);
    }
}
