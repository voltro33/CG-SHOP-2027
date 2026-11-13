//! The `mow` command: solve one config, mine the job grid, compare with
//! Python.
//!
//! - [`registry::Kit`]: a config label to the concrete plugins of one run,
//!   as the table in the `mow-pipeline` crate docs maps them.
//! - [`grid`]: the job grid of `jobs.py`.
//! - [`solve_cmd`]: `mow solve`, one config on one instance.
//! - [`mine`]: `mow mine`, the grid on threads (`mine.py`).
//! - [`compare`]: `mow compare`, Rust bests against Python's.

pub mod args;
pub mod compare;
pub mod grid;
pub mod layout;
pub mod meta;
pub mod mine;
pub mod official;
pub mod registry;
pub mod solve_cmd;
