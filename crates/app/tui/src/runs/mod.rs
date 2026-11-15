//! Background runs: work that outlives the menus. Each run is a folder
//! `data/rust/runs/<id>/`:
//!
//! - `run.json`, the [`Spec`]: phases in order, the steps of one phase side
//!   by side;
//! - `pid`, the supervisor's process id, written when it starts;
//! - `<step>.log`, each command's output;
//! - `done.json` once the supervisor finishes, or `stopped` if the user
//!   stopped it.
//!
//! The supervisor is `mow __supervise <dir>` ([`supervise`]), started
//! detached ([`process`]), so closing the menus leaves the run going.

pub mod process;
pub mod progress;
pub mod supervise;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub use progress::Progress;

const SPEC: &str = "run.json";
const DONE: &str = "done.json";
const PID: &str = "pid";
const STOPPED: &str = "stopped";

/// What a run does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub id: String,
    /// `lns`, `turns`, `pack`, `build`, `experiment`, ...
    pub kind: String,
    pub title: String,
    /// Seconds since 1970.
    pub started: u64,
    /// The score of `data` when it started.
    pub score_before: f64,
    /// The data folder whose score the run changes.
    pub data: PathBuf,
    pub instances: PathBuf,
    /// Files written into the run's folder when it is created: name, text.
    #[serde(default)]
    pub files: Vec<(String, String)>,
    pub phases: Vec<Vec<Step>>,
}

/// One unit of work.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "do", rename_all = "lowercase")]
pub enum Step {
    /// A command, its output in `<name>.log`.
    Exec {
        name: String,
        program: PathBuf,
        args: Vec<String>,
        cwd: PathBuf,
        env: Vec<(String, String)>,
    },
    /// A file copied, creating the target's folder.
    Copy {
        name: String,
        from: PathBuf,
        to: PathBuf,
    },
}

impl Step {
    pub fn name(&self) -> &str {
        match self {
            Self::Exec { name, .. } | Self::Copy { name, .. } => name,
        }
    }
}

/// How a run ended, written by the supervisor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Done {
    pub finished: u64,
    pub ok: bool,
    /// The step that failed and why.
    pub failure: Option<String>,
    pub score_after: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Running,
    Finished,
    Failed(String),
    Stopped,
    /// The supervisor is gone without saying how it ended.
    Lost,
}

impl Status {
    pub fn word(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Finished => "finished",
            Self::Failed(_) => "failed",
            Self::Stopped => "stopped",
            Self::Lost => "ended",
        }
    }
}

/// A run as found on disk.
#[derive(Debug, Clone)]
pub struct Run {
    pub dir: PathBuf,
    pub spec: Spec,
    pub done: Option<Done>,
    pub status: Status,
    pub progress: Option<Progress>,
}

impl Run {
    /// Reads the run in `dir`. `alive` tells whether a process id runs.
    pub fn read(dir: &Path, alive: impl Fn(u32) -> bool) -> Result<Self, String> {
        let spec: Spec = read_json(&dir.join(SPEC))?;
        let done: Option<Done> = read_json(&dir.join(DONE)).ok();

        let status = match (&done, dir.join(STOPPED).is_file()) {
            (Some(d), _) if d.ok => Status::Finished,
            (Some(d), _) => Status::Failed(d.failure.clone().unwrap_or_default()),
            (None, true) => Status::Stopped,
            (None, false) => match pid(dir) {
                Some(p) if alive(p) => Status::Running,
                // the pid is written just after the start: give it a moment
                None if crate::clock::now() < spec.started + 10 => Status::Running,
                _ => Status::Lost,
            },
        };
        let progress = Progress::of_logs(&log_paths(dir, &spec));

        Ok(Self {
            dir: dir.to_path_buf(),
            spec,
            done,
            status,
            progress,
        })
    }

    pub fn is_running(&self) -> bool {
        self.status == Status::Running
    }

    /// The logs of its commands, in order.
    pub fn logs(&self) -> Vec<PathBuf> {
        log_paths(&self.dir, &self.spec)
    }

    pub fn pid(&self) -> Option<u32> {
        pid(&self.dir)
    }

    /// Notes that the user stopped it, so it does not read as lost.
    pub fn mark_stopped(&self) -> Result<(), String> {
        write_text(&self.dir.join(STOPPED), "stopped by the user\n")
    }
}

/// Every run under `runs`, newest first. Folders that do not read are
/// skipped.
pub fn list(runs: &Path, alive: impl Fn(u32) -> bool) -> Vec<Run> {
    let Ok(entries) = std::fs::read_dir(runs) else {
        return Vec::new();
    };

    let mut found: Vec<Run> = entries
        .filter_map(Result::ok)
        .filter_map(|e| Run::read(&e.path(), &alive).ok())
        .collect();
    found.sort_by(|a, b| (b.spec.started, &b.spec.id).cmp(&(a.spec.started, &a.spec.id)));

    found
}

/// Writes a new run's folder and spec. The folder must not exist yet.
pub fn create(runs: &Path, spec: &Spec) -> Result<PathBuf, String> {
    let dir = runs.join(&spec.id);
    if dir.exists() {
        return Err(format!("{} exists already", dir.display()));
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;

    for (name, text) in &spec.files {
        write_text(&dir.join(name), text)?;
    }
    let json = serde_json::to_string_pretty(spec).map_err(|e| e.to_string())?;
    write_text(&dir.join(SPEC), &json)?;

    Ok(dir)
}

pub fn write_pid(dir: &Path, pid: u32) -> Result<(), String> {
    write_text(&dir.join(PID), &pid.to_string())
}

pub fn write_done(dir: &Path, done: &Done) -> Result<(), String> {
    let json = serde_json::to_string_pretty(done).map_err(|e| e.to_string())?;
    write_text(&dir.join(DONE), &json)
}

pub fn read_spec(dir: &Path) -> Result<Spec, String> {
    read_json(&dir.join(SPEC))
}

/// `<dir>/<name>.log`.
pub fn log_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.log"))
}

fn log_paths(dir: &Path, spec: &Spec) -> Vec<PathBuf> {
    spec.phases
        .iter()
        .flatten()
        .filter(|s| matches!(s, Step::Exec { .. }))
        .map(|s| log_path(dir, s.name()))
        .collect()
}

fn pid(dir: &Path) -> Option<u32> {
    std::fs::read_to_string(dir.join(PID)).ok()?.trim().parse().ok()
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn write_text(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// A fresh folder under the temp dir, removed by the caller.
    pub fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mow-tui-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    pub fn spec(id: &str, started: u64) -> Spec {
        Spec {
            id: id.to_owned(),
            kind: "lns".to_owned(),
            title: "a pass".to_owned(),
            started,
            score_before: 1.0,
            data: PathBuf::from("data"),
            instances: PathBuf::from("instances"),
            files: vec![(
                "note.txt".to_owned(),
                "hello
"
                .to_owned(),
            )],
            phases: vec![vec![Step::Exec {
                name: "lns".to_owned(),
                program: PathBuf::from("mow"),
                args: vec!["lns".to_owned()],
                cwd: PathBuf::from("."),
                env: Vec::new(),
            }]],
        }
    }

    #[test]
    fn a_run_reads_back_with_its_status() {
        let runs = scratch("runs-status");
        let old = create(&runs, &spec("old", 100)).unwrap();
        let new = create(&runs, &spec("new", 200)).unwrap();
        assert_eq!(
            std::fs::read_to_string(new.join("note.txt")).unwrap(),
            "hello
"
        );

        write_pid(&old, 7).unwrap();
        write_pid(&new, 8).unwrap();
        let listed = list(&runs, |pid| pid == 8);
        assert_eq!(listed[0].spec.id, "new");
        assert_eq!(listed[0].status, Status::Running);
        assert_eq!(listed[1].status, Status::Lost);

        Run::read(&new, |_| true).unwrap().mark_stopped().unwrap();
        assert_eq!(Run::read(&new, |_| true).unwrap().status, Status::Stopped);

        let done = Done {
            finished: 300,
            ok: false,
            failure: Some("lns: exit 1".to_owned()),
            score_after: None,
        };
        write_done(&old, &done).unwrap();
        assert_eq!(
            Run::read(&old, |_| false).unwrap().status,
            Status::Failed("lns: exit 1".to_owned())
        );
        assert!(create(&runs, &spec("old", 1)).is_err());

        std::fs::remove_dir_all(runs).unwrap();
    }
}
