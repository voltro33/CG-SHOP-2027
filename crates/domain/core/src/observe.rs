//! Watching a pipeline run without changing it. The visualizer is one
//! observer; a progress log could be another.
//!
//! Events borrow what they show, so observing copies nothing. A pipeline
//! that is not watched passes [`NoObserver`] and pays one empty call per step.

use crate::order::Order;
use crate::problem::Problem;
use crate::route::Route;
use crate::segment::Segment;
use crate::solution::Solution;
use crate::walk::Walk;

#[derive(Debug, Clone, Copy)]
pub enum Event<'a> {
    /// A run starts on this problem.
    Started { problem: &'a Problem, label: &'a str },

    /// A decomposer produced these segments.
    Segments {
        problem: &'a Problem,
        segments: &'a [Segment],
    },

    /// An orderer put them in this order.
    Ordered {
        problem: &'a Problem,
        segments: &'a [Segment],
        order: &'a Order,
    },

    /// The open walk after a named step, e.g. `walk`, `repair`, `improve`.
    Walk {
        problem: &'a Problem,
        step: &'a str,
        walk: &'a Walk,
    },

    /// Routes after a named step, e.g. `assign`, `repair`, `improve`.
    Routes {
        problem: &'a Problem,
        step: &'a str,
        routes: &'a [Route],
    },

    /// The run produced this solution.
    Finished {
        problem: &'a Problem,
        solution: &'a Solution,
    },
}

pub trait Observer: Send + Sync {
    fn observe(&self, event: &Event<'_>);
}

/// Watches nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoObserver;

impl Observer for NoObserver {
    fn observe(&self, _event: &Event<'_>) {}
}
