//! The `mow` command: solve one config, mine the job grid, compare with
//! Python.
//!
//! - [`registry::Kit`]: a config label to the concrete plugins of one run,
//!   as the table in the `mow-pipeline` crate docs maps them.
//! - [`grid`]: the job grid of `jobs.py`.
//! - [`solve_cmd`]: `mow solve`, one config on one instance.
//! - [`mine`]: `mow mine`, the grid on threads (`mine.py`).
//! - [`compare`]: `mow compare`, Rust bests against Python's.
//! - [`polish_cmd`]: `mow polish`, CP-SAT window polish of the bests (`polish.py`).
//! - [`reports`]: `mow report | mine-report | diagnose | catalog | bench`,
//!   over the `mow-report` crate.

pub mod args;
pub mod compare;
pub mod grid;
pub mod layout;
pub mod meta;
pub mod mine;
pub mod official;
mod patterns;
pub mod polish_cmd;
pub mod registry;
pub mod reports;
pub mod solve_cmd;
