//! [`CpSatPool`]: a few `cpsat.py` processes behind the [`CpSolver`] port.

use std::path::{Path, PathBuf};
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::Duration;

use mow_core::{CpModel, CpParams, CpResponse, CpSolver, CpStatus, PortError};
use serde::{Deserialize, Serialize};

use crate::process::{external, Launch, Worker};
use crate::proto::{int64s, ModelProto};

/// Where `cpsat.py` sits below the repository root.
pub const SCRIPT: &str = "tools/cpsat/cpsat.py";

/// Overrides the python executable that [`CpSatPool::from_repo`] finds, as
/// it does for the verifier.
pub const PYTHON_ENV: &str = "MOW_PYTHON";

/// How long past its own time limit a solve may take to answer (loading the
/// model, a first OR-Tools import) before its process counts as stuck.
const GRACE: Duration = Duration::from_secs(60);

/// How long an echo, which solves nothing, may take.
const ECHO_TIMEOUT: Duration = Duration::from_secs(120);

const STATUSES: [CpStatus; 5] = [
    CpStatus::Optimal,
    CpStatus::Feasible,
    CpStatus::Infeasible,
    CpStatus::ModelInvalid,
    CpStatus::Unknown,
];

/// One request line of `cpsat.py`.
#[derive(Serialize)]
struct Request<'a> {
    model: &'a ModelProto,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Params>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    echo: bool,
}

/// The `SatParameters` a request sets, named as the proto names them.
#[derive(Serialize)]
struct Params {
    max_time_in_seconds: f64,
    num_workers: u32,
}

/// One answer line of `cpsat.py`.
#[derive(Deserialize)]
struct Answer {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    objective: Option<f64>,
    #[serde(default, deserialize_with = "int64s")]
    values: Vec<i64>,
    #[serde(default)]
    model: Option<serde_json::Value>,
    #[serde(default)]
    fault: Option<String>,
}

/// `size` long-lived `cpsat.py` processes. Each solve takes a free one and
/// waits while all are busy, so up to `size` models are solved at once.
/// A process that died is started again once per solve.
pub struct CpSatPool {
    launch: Launch,
    size: usize,
    /// The free slots; `None` is a slot whose process died, started again
    /// by the next solve that takes it.
    idle: Mutex<Vec<Option<Worker>>>,
    freed: Condvar,
}

/// A slot taken from the pool, given back when dropped, even on a panic.
struct Lease<'a> {
    pool: &'a CpSatPool,
    slot: Option<Worker>,
}

impl CpSatPool {
    /// Starts every process now, so a missing python is reported here.
    /// A `size` of 0 is taken as 1.
    pub fn new(
        python: impl Into<PathBuf>,
        script: impl Into<PathBuf>,
        size: usize,
    ) -> Result<Self, PortError> {
        let launch = Launch {
            python: python.into(),
            script: script.into(),
        };

        if !launch.script.is_file() {
            return Err(not_found("the CP-SAT script", &launch.script));
        }

        let size = size.max(1);
        let idle = (0..size)
            .map(|_| launch.spawn().map(Some))
            .collect::<Result<_, _>>()?;

        Ok(Self {
            launch,
            size,
            idle: Mutex::new(idle),
            freed: Condvar::new(),
        })
    }

    /// The repository's `.venv` python (or `$MOW_PYTHON`) and its `cpsat.py`.
    pub fn from_repo(root: &Path, size: usize) -> Result<Self, PortError> {
        let python = python_in(root);
        if std::env::var_os(PYTHON_ENV).is_none() && !python.is_file() {
            return Err(not_found("the virtual environment's python", &python));
        }

        Self::new(python, root.join(SCRIPT), size)
    }

    /// How many models can be solved at once.
    pub fn size(&self) -> usize {
        self.size
    }

    /// The model as Python read it, sent back without solving: a check that
    /// both sides mean the same model.
    pub fn echo(&self, model: &CpModel) -> Result<CpModel, PortError> {
        let proto = ModelProto::from_model(model);
        let answer = self.answer(
            &Request {
                model: &proto,
                params: None,
                echo: true,
            },
            ECHO_TIMEOUT,
        )?;

        let json = answer
            .model
            .ok_or_else(|| self.external("the echo has no model".to_owned()))?;

        crate::from_proto_json(&json.to_string())
    }

    /// Sends a request and reads its answer, a `fault` made an error.
    fn answer(&self, request: &Request, timeout: Duration) -> Result<Answer, PortError> {
        let request = serde_json::to_string(request).expect("plain numbers always serialize");
        let answer = self.ask(&request, timeout)?;
        if let Some(fault) = answer.fault {
            return Err(self.external(fault));
        }

        Ok(answer)
    }

    /// One round trip on a free process, with one fresh process if it is gone.
    /// A process whose answer does not parse is dropped, not reused: a stray
    /// line would shift every later answer by one. A process that does not
    /// answer within `timeout` is killed and the request not tried again.
    fn ask(&self, request: &str, timeout: Duration) -> Result<Answer, PortError> {
        let mut lease = self.lease();

        let mut last_error = None;
        for _ in 0..2 {
            let mut worker = match lease.slot.take() {
                Some(worker) => worker,
                None => self.launch.spawn()?,
            };

            match worker.ask(request, timeout) {
                Ok(line) => {
                    let answer: Answer = serde_json::from_str(&line)
                        .map_err(|e| self.external(format!("bad answer {line:?}: {e}")))?;
                    lease.slot = Some(worker);
                    return Ok(answer);
                }
                // dropping the worker kills what is left of it
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    return Err(self.external(format!("the solver is stuck: {e}")));
                }
                Err(e) => last_error = Some(e),
            }
        }

        let reason = last_error.map(|e| e.to_string()).unwrap_or_default();
        Err(self.external(format!("the solver died twice: {reason}")))
    }

    /// Waits for a free slot and takes it.
    fn lease(&self) -> Lease<'_> {
        let mut idle = self.idle();
        loop {
            if let Some(slot) = idle.pop() {
                return Lease { pool: self, slot };
            }

            idle = self.freed.wait(idle).unwrap_or_else(|p| p.into_inner());
        }
    }

    /// The free slots. A panic while holding the lock leaves at worst a dead
    /// worker in a slot, which the next solve replaces.
    fn idle(&self) -> MutexGuard<'_, Vec<Option<Worker>>> {
        self.idle.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn external(&self, message: String) -> PortError {
        external(&self.launch.script, message)
    }

    #[cfg(test)]
    pub(crate) fn kill_workers(&self) {
        for worker in self.idle().iter_mut().flatten() {
            worker.kill();
        }
    }
}

impl Drop for Lease<'_> {
    fn drop(&mut self) {
        self.pool.idle().push(self.slot.take());
        self.pool.freed.notify_one();
    }
}

impl CpSolver for CpSatPool {
    fn solve(&self, model: &CpModel, params: &CpParams) -> Result<CpResponse, PortError> {
        let proto = ModelProto::from_model(model);
        let timeout = params.time_limit.saturating_add(GRACE);
        let params = Params {
            max_time_in_seconds: params.time_limit.as_secs_f64(),
            num_workers: params.workers,
        };

        let answer = self.answer(
            &Request {
                model: &proto,
                params: Some(params),
                echo: false,
            },
            timeout,
        )?;

        let name = answer.status.unwrap_or_default();
        let status = STATUSES
            .into_iter()
            .find(|status| status.name() == name)
            .ok_or_else(|| self.external(format!("unknown status {name:?}")))?;

        let expected = if status.found() { model.domains.len() } else { 0 };
        if answer.values.len() != expected {
            let found = answer.values.len();
            return Err(self.external(format!("{name} with {found} values for {expected}")));
        }

        Ok(CpResponse {
            status,
            // CP-SAT always reports a finite one; NaN marks one that was not
            objective: answer.objective.unwrap_or(f64::NAN),
            values: answer.values,
        })
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

fn not_found(what: &str, path: &Path) -> PortError {
    PortError::NotFound(format!("{what} {}", path.display()))
}
