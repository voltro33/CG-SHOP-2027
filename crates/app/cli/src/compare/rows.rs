//! One row per instance: its size, its lower bound and every side's best.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use mow_core::lower_bound;
use mow_io::{load_solution, SolutionFile};
use mow_store::{best_objectives, DirStore, JobLog, JobRecord};

use crate::layout::{instance_files, uid_of};
use crate::meta::meta_field;

/// What the official verifier said about the Rust best.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Check {
    /// No Rust best, or verification was switched off.
    NotRun,
    Passed,
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct Row {
    pub uid: String,
    pub k: usize,
    pub cells: usize,
    pub lower_bound: u64,
    pub bench: bool,
    /// Python's stored best, polish included.
    pub python_best: Option<u64>,
    /// Python's best mined result, polish left out (rounds 0 and up).
    pub python_mined: Option<u64>,
    pub rust: Option<u64>,
    pub rust_config: Option<String>,
    /// The Rust best as stored, for the verifier.
    pub rust_file: Option<SolutionFile>,
    pub check: Check,
}

impl Row {
    pub fn ratio_best(&self) -> Option<f64> {
        ratio(self.rust, self.python_best)
    }

    pub fn ratio_mined(&self) -> Option<f64> {
        ratio(self.rust, self.python_mined)
    }
}

fn ratio(ours: Option<u64>, theirs: Option<u64>) -> Option<f64> {
    match (ours, theirs) {
        (Some(ours), Some(theirs)) if theirs > 0 => Some(ours as f64 / theirs as f64),
        _ => None,
    }
}

/// Where each side keeps its results.
pub struct Sources<'a> {
    pub instances: &'a Path,
    /// Python's data folder: `solutions/best`, `jobs.jsonl`.
    pub python: &'a Path,
    pub rust: &'a DirStore,
    pub bench: &'a HashSet<String>,
}

/// One row per instance file, in name order.
pub fn rows(sources: &Sources) -> Result<Vec<Row>, String> {
    let python_log = JobLog::read(&sources.python.join("jobs.jsonl")).map_err(|e| e.to_string())?;
    let mined: Vec<JobRecord> = python_log.into_iter().filter(|r| r.round >= 0).collect();
    let mined = best_objectives(&mined);

    instance_files(sources.instances)?
        .iter()
        .map(|path| row(sources, &mined, path))
        .collect()
}

fn row(sources: &Sources, mined: &BTreeMap<String, (u64, String)>, path: &Path) -> Result<Row, String> {
    let uid = uid_of(path).unwrap_or_default();
    let problem = mow_io::load_problem(path).map_err(|e| format!("{}: {e}", path.display()))?;

    let python_path = sources
        .python
        .join("solutions/best")
        .join(format!("{uid}.solution.json"));
    let python_best = match python_path.is_file() {
        true => Some(
            load_solution(&python_path)
                .map_err(|e| format!("{}: {e}", python_path.display()))?
                .objective(),
        ),
        false => None,
    };

    let rust_file = sources.rust.best_file(&uid).map_err(|e| e.to_string())?;
    let rust = match &rust_file {
        Some(file) => Some(file.to_solution().map_err(|e| format!("{uid}: {e}"))?.objective()),
        None => None,
    };

    Ok(Row {
        k: problem.k(),
        cells: problem.num_cells(),
        lower_bound: lower_bound(&problem),
        bench: sources.bench.contains(&uid),
        python_best,
        python_mined: mined.get(&uid).map(|&(objective, _)| objective),
        rust,
        rust_config: rust_file.as_ref().and_then(|f| meta_field(f, "config")),
        rust_file,
        check: Check::NotRun,
        uid,
    })
}

/// Total seconds in a job log, and in its records with round 0 and up.
pub fn cpu_seconds(path: &Path) -> Result<(f64, f64), String> {
    let records = JobLog::read(path).map_err(|e| e.to_string())?;

    let all = records.iter().map(|r| r.seconds).sum();
    let mined = records.iter().filter(|r| r.round >= 0).map(|r| r.seconds).sum();

    Ok((all, mined))
}
