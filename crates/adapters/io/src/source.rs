//! Instances from a directory of `<uid>.instance.json` files.

use std::fs;
use std::path::{Path, PathBuf};

use mow_core::{InstanceSource, PortError, Problem};

use crate::error::IoError;
use crate::instance::load_problem;

const SUFFIX: &str = ".instance.json";

/// An [`InstanceSource`] over one directory, not recursive.
#[derive(Debug, Clone)]
pub struct FileInstanceSource {
    dir: PathBuf,
}

impl FileInstanceSource {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Where the instance `uid` would be. `None` for a uid that is not a
    /// plain file name, so a uid can never reach outside the directory.
    pub fn path_of(&self, uid: &str) -> Option<PathBuf> {
        let plain = !uid.is_empty() && !uid.contains(['/', '\\', ':']) && uid != "." && uid != "..";
        plain.then(|| self.dir.join(format!("{uid}{SUFFIX}")))
    }
}

impl InstanceSource for FileInstanceSource {
    /// Every uid in the directory, sorted.
    fn uids(&self) -> Result<Vec<String>, PortError> {
        let entries = fs::read_dir(&self.dir).map_err(|e| external(&self.dir, e))?;

        let mut uids = Vec::new();
        for entry in entries {
            let name = entry.map_err(|e| external(&self.dir, e))?.file_name();
            if let Some(uid) = name.to_str().and_then(|n| n.strip_suffix(SUFFIX)) {
                uids.push(uid.to_owned());
            }
        }

        uids.sort_unstable();
        Ok(uids)
    }

    fn load(&self, uid: &str) -> Result<Problem, PortError> {
        let path = self
            .path_of(uid)
            .ok_or_else(|| PortError::NotFound(uid.to_owned()))?;
        if !path.is_file() {
            return Err(PortError::NotFound(uid.to_owned()));
        }

        let problem = load_problem(&path).map_err(|e| port_error(&path, e))?;
        if problem.uid() != uid {
            return Err(PortError::External {
                context: path.display().to_string(),
                message: format!("the file holds instance {:?}", problem.uid()),
            });
        }

        Ok(problem)
    }
}

fn port_error(path: &Path, error: IoError) -> PortError {
    match error {
        IoError::Model(e) => PortError::Model(e),
        other => external(path, other),
    }
}

fn external(path: &Path, error: impl std::fmt::Display) -> PortError {
    PortError::External {
        context: path.display().to_string(),
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUARE: &str = r#"{"instance_uid":"sq",
        "region_to_cover":{"outer_boundary":{"x":[0,4,4,0],"y":[0,0,4,4]},"inner_boundaries":[]},
        "cutter":{"x":[0,1,1,0],"y":[0,0,1,1]},"cutter_center":[0,0],"number_of_cutters":1}"#;

    fn source() -> FileInstanceSource {
        let dir = std::env::temp_dir().join(format!("mow-io-source-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();

        fs::write(dir.join("sq.instance.json"), SQUARE).unwrap();
        fs::write(dir.join("other.instance.json"), SQUARE).unwrap();
        fs::write(dir.join("notes.txt"), "ignored").unwrap();

        FileInstanceSource::new(dir)
    }

    #[test]
    fn lists_and_loads() {
        let source = source();

        assert_eq!(source.uids().unwrap(), vec!["other", "sq"]);
        assert_eq!(source.load("sq").unwrap().num_cells(), 16);
    }

    #[test]
    fn missing_mismatched_and_escaping_uids_are_errors() {
        let source = source();

        assert!(matches!(source.load("nope"), Err(PortError::NotFound(_))));
        assert!(matches!(source.load("../sq"), Err(PortError::NotFound(_))));
        assert!(matches!(source.load("other"), Err(PortError::External { .. })));
        assert!(FileInstanceSource::new("no/such/dir").uids().is_err());
    }
}
