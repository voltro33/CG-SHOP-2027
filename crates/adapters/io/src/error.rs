//! Errors are values: a bad file is reported, never a panic.

use std::path::PathBuf;

use mow_core::{ModelError, Point};
use thiserror::Error;

/// Why a ring (an outer boundary, a hole or the cutter) is rejected.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RingError {
    #[error("{x} x coordinates but {y} y coordinates")]
    LengthMismatch { x: usize, y: usize },

    #[error("{0} points, a ring needs at least 4")]
    TooFewPoints(usize),

    #[error("the edge from {0:?} to {1:?} is not orthogonal")]
    NotOrthogonal(Point, Point),

    #[error("all points lie on one line, the ring encloses no cell")]
    Degenerate,
}

/// Why a tour of a solution file is rejected.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TourError {
    #[error("{x} x coordinates but {y} y coordinates")]
    LengthMismatch { x: usize, y: usize },

    #[error("the point {0:?} at index {1} is repeated right after it")]
    RepeatedPoint(Point, usize),

    #[error(transparent)]
    Model(#[from] ModelError),
}

/// Anything that can go wrong reading or writing the official files.
#[derive(Debug, Error)]
pub enum IoError {
    #[error("cannot read {path}: {source}")]
    Read { path: PathBuf, source: std::io::Error },

    #[error("cannot write {path}: {source}")]
    Write { path: PathBuf, source: std::io::Error },

    #[error("bad JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("bad JSON in {path}: {source}")]
    JsonFile {
        path: PathBuf,
        source: serde_json::Error,
    },

    #[error("content_type is {found:?}, expected {expected:?}")]
    ContentType { found: String, expected: &'static str },

    #[error("{ring}: {reason}")]
    Ring { ring: String, reason: RingError },

    #[error("there are no rings to rasterize")]
    NoRings,

    #[error("tour {index}: {reason}")]
    Tour { index: usize, reason: TourError },

    #[error(transparent)]
    Model(#[from] ModelError),
}

impl IoError {
    /// Adds the path to a JSON error, which serde reports without it.
    pub(crate) fn in_file(self, path: &std::path::Path) -> Self {
        match self {
            Self::Json(source) => Self::JsonFile {
                path: path.to_path_buf(),
                source,
            },
            other => other,
        }
    }
}
