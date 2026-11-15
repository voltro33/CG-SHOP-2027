//! Which cells a set of routes mows, in full and incrementally.
//!
//! The full check is the official one (`verify.py`): mark every anchor the
//! routes pass, edge by edge, clipped to the anchor frame, then take the
//! Minkowski sum with the cutter. The sum works on whole `u64` row words, one
//! shift-or per cutter row run.
//!
//! The incremental check is [`VisitCounts`]: visits per anchor and visited
//! anchors per cell, so that taking a stretch of positions out of a tour is
//! answered by looking only at the cells around that stretch.

pub mod error;
pub mod full;
mod rows;
mod runs;
pub mod visits;

pub use error::CoverageError;
pub use full::{anchors, is_covered, swept, uncovered, uncovered_count, uncovered_walks, walk_anchors};
pub use visits::{Scratch, VisitCounts};
