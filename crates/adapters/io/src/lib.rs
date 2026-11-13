//! Instance and solution JSON files: the adapter to the official file format.
//!
//! - [`load_problem`] reads an `*.instance.json`, checks its rings and
//!   rasterizes them exactly as `cgshop2027_pyutils.grid._rasterize` does.
//! - [`load_solution`] and [`save_solution`] read and write
//!   `*.solution.json`; [`SolutionFile`] keeps `meta` when that matters.
//! - [`FileInstanceSource`] serves a directory of instances as an
//!   [`mow_core::InstanceSource`].
//!
//! Bad input is an [`IoError`], never a panic.

pub mod error;
pub mod instance;
pub mod raster;
pub mod schema;
pub mod solution;
pub mod source;

use std::path::Path;

pub use error::{IoError, RingError, TourError};
pub use instance::{load_problem, parse_problem, problem_from_file};
pub use raster::{rasterize_rings, Ring};
pub use schema::{InstanceFile, PointSequence, PolyominoWithHoles, SolutionFile};
pub use solution::{load_solution, load_solution_file, save_solution, save_solution_file};
pub use source::FileInstanceSource;

fn read_text(path: &Path) -> Result<String, IoError> {
    std::fs::read_to_string(path).map_err(|source| IoError::Read {
        path: path.to_path_buf(),
        source,
    })
}
