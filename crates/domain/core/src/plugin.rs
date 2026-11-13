//! The steps of the pipeline, one trait each. A new approach is a new
//! implementation of one of these, registered under a label.
//!
//! Every step borrows the problem. Steps that rewrite routes take them by
//! value and return new ones, so no route is ever shared while it changes.

use std::time::{Duration, Instant};

use crate::error::SolveError;
use crate::order::{Closure, Order};
use crate::problem::Problem;
use crate::route::Route;
use crate::segment::Segment;
use crate::walk::Walk;

/// How long a step may run, and the seed for anything random it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    pub deadline: Instant,
    pub seed: u64,
}

impl Budget {
    pub fn new(time: Duration, seed: u64) -> Self {
        Self {
            deadline: Instant::now() + time,
            seed,
        }
    }

    pub fn expired(self) -> bool {
        Instant::now() >= self.deadline
    }

    pub fn remaining(self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }

    /// The same deadline with another seed, for a sub step.
    pub const fn with_seed(self, seed: u64) -> Self {
        Self { seed, ..self }
    }

    /// A deadline no later than `limit` from now, for a step that must leave
    /// time to the ones after it.
    pub fn capped(self, limit: Duration) -> Self {
        // a cap too far out to represent caps nothing
        let deadline = match Instant::now().checked_add(limit) {
            Some(cap) => self.deadline.min(cap),
            None => self.deadline,
        };

        Self { deadline, ..self }
    }
}

/// Cuts the region into segments that together mow every cell.
pub trait Decomposer: Send + Sync {
    /// A short name plus knobs, for job labels, e.g. `segments-h/gap=2`.
    fn label(&self) -> String;

    fn decompose(&self, problem: &Problem) -> Result<Vec<Segment>, SolveError>;
}

/// Puts segments in a short order, each driven one way or the other.
pub trait Orderer: Send + Sync {
    fn label(&self) -> String;

    /// An order of all of `segments`. Must return its best order so far when
    /// the budget runs out, not an error.
    fn order(&self, segments: &[Segment], closure: Closure, budget: Budget) -> Result<Order, SolveError>;
}

/// Shares the segments among the cutters directly: at most `problem.k()`
/// closed routes through the segments, before any repair. Routes may miss
/// cells; a [`Repairer`] runs after.
pub trait Assigner: Send + Sync {
    fn label(&self) -> String;

    fn assign(
        &self,
        problem: &Problem,
        segments: &[Segment],
        orderer: &dyn Orderer,
        budget: Budget,
    ) -> Result<Vec<Route>, SolveError>;
}

/// Cuts one open walk into at most `k` consecutive pieces, so that the
/// longest piece, closed, is as short as possible. Every position of the
/// walk lands in exactly one piece, so coverage is kept.
pub trait Splitter: Send + Sync {
    fn label(&self) -> String;

    fn split(&self, walk: &Walk, k: usize) -> Result<Vec<Walk>, SolveError>;
}

/// Cuts the region into bands, one single-cutter problem each, holding
/// roughly `shares` of the cells (shares need not sum to one). Empty bands
/// are left out, so fewer problems than shares can come back.
pub trait Partitioner: Send + Sync {
    fn label(&self) -> String;

    fn bands(&self, problem: &Problem, shares: &[f64]) -> Result<Vec<Problem>, SolveError>;
}

/// Makes a walk, or a set of routes together, sweep every region cell, e.g.
/// with detours to missed cells. Walk ends stay; routes stay closed.
pub trait Repairer: Send + Sync {
    fn label(&self) -> String;

    fn repair_walk(&self, problem: &Problem, walk: Walk) -> Result<Walk, SolveError>;

    /// A cell counts as swept if any of the routes sweeps it.
    fn repair(&self, problem: &Problem, routes: Vec<Route>) -> Result<Vec<Route>, SolveError>;
}

/// Shortens a walk or routes and keeps every cell that was swept swept.
/// Must return its best so far when the budget runs out, not an error.
pub trait Improver: Send + Sync {
    fn label(&self) -> String;

    fn improve_walk(&self, problem: &Problem, walk: Walk, budget: Budget) -> Result<Walk, SolveError>;

    /// Longest route first, since the longest is the objective.
    fn improve(
        &self,
        problem: &Problem,
        routes: Vec<Route>,
        budget: Budget,
    ) -> Result<Vec<Route>, SolveError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capped_never_extends_a_budget() {
        let budget = Budget::new(Duration::from_millis(10), 7);

        assert_eq!(budget.capped(Duration::from_secs(60)).deadline, budget.deadline);
        assert_eq!(budget.capped(Duration::MAX).deadline, budget.deadline);
        assert!(budget.capped(Duration::ZERO).expired());
        assert_eq!(budget.with_seed(9).seed, 9);
    }
}
