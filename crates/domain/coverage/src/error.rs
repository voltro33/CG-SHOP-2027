//! What can go wrong while counting visits.

use mow_core::{ModelError, Point};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CoverageError {
    #[error(transparent)]
    Model(#[from] ModelError),

    #[error("the cutter has {0} cells, more than the cover counts can hold")]
    CutterTooLarge(usize),

    #[error("the anchor {0:?} is visited more than 65535 times")]
    VisitOverflow(Point),

    #[error("the anchor {0:?} is removed but not visited")]
    NotVisited(Point),
}
