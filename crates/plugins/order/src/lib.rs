//! Segment ordering: a travelling salesman tour over segment ends where each
//! segment can be driven either way. Port of `construct/ordering.py`.
//!
//! - [`NearestNeighbour`]: greedy, from where we are to the closest free end.
//! - [`IteratedLocalSearch`]: nearest neighbour, then 2-opt and Or-opt limited
//!   to the few nearest segment ends, then kicks (block reversal, double
//!   bridge) and descents until the kick time is up, keeping the best order.
//!   Optionally simulated annealing.
//! - [`Snake`]: the lanes in list order, every second one reversed, no search
//!   (the walk of `construct/lanes.py`).
//!
//! An open order is searched as a cycle with one extra *dummy* stop whose
//! links cost nothing, so every move is a move on a cycle.

mod grid;
mod kicks;
mod moves;
mod nearest;
mod neighbours;
mod search;
mod snake;
mod snapshot;
mod tour;

pub use nearest::{nearest_neighbour, NearestNeighbour};
pub use search::{IteratedLocalSearch, Outcome, Stats};
pub use snake::{snake, Snake};
