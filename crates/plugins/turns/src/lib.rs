//! Turn-shape search: local search on the corners of a finished solution.
//!
//! Every move replaces a few edges of the longest tour by shorter ones and
//! is kept only if every cell stays swept, which the incremental visit
//! counts of `mow-coverage` answer by looking at the changed positions
//! only:
//!
//! - **slide**: pull a U turn (a lane end connector) inward, two steps a pull;
//! - **shortcut**: replace the way over 2 to `window` corners by an L or Z;
//!   this drops detours and lanes another lane already sweeps;
//! - **2-opt**: drive a run of corners the other way, entering a lane from
//!   its other end;
//! - **or-opt**: move a block of 1 to 3 corners elsewhere, either way round.
//!
//! [`turns_routes`] runs them pass after pass, then the corner cut of
//! `mow-improve`.

mod edit;
mod moves;
mod path;
mod search;

pub use search::{turns_routes, TurnsStats};

/// A round number no mining round uses (polish is -2).
pub const TURNS_ROUND: i32 = -3;

/// Seconds per instance when no time is given.
pub const DEFAULT_SECONDS: f64 = 60.0;

/// The moves by name, in the order of [`TurnsStats::kept`].
pub const MOVES: [&str; 4] = ["slide", "shortcut", "2-opt", "or-opt"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TurnsOptions {
    /// The most corners one shortcut skips.
    pub window: usize,
    /// Old ways longer than this are not shortcut.
    pub longest_shortcut: u64,
    /// Which moves run, in the order of [`MOVES`].
    pub moves: [bool; 4],
    /// For k > 1: once the longest tour is stuck, shorten the others and
    /// move blocks to them from the longest.
    pub balance: bool,
}

impl Default for TurnsOptions {
    fn default() -> Self {
        Self {
            window: 6,
            longest_shortcut: 2_000,
            moves: [true; 4],
            balance: true,
        }
    }
}
