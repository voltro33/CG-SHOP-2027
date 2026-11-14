//! One ruin and recreate move on [`Tours`]: take the pieces of a tour out
//! of a window, sweep the cells left bare with new stops, reconnect,
//! repair what is still missed, cut corners, and keep it if the cost rises
//! by no more than the threshold the annealing drew.
//!
//! Coverage is never recomputed in full: the counts lose the pieces, gain
//! the new walks, and only the cells around the pieces are asked about.

use std::time::Instant;

use mow_core::Point;
use rand::Rng;

use crate::chains::{cut, Chain, Cut};
use crate::cycle::Stop;
use crate::destroy::Destroy;
use crate::edit::Edit;
use crate::error::LnsError;
use crate::recreate::{anchors_for, Recreate};
use crate::tours::{cost, dense_length, is_unit_cycle, Tours};

/// Rounds of anchors for cells the new stops still miss.
const REPAIR_ROUNDS: usize = 3;

/// How much corner cutting may still save, in cutter spans, when a plan is
/// judged before its walks are built.
const PRUNE_SLACK_SPANS: f64 = 1.0;

/// Chance that a move works on the longest tour rather than any.
const LONGEST_SHARE: f64 = 0.75;

/// What became of a move.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tried {
    /// The window missed the tour, or the move could not be built.
    Skipped,
    Rejected,
    /// Kept; the cost changed by `delta`.
    Kept {
        delta: f64,
    },
}

impl Tours<'_> {
    /// One move. `threshold` is the most the cost may rise.
    pub fn try_move(
        &mut self,
        destroy: Destroy,
        recreate: Recreate,
        threshold: f64,
        deadline: Instant,
        rng: &mut impl Rng,
    ) -> Result<Tried, LnsError> {
        let tour = self.pick_tour(destroy, rng);
        if self.tours[tour].len() < 2 {
            return Ok(Tried::Skipped);
        }

        let window = destroy.window(&self.tours[tour], self.span, &self.counts, &mut self.scratch, rng);
        let Some(Cut { removed, chains }) = cut(&self.tours[tour], window) else {
            return Ok(Tried::Skipped);
        };

        self.counts.remove_positions(removed.iter().copied())?;
        let bare = self.counts.unswept_around(removed.iter().copied());
        let offset = rng.gen_range(0..self.span.max(1));
        let stops = recreate.stops(self.problem, &bare, offset)?;

        let center = Point::new((window.x0 + window.x1) / 2, (window.y0 + window.y1) / 2);
        let mut edits = self.plan(tour, chains, stops, destroy == Destroy::Transfer, center);

        let before = cost(&self.lengths());
        let slack = PRUNE_SLACK_SPANS * f64::from(self.span);
        if self.planned_cost(&edits) - before > threshold + slack || self.degenerate(&edits) {
            self.counts.add_positions(removed)?;
            return Ok(Tried::Rejected);
        }

        if !self.realize(&mut edits, &removed, deadline)? {
            self.counts.add_positions(removed)?;
            return Ok(Tried::Rejected);
        }

        let delta = self.cost_with(&edits, |e| e.length()) - before;
        if delta > threshold {
            self.undo(&edits, removed)?;
            return Ok(Tried::Rejected);
        }

        self.commit(&edits)?;
        Ok(Tried::Kept { delta })
    }

    fn pick_tour(&self, destroy: Destroy, rng: &mut impl Rng) -> usize {
        let n = self.tours.len();
        if destroy == Destroy::Transfer || n == 1 || rng.gen_bool(LONGEST_SHARE) {
            return self.longest();
        }

        rng.gen_range(0..n)
    }

    /// The edits of a move: the cut tour with the new stops, or for a
    /// transfer the cut tour closed up and the stops in the nearest other
    /// tour, opened next to its position nearest the window. The last edit
    /// takes the repair anchors.
    fn plan(
        &self,
        tour: usize,
        chains: Vec<Chain>,
        stops: Vec<Stop>,
        transfer: bool,
        center: Point,
    ) -> Vec<Edit> {
        let mut cut = Edit::new(tour, &self.tours[tour], chains);
        let receiver = match transfer {
            true => self.nearest_other(tour, center),
            false => None,
        };

        let Some((other, at)) = receiver.filter(|_| !stops.is_empty()) else {
            stops.into_iter().for_each(|s| cut.insert(s));
            cut.improve();
            return vec![cut];
        };

        cut.improve();
        let positions = &self.tours[other];
        let whole = Chain {
            first: (at + 1) % positions.len(),
            len: positions.len(),
        };
        let mut open = Edit::new(other, positions, vec![whole]);
        stops.into_iter().for_each(|s| open.insert(s));
        open.improve();

        vec![cut, open]
    }

    /// Builds the walks, adds them to the counts and repairs: true once
    /// every cell around `removed` is swept, with corners cut. On false the
    /// counts are back to holding no walk.
    fn realize(
        &mut self,
        edits: &mut [Edit],
        removed: &[Point],
        deadline: Instant,
    ) -> Result<bool, LnsError> {
        build_all(edits)?;
        apply_all(edits, &mut self.counts)?;

        for round in 0..=REPAIR_ROUNDS {
            if self.counts.all_swept_around(removed.iter().copied()) {
                break;
            }

            unapply_all(edits, &mut self.counts)?;
            if round == REPAIR_ROUNDS {
                return Ok(false);
            }

            let missing = self.counts.unswept_around(removed.iter().copied());
            let target = edits.len() - 1;
            let near: Vec<Point> = edits[target].stop_ends().collect();
            for anchor in anchors_for(self.problem, &missing, &near) {
                edits[target].insert(Stop {
                    entry: anchor,
                    exit: anchor,
                });
            }
            edits[target].improve();

            build_all(edits)?;
            apply_all(edits, &mut self.counts)?;
        }

        for edit in edits.iter_mut() {
            edit.prune(&mut self.counts, deadline)?;
        }

        Ok(true)
    }

    fn undo(&mut self, edits: &[Edit], removed: Vec<Point>) -> Result<(), LnsError> {
        unapply_all(edits, &mut self.counts)?;
        self.counts.add_positions(removed)?;
        Ok(())
    }

    /// Writes the new tours. A tour that is not a unit step cycle would be
    /// a bug in the walks; it is refused, never kept.
    fn commit(&mut self, edits: &[Edit]) -> Result<(), LnsError> {
        let dense: Vec<Vec<Point>> = edits.iter().map(|e| e.dense(&self.tours[e.tour])).collect();

        for (edit, tour) in edits.iter().zip(&dense) {
            if !is_unit_cycle(tour) || dense_length(tour) != edit.length() {
                return Err(LnsError::BrokenTour(edit.tour));
            }
        }

        for (edit, tour) in edits.iter().zip(dense) {
            self.tours[edit.tour] = tour;
        }

        Ok(())
    }

    fn planned_cost(&self, edits: &[Edit]) -> f64 {
        self.cost_with(edits, |e| e.planned_length())
    }

    /// The cost with the edited tours at the lengths `length` gives.
    fn cost_with(&self, edits: &[Edit], length: impl Fn(&Edit) -> u64) -> f64 {
        let mut lengths = self.lengths();
        for edit in edits {
            lengths[edit.tour] = length(edit);
        }

        cost(&lengths)
    }

    /// A tour that would shrink to the single point it stands on: its walk
    /// would join a chain to itself, which the counts cannot follow.
    fn degenerate(&self, edits: &[Edit]) -> bool {
        edits.iter().any(|e| e.planned_length() == 0)
    }
}

fn build_all(edits: &mut [Edit]) -> Result<(), LnsError> {
    for edit in edits.iter_mut() {
        edit.build()?;
    }

    Ok(())
}

fn apply_all(edits: &[Edit], counts: &mut mow_coverage::VisitCounts) -> Result<(), LnsError> {
    edits.iter().try_for_each(|e| e.apply(counts))
}

fn unapply_all(edits: &[Edit], counts: &mut mow_coverage::VisitCounts) -> Result<(), LnsError> {
    edits.iter().try_for_each(|e| e.unapply(counts))
}
