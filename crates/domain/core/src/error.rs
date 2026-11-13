//! Errors are values. One type per layer, so a caller knows where it came from.

use thiserror::Error;

use crate::geom::Point;

/// A domain value that would break an invariant.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModelError {
    #[error("the region has no cells")]
    EmptyRegion,

    #[error("the cutter has no cells")]
    EmptyCutter,

    #[error("the instance asks for {0} cutters, only 1 to 3 are allowed")]
    CutterCount(usize),

    #[error("a route needs at least one corner")]
    EmptyRoute,

    #[error("corners {0:?} and {1:?} are not axis parallel")]
    NotAxisParallel(Point, Point),

    #[error("the grid box {width} x {height} is too large")]
    GridTooLarge { width: u64, height: u64 },

    #[error("the {0} is so far out that a coordinate would overflow")]
    OutOfRange(&'static str),

    #[error("segment {from}..{to} runs backwards")]
    BackwardSegment { from: i32, to: i32 },

    #[error("the order has {visits} visits for {segments} segments, or repeats one")]
    BadOrder { visits: usize, segments: usize },
}

/// A step of the pipeline could not finish.
#[derive(Debug, Error)]
pub enum SolveError {
    #[error(transparent)]
    Model(#[from] ModelError),

    #[error("{step} ran out of time")]
    OutOfTime { step: &'static str },

    #[error("{step} cannot handle this instance: {reason}")]
    Unsupported { step: &'static str, reason: String },
}

/// An adapter failed to talk to the outside world.
#[derive(Debug, Error)]
pub enum PortError {
    #[error(transparent)]
    Model(#[from] ModelError),

    #[error("{0} not found")]
    NotFound(String),

    #[error("{context}: {message}")]
    External { context: String, message: String },
}
