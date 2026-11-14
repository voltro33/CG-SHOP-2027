//! Where the reports read and write: the two data folder layouts, defaults
//! that prefer Rust's files, and the Python files that are never written.

use std::path::{Path, PathBuf};

use mow_report::catalog::{self, Features};
use mow_store::{write_atomic, JobLog, JobRecord};

use crate::layout::{DEFAULT_OUT, PYTHON_DATA};

/// Python's files that only Python writes.
const PROTECTED: [&str; 5] = [
    "data/solutions",
    "data/jobs.jsonl",
    "data/catalog.csv",
    "data/leaderboard.csv",
    "data/bench.txt",
];

/// The bests of a data folder: `solutions/best/` in Python's, `best/` in Rust's.
pub fn best_dir(data: &Path) -> PathBuf {
    let python = data.join("solutions").join("best");

    match python.is_dir() {
        true => python,
        false => data.join("best"),
    }
}

/// `data/rust/<name>` if it exists, else Python's `data/<name>`.
pub fn prefer_rust(name: &str) -> PathBuf {
    let rust = Path::new(DEFAULT_OUT).join(name);

    match rust.is_file() {
        true => rust,
        false => Path::new(PYTHON_DATA).join(name),
    }
}

/// The catalog at `path`, or the default one.
pub fn read_catalog(path: Option<&str>) -> Result<Vec<Features>, String> {
    let path = path.map_or_else(|| prefer_rust("catalog.csv"), PathBuf::from);

    catalog::read(&path)
}

pub fn read_log(path: &Path) -> Result<Vec<JobRecord>, String> {
    JobLog::read(path).map_err(|e| e.to_string())
}

/// True if `path` is, or lies in, one of Python's own files.
pub fn is_protected(path: &Path) -> bool {
    let Ok(path) = std::path::absolute(path) else {
        return true;
    };
    let root = crate::layout::repo_root();

    PROTECTED
        .iter()
        .filter_map(|name| std::path::absolute(root.join(name)).ok())
        .any(|protected| path.starts_with(protected))
}

/// Writes `text` to `path` unless it is one of Python's files.
pub fn save(path: &Path, text: &str) -> Result<(), String> {
    if is_protected(path) {
        return Err(format!("{} belongs to Python, pick another path", path.display()));
    }

    write_atomic(path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_files_are_protected() {
        let root = crate::layout::repo_root();

        assert!(is_protected(&root.join("data/leaderboard.csv")));
        assert!(is_protected(&root.join("data/solutions/best/x.solution.json")));
        assert!(!is_protected(&root.join("data/rust/leaderboard.csv")));
    }
}
