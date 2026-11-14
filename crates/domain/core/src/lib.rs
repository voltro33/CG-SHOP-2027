//! The domain of the lawn mowing solver: what an instance is, what a tour is,
//! and the contracts every step and every adapter implements.
//!
//! Vocabulary, as in the Python solver:
//!
//! - *cell*: a unit square `(x, y)` of the grid.
//! - *anchor*: a lattice point where the cutter's center stands.
//! - *footprint* of an anchor `p`: the cells `p + o` for every cutter offset `o`.
//! - *segment*: a straight stretch of anchors on one lane.
//! - *route*: a closed tour, stored as its corners.
//!
//! Ownership: one [`Problem`] is owned per instance and every step borrows it.
//! Small values ([`Point`], [`Segment`], [`Visit`]) are `Copy`. Steps that
//! rewrite routes take them by value and hand back new ones.

pub mod bounds;
pub mod cutter;
pub mod error;
pub mod geom;
pub mod grid;
pub mod hull;
pub mod observe;
pub mod order;
pub mod plugin;
pub mod port;
pub mod problem;
pub mod route;
pub mod segment;
pub mod solution;
pub mod walk;
pub mod witness;

pub use bounds::{area_bound, extent_bound, lower_bound};
pub use cutter::{Cutter, RowExtent};
pub use error::{ModelError, PortError, SolveError};
pub use geom::{Axis, Bounds, Point};
pub use grid::{BitGrid, Run};
pub use hull::{hull_bound, orthogonal_hull};
pub use observe::{Event, NoObserver, Observer};
pub use order::{Closure, Order, Visit};
pub use plugin::{Assigner, Budget, Decomposer, Improver, Orderer, Partitioner, Repairer, Splitter};
pub use port::{InstanceSource, Offer, SolutionStore, Verdict, Verifier};
pub use problem::{Frame, Problem};
pub use route::Route;
pub use segment::Segment;
pub use solution::Solution;
pub use walk::Walk;
pub use witness::witness_bound;
