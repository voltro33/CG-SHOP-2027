//! Sharing the work among the `k` cutters.
//!
//! - [`SplitWalk`]: cut one walk into at most `k` pieces (port of `split.py`).
//! - [`Groups`]: order the segments, cut the order into `k` groups priced
//!   with their way home, then reorder each group closed (port of
//!   `construct/groups.py`).
//! - [`Bands`]: cut the region into single cutter bands by straight lines
//!   (port of `partition.band_problems`); [`rebalanced`] gives the next
//!   round's shares. The rebalancing loop itself belongs to the pipeline.

mod bands;
mod cut;
mod groups;
mod rebalance;
mod split;

pub use bands::{rebalanced, Bands};
pub use cut::group_order;
pub use groups::{auto_kick_seconds, Groups};
pub use split::{split_bounds, SplitWalk};
