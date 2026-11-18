//! The tasks that start, stop and merge: runs, bests, binaries and
//! experiments. Each returns the status line it leaves.

use std::path::{Path, PathBuf};

use crate::app::App;
use crate::bests::{self, Backup, Plan, Source};
use crate::binaries::{self, Commit};
use crate::experiments::{self, Choice, ARMS};
use crate::recipes::{self, runs_dir};
use crate::runs::{process, supervise, Run, Spec};
use crate::screens::experiments::{Binaries, Commits, ReportScreen};
use crate::screens::merge::{Bests, MergePlan};
use crate::screens::{Screen, Task};
use crate::viewer;

impl App {
    /// Runs a task that is not about looking at instances.
    pub(crate) fn work(&mut self, task: Task) -> Result<String, String> {
        match task {
            Task::StartRun(spec) => self.start(&spec),
            Task::StopRun(dir) => stop(&dir),
            Task::OpenFolder(dir) => viewer::open(&dir).map(|()| format!("opened {}", dir.display())),
            Task::OpenBests => self.open_bests(),
            Task::PlanMerge(source) => self.plan_merge(&source),
            Task::Merge(plan) => self.merge(&plan),
            Task::Undo(backup) => self.undo(&backup),
            Task::Pack => {
                let spec = recipes::pack_spec(&self.ctx.paths, &self.ctx.lawn)?;
                self.start(&spec)
            }
            Task::SaveBinary => self.save_binary(),
            Task::OpenCommits => {
                let list = binaries::commits(&self.ctx.paths, 30)?;
                self.stack.push(Screen::Commits(Commits::new(list)));
                Ok(String::new())
            }
            Task::Build(commit) => self.build(&commit),
            Task::StartExperiment(choice) => self.start_experiment(&choice),
            Task::OpenReport(dir) => self.open_report(dir),
            Task::MergeExperiment(dir) => self.merge_experiment(&dir),
            Task::Pick { slot, path } => {
                if let Some(Screen::Compare(compare)) = self.stack.last_mut() {
                    compare.set(slot, path.clone());
                }
                Ok(format!("{} is {}", slot.letter(), path.display()))
            }
            Task::StartCompare { a, b } => {
                let spec = recipes::compare_spec(&self.ctx.paths, &self.ctx.lawn, &a, &b);
                self.start(&spec)
                    .map(|started| format!("{started}; the page opens in the browser when it is done"))
            }
            other => Err(format!("{} is not handled here", other.busy())),
        }
    }

    fn start(&self, spec: &Spec) -> Result<String, String> {
        let dir = supervise::start(&runs_dir(&self.ctx.paths), spec, &self.ctx.paths.mow)?;
        Ok(format!(
            "started {}; follow it under Runs ({})",
            spec.title,
            dir.display()
        ))
    }

    fn open_bests(&mut self) -> Result<String, String> {
        let sources = bests::sources(&self.ctx.paths);
        let backups = bests::backups(&self.ctx.paths);
        let count = sources.len();

        self.stack.push(Screen::Bests(Bests::new(sources, backups)));
        Ok(format!("{count} folders to merge from"))
    }

    fn plan_merge(&mut self, source: &Source) -> Result<String, String> {
        let plan = bests::plan(source, &self.ctx.lawn)?;
        let message = format!("{} shorter tours, {:+.3} points", plan.changes.len(), plan.gain);

        self.stack.push(Screen::MergePlan(MergePlan::new(plan)));
        Ok(message)
    }

    fn merge(&mut self, plan: &Plan) -> Result<String, String> {
        let backup = bests::merge(&self.ctx.paths, plan)?;
        self.after_merge()?;

        Ok(format!(
            "merged {} tours, {:+.3} points; backed up in {}",
            plan.changes.len(),
            plan.gain,
            backup.display()
        ))
    }

    fn undo(&mut self, backup: &Backup) -> Result<String, String> {
        let (restored, skipped) = bests::undo(&self.ctx.paths, backup)?;
        self.after_merge()?;

        Ok(format!(
            "restored {restored} tours, {skipped} left as a newer tour replaced them"
        ))
    }

    /// The bests changed: read them again, close a merge preview, and give
    /// the Bests screen its new lists.
    fn after_merge(&mut self) -> Result<(), String> {
        self.reload()?;
        if matches!(self.stack.last(), Some(Screen::MergePlan(_))) {
            self.stack.pop();
        }

        let (sources, backups) = (bests::sources(&self.ctx.paths), bests::backups(&self.ctx.paths));
        for screen in &mut self.stack {
            if let Screen::Bests(b) = screen {
                b.replace(sources.clone(), backups.clone());
            }
        }
        Ok(())
    }

    fn save_binary(&mut self) -> Result<String, String> {
        let label = binaries::save_current(&self.ctx.paths)?;

        for screen in &mut self.stack {
            if let Screen::Binaries(b) = screen {
                *b = Binaries::new(&self.ctx);
            }
        }
        Ok(format!("saved the running build as {label}"))
    }

    fn build(&mut self, commit: &Commit) -> Result<String, String> {
        let spec = binaries::build_spec(&self.ctx.paths, &self.ctx.lawn, commit)?;
        self.start(&spec)
    }

    fn start_experiment(&mut self, choice: &Choice) -> Result<String, String> {
        let spec = experiments::create(&self.ctx.paths, &self.ctx.lawn, choice)?;
        self.start(&spec)
    }

    fn open_report(&mut self, dir: PathBuf) -> Result<String, String> {
        let (_, setup) = experiments::list(&self.ctx.paths)
            .into_iter()
            .find(|(d, _)| *d == dir)
            .ok_or_else(|| format!("{} is not an experiment", dir.display()))?;

        self.stack
            .push(Screen::Report(Box::new(ReportScreen::new(dir, setup))));
        Ok(String::new())
    }

    /// Merges each arm in turn: the second takes only what still beats the
    /// bests after the first.
    fn merge_experiment(&mut self, dir: &Path) -> Result<String, String> {
        let mut taken = Vec::new();

        for arm in ARMS {
            let best = dir.join(arm).join("best");
            let name = best
                .strip_prefix(&self.ctx.paths.root)
                .unwrap_or(&best)
                .to_string_lossy()
                .replace('\\', "/");
            let plan = bests::plan(&Source { name, dir: best }, &self.ctx.lawn)?;
            if plan.changes.is_empty() {
                continue;
            }

            bests::merge(&self.ctx.paths, &plan)?;
            self.reload()?;
            taken.push(format!("{arm} {} tours {:+.3}", plan.changes.len(), plan.gain));
        }

        Ok(match taken.is_empty() {
            true => "neither arm beats the stored bests".to_owned(),
            false => format!(
                "merged {}; score {:.3}",
                taken.join(", "),
                self.ctx.summary.all.points
            ),
        })
    }
}

/// Stops a run's supervisor and everything under it.
fn stop(dir: &Path) -> Result<String, String> {
    let pids = process::running_pids();
    let run = Run::read(dir, |pid| pids.contains(&pid))?;
    if !run.is_running() {
        return Ok("it was not running".to_owned());
    }

    let pid = run.pid().ok_or("it has no process id")?;
    process::stop_tree(pid)?;
    run.mark_stopped()?;

    Ok(format!("stopped {}", run.spec.title))
}
