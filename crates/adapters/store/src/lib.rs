//! Best solutions, candidates and the job log on disk: the storage side of
//! Python's miner (`mine.py`), in the same folders and formats, so the two
//! can share one data directory.
//!
//! - [`DirStore`] implements [`mow_core::SolutionStore`]: the best per
//!   instance in `best/`, near-best candidates in `candidates/`.
//! - [`JobLog`] appends one JSON line per finished job and reads Python's
//!   `jobs.jsonl`; [`best_objectives`] condenses it per instance.
//!
//! Every write goes through a temporary file and a rename.

mod atomic;
mod dir_store;
mod job_log;

use std::path::Path;

use mow_core::PortError;

pub use atomic::{safe_name, write_atomic};
pub use dir_store::{DirStore, CANDIDATE_SLACK};
pub use job_log::{best_objectives, JobLog, JobRecord};

fn external(path: &Path, error: impl std::fmt::Display) -> PortError {
    PortError::External {
        context: path.display().to_string(),
        message: error.to_string(),
    }
}
