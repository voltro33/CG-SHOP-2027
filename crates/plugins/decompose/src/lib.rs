//! Lawn to segments: plain lanes, lane segments and mixed lanes.
//!
//! The first two decomposers cut the region into horizontal lanes one cutter
//! height apart, bottom to top, and give vertical lanes by running on the
//! mirrored problem. [`Lanes`] drives each lane edge to edge; [`LaneSegments`]
//! cuts each lane into the stretches that have cells to mow, bridging short
//! gaps. [`MixedLanes`] cuts the region into square tiles and sweeps each one
//! horizontally or vertically, whichever is cheaper. [`HuggedLanes`] moves
//! the lane grid per strip of columns so lanes sit flush with the edges.
//!
//! Output order matches the Python solver: lanes bottom to top (or left to
//! right for vertical lanes), segments within a lane from low to high.

mod axis;
mod hug;
mod intervals;
mod lanes;
mod mixed;
mod rows;
mod segments;

#[cfg(test)]
mod testing;

pub use hug::HuggedLanes;
pub use lanes::Lanes;
pub use mixed::MixedLanes;
pub use segments::LaneSegments;
