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
//! - [`turns_cmd`]: `mow turns`, the corner search of `mow-turns` on the bests.
//! - [`lns_cmd`]: `mow lns`, the ruin and recreate search of `mow-lns` on the bests.
//! - [`lns_plan`]: the seconds of each `mow lns` job, even or weighted.
//! - [`reports`]: `mow report | mine-report | diagnose | catalog | bench`,
//!   over the `mow-report` crate.

pub mod args;
pub mod compare;
pub mod grid;
pub mod layout;
pub mod lns_cmd;
pub mod lns_plan;
pub mod meta;
pub mod mine;
pub mod official;
mod patterns;
pub mod polish_cmd;
pub mod registry;
pub mod reports;
pub mod solve_cmd;
pub mod turns_cmd;
