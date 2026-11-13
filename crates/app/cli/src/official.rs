//! The official verifier, as a few long-lived Python processes.
//!
//! One `verify.py` process answers one request at a time and caches the
//! last few instances it loaded. A pool of them sends every instance to the
//! same process, so its cache keeps working, while other instances are
//! checked meanwhile by the others.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

use mow_core::PortError;
use mow_io::SolutionFile;
use mow_verify::{PythonVerifier, Report};

/// Checks a solution file the official way.
pub trait Official: Sync {
    fn check(&self, file: &SolutionFile) -> Result<Report, PortError>;
}

/// `size` verifier processes for the instances in `instances`.
pub struct VerifierPool {
    workers: Vec<PythonVerifier>,
}

impl VerifierPool {
    /// Starts every process now, so a missing python shows up here.
    pub fn start(root: &Path, instances: &Path, size: usize) -> Result<Self, PortError> {
        let workers = (0..size.max(1))
            .map(|_| PythonVerifier::from_repo(root, instances))
            .collect::<Result<_, _>>()?;

        Ok(Self { workers })
    }

    fn worker(&self, uid: &str) -> &PythonVerifier {
        let mut hasher = DefaultHasher::new();
        uid.hash(&mut hasher);

        &self.workers[hasher.finish() as usize % self.workers.len()]
    }
}

impl Official for VerifierPool {
    fn check(&self, file: &SolutionFile) -> Result<Report, PortError> {
        self.worker(&file.instance_uid).check_file(file)
    }
}

/// What the official check says about a solution whose objective the Rust
/// side measured: `Ok` when it is feasible with that same objective.
pub fn confirm(official: &dyn Official, file: &SolutionFile, objective: u64) -> Result<(), String> {
    let report = official.check(file).map_err(|e| format!("verifier: {e}"))?;

    if !report.is_feasible() {
        let first = report.errors.first().map_or("infeasible", String::as_str);
        return Err(format!("verifier: {}", clip(first)));
    }

    match report.objective {
        Some(found) if found == objective => Ok(()),
        found => Err(format!(
            "verifier: objective {found:?}, Rust measured {objective}"
        )),
    }
}

/// At most 300 characters, as the Python job log keeps errors.
pub fn clip(text: &str) -> String {
    text.chars().take(300).collect()
}
