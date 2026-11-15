//! What was tried on one instance: the candidate tours kept near the best,
//! and every job the log holds for it.

use std::path::PathBuf;

use mow_store::{JobLog, JobRecord};

use crate::data::Paths;

const SUFFIX: &str = ".solution.json";

/// A tour kept in `candidates/<uid>/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The file name without the uid and suffix: the job that made it.
    pub label: String,
    pub objective: u64,
    pub path: PathBuf,
}

/// The candidates of `uid`, shortest first. Files that do not parse are
/// left out.
pub fn candidates(paths: &Paths, uid: &str) -> Vec<Candidate> {
    let Ok(entries) = std::fs::read_dir(paths.candidates_dir(uid)) else {
        return Vec::new();
    };

    let mut found: Vec<Candidate> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?.strip_suffix(SUFFIX)?.to_owned();
            let objective = mow_io::load_solution(&path).ok()?.objective();
            let label = name
                .strip_prefix(uid)
                .map_or(name.as_str(), |rest| rest.trim_start_matches('_'))
                .to_owned();

            Some(Candidate {
                label,
                objective,
                path,
            })
        })
        .collect();
    found.sort_by(|a, b| (a.objective, &a.label).cmp(&(b.objective, &b.label)));

    found
}

/// The logged jobs of `uid`, in the order they ran.
pub fn jobs(paths: &Paths, uid: &str) -> Result<Vec<JobRecord>, String> {
    let path = paths.job_log();
    if !path.is_file() {
        return Ok(Vec::new());
    }

    let records = JobLog::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(records.into_iter().filter(|r| r.instance == uid).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_folders_hold_no_history() {
        let root = std::env::temp_dir().join(format!("mow-tui-history-{}", std::process::id()));
        let paths = crate::data::paths_under(&root, "instances");

        assert!(candidates(&paths, "nothing").is_empty());
        assert!(jobs(&paths, "nothing").unwrap().is_empty());
    }
}
