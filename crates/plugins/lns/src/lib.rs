//! Ruin and recreate at segment scale on a finished solution (adaptive
//! large neighbourhood search, Ropke and Pisinger 2006).
//!
//! One move:
//!
//! - **destroy** ([`Destroy`]): take the positions of a tour inside a window
//!   out (a square or a band of lanes, around a random or a dead position);
//!   the tour falls into chains;
//! - **recreate** ([`Recreate`]): sweep the cells left bare with lane
//!   segments of `mow-decompose` restricted to them, in either direction,
//!   at a random lane offset and gap setting, or with single anchors;
//! - **reconnect**: chains and new segments in one small closed order,
//!   cheapest insertion then 2-opt and Or-opt (chains may turn round);
//! - **repair and cut**: anchors for cells still missed, then corner cuts of
//!   `mow-improve` on the new walks only;
//! - **accept**: by the real cost (the longest tour, plus a tenth of the
//!   others), simulated annealing, coverage checked incrementally with
//!   `mow-coverage`'s visit counts on every move.
//!
//! For k > 1 the `transfer` destroy moves the bare cells of the longest
//! tour's window into the nearest other tour.
//!
//! Operators are drawn by adaptive weights, kept per instance in a state
//! file with the walk's current solution ([`resume`]).

mod anchors;
mod chains;
mod cycle;
mod destroy;
mod edit;
mod error;
mod moves;
mod recreate;
mod resume;
mod saved;
mod search;
mod tours;
mod weights;

#[cfg(test)]
mod testing;

pub use destroy::Destroy;
pub use error::LnsError;
pub use recreate::Recreate;
pub use resume::{resume, DRIFT};
pub use saved::{shape_of, Saved};
pub use search::{default_wheels, search, LnsOptions, LnsOutcome, SearchStats};
pub use weights::{Operator, Tally, Wheel};
