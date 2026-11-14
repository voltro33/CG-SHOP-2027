//! What can go wrong in a search: bad input, or a coverage count that
//! disagrees with the tours (a bug, never a rejected move).

use thiserror::Error;

use mow_core::{ModelError, SolveError};
use mow_coverage::CoverageError;

#[derive(Debug, Error)]
pub enum LnsError {
    #[error(transparent)]
    Model(#[from] ModelError),

    #[error(transparent)]
    Coverage(#[from] CoverageError),

    #[error(transparent)]
    Solve(#[from] SolveError),

    #[error("there are no tours to search from")]
    NoTours,

    #[error("the tours to start from miss {0} cells")]
    NotCovered(usize),

    #[error("a move broke tour {0}: not a closed walk of unit steps")]
    BrokenTour(usize),

    #[error("state file {path}: {reason}")]
    State { path: String, reason: String },
}
