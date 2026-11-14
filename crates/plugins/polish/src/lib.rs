//! The window polish: re-solve one square window of a finished tour with
//! CP-SAT and keep the rest, as `solver/window.py`, `window_map.py` and
//! the algorithm half of `polish.py`.
//!
//! The tour inside a window falls apart into pieces, each a run of
//! positions from an entry to an exit. Outside the window nothing changes.
//! The model ([`WindowModel`]) finds new ways through the window:
//!
//! - one integer per directed unit step between neighbouring anchors in the
//!   window, how often the new walk takes it (0, 1 or 2)
//! - two port nodes per piece: `in_i` steps onto entry `i`, exit `i` steps
//!   onto `out_i`, and a fixed arc `out_i -> in_{i+1}` stands for the tour
//!   outside
//! - every node has as many steps in as out, and a flow from the first port
//!   reaches every node the walk visits, so the steps form one closed walk
//! - every region cell that nothing outside the cut pieces sweeps has a
//!   visited anchor among `cell - offsets`
//! - minimize the number of steps inside the window, at most the old ones
//!
//! The model is plain data ([`mow_core::CpModel`]) and goes to whatever
//! [`mow_core::CpSolver`] the caller hands in. It is built variable for
//! variable and constraint for constraint as Python builds it.
//!
//! - [`solve_window`]: one window.
//! - [`polish_tours`], [`polish_routes`]: windows over the longest tour
//!   until a deadline.
//! - [`window_map`], [`summarize`], [`window_report`]: where the room to
//!   save is.

mod error;
mod ids;
mod model;
mod polish;
mod required;
mod solve;
mod splice;
mod summary;
mod tiles;
mod window;
mod window_map;

pub use error::PolishError;
pub use ids::{exclusive_cells, AnchorIds, NO_WINDOW};
pub use model::{WindowModel, MAX_USES};
pub use polish::{
    longest, polish_routes, polish_seconds, polish_tours, PolishOptions, PolishStats, DEFAULT_SIDE,
    DEFAULT_WINDOW_SECONDS, POLISH_ROUND, POLISH_SECONDS, POLISH_SECONDS_LARGEST,
};
pub use required::required_cells;
pub use solve::{prepare_window, solve_window, PreparedWindow, WindowResult};
pub use summary::{
    family, numpy_sum, report_header, report_line, summarize, window_report, WindowSummary, DEFAULT_SIDES,
    WORST_SHARES,
};
pub use tiles::{aimed, tiles, tiles_at};
pub use window::{cut_pieces, Piece, Window};
pub use window_map::{cutter_span, dense_tours, window_map, WindowMap};
