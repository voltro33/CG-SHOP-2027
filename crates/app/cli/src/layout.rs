//! Where things are: the repository root, the data folders, instance files.

use std::path::{Path, PathBuf};

use mow_verify::SCRIPT;

/// The challenge instances, relative to the repository root.
pub const DEFAULT_INSTANCES: &str = "data/instances/challenge_v0/instances";

/// Where everything Rust writes goes by default.
pub const DEFAULT_OUT: &str = "data/rust";

/// Python's data folder: `solutions/best`, `jobs.jsonl`, `bench.txt`.
pub const PYTHON_DATA: &str = "data";

const INSTANCE_SUFFIX: &str = ".instance.json";

/// The nearest folder at or above the working directory that holds the
/// verifier script, else the working directory itself.
pub fn repo_root() -> PathBuf {
    let here = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    here.ancestors()
        .find(|dir| dir.join(SCRIPT).is_file())
        .map(Path::to_path_buf)
        .unwrap_or(here)
}

/// The uid of an instance file: its name without `.instance.json`.
pub fn uid_of(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;

    name.strip_suffix(INSTANCE_SUFFIX).map(str::to_owned)
}

/// Every `*.instance.json` in `dir`, sorted by name.
pub fn instance_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("cannot list {}: {e}", dir.display()))?;

    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| uid_of(path).is_some())
        .collect();
    files.sort();

    Ok(files)
}

/// `<dir>/<uid>.instance.json`.
pub fn instance_path(dir: &Path, uid: &str) -> PathBuf {
    dir.join(format!("{uid}{INSTANCE_SUFFIX}"))
}

/// The uids of a list file such as `data/bench.txt`: one per line, `#`
/// starts a comment.
pub fn read_uid_list(path: &Path) -> Result<Vec<String>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;

    Ok(text
        .lines()
        .map(|line| line.split('#').next().unwrap_or("").trim())
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uids_come_from_instance_file_names() {
        assert_eq!(uid_of(Path::new("a/b/x_1.instance.json")).as_deref(), Some("x_1"));
        assert_eq!(uid_of(Path::new("x.solution.json")), None);
        assert_eq!(
            instance_path(Path::new("d"), "u"),
            Path::new("d").join("u.instance.json")
        );
    }

    #[test]
    fn uid_lists_skip_comments() {
        let path = std::env::temp_dir().join(format!("mow-cli-list-{}.txt", std::process::id()));
        std::fs::write(&path, "# header\nabc\n\n def # note\n").unwrap();

        assert_eq!(read_uid_list(&path).unwrap(), ["abc", "def"]);
        std::fs::remove_file(path).unwrap();
    }
}
