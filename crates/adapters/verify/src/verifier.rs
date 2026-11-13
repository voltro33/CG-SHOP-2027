//! [`PythonVerifier`]: the official verifier behind the [`Verifier`] port.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use mow_core::{PortError, Solution, Verdict, Verifier};
use mow_io::{FileInstanceSource, SolutionFile};
use serde::Deserialize;

use crate::process::{external, Launch, Worker};

/// Where `verify.py` sits below the repository root.
pub const SCRIPT: &str = "tools/verify/verify.py";

/// Overrides the python executable that [`PythonVerifier::from_repo`] finds.
pub const PYTHON_ENV: &str = "MOW_PYTHON";

/// What the official verifier said about one solution.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Report {
    pub errors: Vec<String>,
    /// The longest tour as Python measures it; `None` unless feasible.
    pub objective: Option<u64>,
}

impl Report {
    pub fn is_feasible(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn verdict(&self) -> Verdict {
        Verdict {
            errors: self.errors.clone(),
        }
    }
}

/// One answer line of `verify.py`.
#[derive(Deserialize)]
struct Answer {
    #[serde(default)]
    errors: Vec<String>,
    #[serde(default)]
    objective: Option<u64>,
    #[serde(default)]
    fault: Option<String>,
}

/// Runs `verify.py` once and sends it every solution, one at a time.
/// The process is started again once if it died.
pub struct PythonVerifier {
    launch: Launch,
    instances: FileInstanceSource,
    worker: Mutex<Option<Worker>>,
}

impl PythonVerifier {
    /// Starts the script now, so a missing python is reported here.
    /// Instance `uid` is read from `<instances>/<uid>.instance.json`.
    pub fn new(
        python: impl Into<PathBuf>,
        script: impl Into<PathBuf>,
        instances: impl Into<PathBuf>,
    ) -> Result<Self, PortError> {
        let launch = Launch {
            python: python.into(),
            script: script.into(),
        };
        let instances = FileInstanceSource::new(instances);

        if !launch.script.is_file() {
            return Err(not_found("the verifier script", &launch.script));
        }
        if !instances.dir().is_dir() {
            return Err(not_found("the instance directory", instances.dir()));
        }

        let worker = launch.spawn()?;

        Ok(Self {
            launch,
            instances,
            worker: Mutex::new(Some(worker)),
        })
    }

    /// The repository's `.venv` python (or `$MOW_PYTHON`) and its `verify.py`.
    pub fn from_repo(root: &Path, instances: impl Into<PathBuf>) -> Result<Self, PortError> {
        let python = python_in(root);
        if std::env::var_os(PYTHON_ENV).is_none() && !python.is_file() {
            return Err(not_found("the virtual environment's python", &python));
        }

        Self::new(python, root.join(SCRIPT), instances)
    }

    /// Verifies a solution file as it is, `meta` included.
    pub fn check_file(&self, file: &SolutionFile) -> Result<Report, PortError> {
        let uid = &file.instance_uid;
        let path = self
            .instances
            .path_of(uid)
            .filter(|path| path.is_file())
            .ok_or_else(|| PortError::NotFound(format!("instance {uid:?}")))?;

        let request = request(&path, file)?;
        let line = self.ask(&request)?;

        let answer: Answer = serde_json::from_str(&line)
            .map_err(|e| external(&self.launch.script, format!("bad answer {line:?}: {e}")))?;
        if let Some(fault) = answer.fault {
            return Err(external(&path, fault));
        }

        Ok(Report {
            errors: answer.errors,
            objective: answer.objective,
        })
    }

    pub fn check(&self, solution: &Solution) -> Result<Report, PortError> {
        self.check_file(&SolutionFile::from_solution(solution))
    }

    /// One round trip, with one fresh process if the current one is gone.
    fn ask(&self, request: &str) -> Result<String, PortError> {
        // a panic while holding the lock leaves at worst a dead worker,
        // which the retry below replaces
        let mut slot = self.worker.lock().unwrap_or_else(|p| p.into_inner());

        let mut last_error = None;
        for _ in 0..2 {
            let mut worker = match slot.take() {
                Some(worker) => worker,
                None => self.launch.spawn()?,
            };

            match worker.ask(request) {
                Ok(line) => {
                    *slot = Some(worker);
                    return Ok(line);
                }
                // dropping the worker kills what is left of it
                Err(e) => last_error = Some(e),
            }
        }

        let reason = last_error.map(|e| e.to_string()).unwrap_or_default();
        Err(external(
            &self.launch.script,
            format!("the verifier died twice: {reason}"),
        ))
    }

    #[cfg(test)]
    pub(crate) fn kill_worker(&self) {
        let mut slot = self.worker.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(worker) = slot.as_mut() {
            worker.kill();
        }
    }
}

impl Verifier for PythonVerifier {
    fn verify(&self, solution: &Solution) -> Result<Verdict, PortError> {
        Ok(self.check(solution)?.verdict())
    }
}

/// `$MOW_PYTHON` if set, else the `.venv` python below `root`.
pub fn python_in(root: &Path) -> PathBuf {
    if let Some(python) = std::env::var_os(PYTHON_ENV) {
        return PathBuf::from(python);
    }

    let windows = root.join(".venv/Scripts/python.exe");
    if windows.is_file() {
        return windows;
    }

    root.join(".venv/bin/python")
}

/// `{"instance": "<path>", "solution": {...}}` on one line.
fn request(instance: &Path, file: &SolutionFile) -> Result<String, PortError> {
    let instance = instance
        .to_str()
        .ok_or_else(|| external(instance, "the path is not UTF-8".to_owned()))?;

    let json = |e: mow_io::IoError| external(Path::new(&file.instance_uid), e.to_string());
    let instance = serde_json::to_string(instance).map_err(|e| json(e.into()))?;
    let mut solution = file.to_json().map_err(json)?;

    // one request per line: a pretty printed meta read from disk would split it
    if solution.contains('\n') {
        let value: serde_json::Value = serde_json::from_str(&solution).map_err(|e| json(e.into()))?;
        solution = serde_json::to_string(&value).map_err(|e| json(e.into()))?;
    }

    Ok(format!(r#"{{"instance":{instance},"solution":{solution}}}"#))
}

fn not_found(what: &str, path: &Path) -> PortError {
    PortError::NotFound(format!("{what} {}", path.display()))
}
