//! The job log: one JSON line per finished job, append only, as the Python
//! miner's `jobs.jsonl`.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use mow_core::PortError;
use serde::{Deserialize, Serialize};

use crate::external;

/// One finished job, the fields of Python's `JobResult` minus the solution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobRecord {
    pub job_id: String,
    pub instance: String,
    pub round: i32,
    pub label: String,
    /// `None` when the job failed.
    pub objective: Option<u64>,
    pub seconds: f64,
    pub error: Option<String>,
}

impl JobRecord {
    /// A verified result: an objective and no error.
    pub fn feasible_objective(&self) -> Option<u64> {
        match self.error {
            None => self.objective,
            Some(_) => None,
        }
    }
}

/// Appends records to one file; several threads may share it.
#[derive(Debug)]
pub struct JobLog {
    path: PathBuf,
    file: Mutex<File>,
}

impl JobLog {
    /// Opens (or creates) the log for appending. If a crash cut the last
    /// line short, a newline ends it, so the next record starts clean.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, PortError> {
        let path = path.into();
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            fs::create_dir_all(dir).map_err(|e| external(dir, e))?;
        }

        let mut file = OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .open(&path)
            .map_err(|e| external(&path, e))?;

        if !ends_with_newline(&mut file).map_err(|e| external(&path, e))? {
            file.write_all(b"\n").map_err(|e| external(&path, e))?;
        }

        Ok(Self {
            path,
            file: Mutex::new(file),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Writes one line and syncs it, so a crash right after loses nothing.
    pub fn append(&self, record: &JobRecord) -> Result<(), PortError> {
        let mut line = serde_json::to_string(record).map_err(|e| external(&self.path, e))?;
        line.push('\n');

        let mut file = self.file.lock().unwrap_or_else(|p| p.into_inner());
        file.write_all(line.as_bytes())
            .and_then(|()| file.sync_data())
            .map_err(|e| external(&self.path, e))
    }

    /// Every readable record of the log at `path`; none if it does not
    /// exist. Unreadable lines (a crash mid write) are skipped, as Python does.
    pub fn read(path: &Path) -> Result<Vec<JobRecord>, PortError> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(external(path, e)),
        };

        Ok(text
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect())
    }
}

/// The best feasible objective per instance and the label that reached it.
/// On a tie the earlier record wins, as in Python's report.
pub fn best_objectives(records: &[JobRecord]) -> BTreeMap<String, (u64, String)> {
    let mut best: BTreeMap<String, (u64, String)> = BTreeMap::new();

    for record in records {
        let Some(objective) = record.feasible_objective() else {
            continue;
        };

        let better = best
            .get(&record.instance)
            .map_or(true, |&(current, _)| objective < current);
        if better {
            best.insert(record.instance.clone(), (objective, record.label.clone()));
        }
    }

    best
}

/// True for an empty file too: there is nothing to finish.
fn ends_with_newline(file: &mut File) -> std::io::Result<bool> {
    if file.metadata()?.len() == 0 {
        return Ok(true);
    }

    let mut last = [0u8];
    file.seek(SeekFrom::End(-1))?;
    file.read_exact(&mut last)?;

    Ok(last[0] == b'\n')
}
