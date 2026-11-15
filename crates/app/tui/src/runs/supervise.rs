//! The supervisor: `mow __supervise <dir>` runs a run's phases, records how
//! it ended and the score after, and [`start`] launches it detached.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use super::{create, log_path, process, read_spec, write_done, write_pid, Done, Spec, Step};
use crate::clock::now;
use crate::data::{Lawn, Paths};

/// The hidden `mow` command that runs a supervisor.
pub const COMMAND: &str = "__supervise";

/// Creates the run's folder and starts its supervisor, `mow __supervise`,
/// detached.
pub fn start(runs: &Path, spec: &Spec, mow: &Path) -> Result<PathBuf, String> {
    let dir = create(runs, spec)?;
    let args = [COMMAND.to_owned(), dir.to_string_lossy().into_owned()];

    let pid = process::spawn_detached(mow, &args, &dir, &dir.join("supervisor.log"))?;
    write_pid(&dir, pid)?;

    Ok(dir)
}

/// Runs every phase of the run in `dir`, then writes `done.json`.
pub fn supervise(dir: &Path) -> Result<(), String> {
    let spec = read_spec(dir)?;
    let failure = run_phases(dir, &spec).err();

    let paths = Paths {
        root: dir.to_path_buf(),
        data: spec.data.clone(),
        instances: spec.instances.clone(),
        mow: PathBuf::new(),
    };
    let done = Done {
        finished: now(),
        ok: failure.is_none(),
        failure,
        score_after: Lawn::load(&paths).ok().map(|lawn| lawn.score()),
    };

    write_done(dir, &done)
}

/// Each phase's steps side by side; a phase with a failed step ends the run
/// once the others in it finish.
fn run_phases(dir: &Path, spec: &Spec) -> Result<(), String> {
    for phase in &spec.phases {
        let mut children: Vec<(String, Child)> = Vec::new();
        let mut failures: Vec<String> = Vec::new();

        for step in phase {
            match step {
                Step::Exec { .. } => match launch(dir, step) {
                    Ok(child) => children.push((step.name().to_owned(), child)),
                    Err(e) => failures.push(e),
                },
                Step::Copy { name, from, to } => {
                    if let Err(e) = copy(from, to) {
                        failures.push(format!("{name}: {e}"));
                    }
                }
            }
        }

        for (name, mut child) in children {
            match child.wait() {
                Ok(status) if status.success() => {}
                Ok(status) => failures.push(format!("{name}: {status}")),
                Err(e) => failures.push(format!("{name}: {e}")),
            }
        }

        if !failures.is_empty() {
            return Err(failures.join("; "));
        }
    }

    Ok(())
}

fn launch(dir: &Path, step: &Step) -> Result<Child, String> {
    let Step::Exec {
        name,
        program,
        args,
        cwd,
        env,
    } = step
    else {
        return Err("not a command".to_owned());
    };

    let log = log_path(dir, name);
    let out = File::create(&log).map_err(|e| format!("{name}: cannot create {}: {e}", log.display()))?;
    let err = out.try_clone().map_err(|e| format!("{name}: {e}"))?;

    Command::new(program)
        .args(args)
        .current_dir(cwd)
        .envs(env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::null())
        .stdout(out)
        .stderr(err)
        .spawn()
        .map_err(|e| format!("{name}: cannot start {}: {e}", program.display()))
}

fn copy(from: &Path, to: &Path) -> Result<(), String> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }

    std::fs::copy(from, to)
        .map(drop)
        .map_err(|e| format!("cannot copy {} to {}: {e}", from.display(), to.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runs::tests::scratch;
    use crate::runs::{Run, Status};

    /// A command that succeeds or fails on every platform.
    fn shell(name: &str, ok: bool, dir: &Path) -> Step {
        let (program, args) = match (cfg!(windows), ok) {
            (true, true) => ("cmd", vec!["/C", "echo 1/1 ^| 1.0 jobs/min, 1 bests, 0 failed"]),
            (true, false) => ("cmd", vec!["/C", "exit 3"]),
            (false, true) => ("sh", vec!["-c", "echo '1/1 | 1.0 jobs/min, 1 bests, 0 failed'"]),
            (false, false) => ("sh", vec!["-c", "exit 3"]),
        };

        Step::Exec {
            name: name.to_owned(),
            program: PathBuf::from(program),
            args: args.into_iter().map(str::to_owned).collect(),
            cwd: dir.to_path_buf(),
            env: Vec::new(),
        }
    }

    fn spec(id: &str, phases: Vec<Vec<Step>>) -> Spec {
        Spec {
            phases,
            ..crate::runs::tests::spec(id, now())
        }
    }

    #[test]
    fn phases_run_in_order_and_a_failure_stops_the_rest() {
        let runs = scratch("supervise");
        let note = runs.join("note.txt");
        std::fs::write(&note, "x").unwrap();

        let good = create(
            &runs,
            &spec(
                "good",
                vec![
                    vec![shell("a", true, &runs), shell("b", true, &runs)],
                    vec![Step::Copy {
                        name: "copy".to_owned(),
                        from: note.clone(),
                        to: runs.join("out").join("note.txt"),
                    }],
                ],
            ),
        )
        .unwrap();
        supervise(&good).unwrap();

        let run = Run::read(&good, |_| false).unwrap();
        assert_eq!(run.status, Status::Finished);
        assert!(runs.join("out").join("note.txt").is_file());
        assert_eq!(run.progress.map(|p| (p.done, p.total, p.bests)), Some((2, 2, 2)));

        let bad = create(
            &runs,
            &spec(
                "bad",
                vec![
                    vec![shell("fails", false, &runs)],
                    vec![shell("never", true, &runs)],
                ],
            ),
        )
        .unwrap();
        supervise(&bad).unwrap();

        let run = Run::read(&bad, |_| false).unwrap();
        assert!(matches!(run.status, Status::Failed(ref why) if why.starts_with("fails:")));
        assert!(!log_path(&bad, "never").exists());

        std::fs::remove_dir_all(runs).unwrap();
    }
}
