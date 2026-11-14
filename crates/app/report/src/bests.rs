//! The stored bests of a data folder, as `mine.BestStore` loads them: the
//! objective per instance and the job that made it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mow_io::load_solution_file;

const SUFFIX: &str = ".solution.json";

/// One stored best.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Best {
    pub objective: u64,
    /// `meta.job` of the file, `?` without one.
    pub label: String,
    pub path: PathBuf,
}

/// Every `*.solution.json` in `dir`, by the uid inside the file. A missing
/// folder holds no bests.
pub fn read_bests(dir: &Path) -> Result<BTreeMap<String, Best>, String> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(e) => return Err(format!("cannot list {}: {e}", dir.display())),
    };

    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.to_string_lossy().ends_with(SUFFIX))
        .collect();
    paths.sort();

    let mut bests = BTreeMap::new();
    for path in paths {
        let (uid, best) = read_best(path)?;
        bests.insert(uid, best);
    }

    Ok(bests)
}

fn read_best(path: PathBuf) -> Result<(String, Best), String> {
    let file = load_solution_file(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let solution = file
        .to_solution()
        .map_err(|e| format!("{}: {e}", path.display()))?;

    let meta: serde_json::Value = serde_json::from_str(file.meta.get()).unwrap_or_default();
    let label = match meta.get("job") {
        None => "?".to_owned(),
        Some(serde_json::Value::String(job)) => job.clone(),
        Some(other) => other.to_string(),
    };

    let best = Best {
        objective: solution.objective(),
        label,
        path,
    };

    Ok((file.instance_uid, best))
}
