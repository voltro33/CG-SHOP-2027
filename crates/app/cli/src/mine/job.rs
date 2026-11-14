//! One worker: take a job, solve it, check it, store it, log it.

use std::any::Any;
use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Mutex;
use std::time::Duration;

use mow_core::{CpSolver, NoObserver, Offer, Solution};
use mow_pipeline::solve;
use mow_store::{DirStore, JobLog, JobRecord};

use super::progress::{Outcome, Progress};
use super::queue::{Queue, Task};
use crate::grid::Job;
use crate::meta::{solution_file, Made};
use crate::official::{clip, confirm, Official};
use crate::registry::Kit;

/// What every worker shares.
pub struct Context<'a> {
    pub store: &'a DirStore,
    pub log: &'a JobLog,
    pub official: &'a dyn Official,
    pub bests: Bests,
    pub version: String,
    pub kick_scale: f64,
    pub time: Option<f64>,
    pub progress: &'a Progress,
    /// Solves the window models of polish jobs; `None` fails them.
    pub cpsat: Option<&'a dyn CpSolver>,
}

/// The best stored objective per instance, read from the store once.
#[derive(Debug, Default)]
pub struct Bests {
    known: Mutex<HashMap<String, Option<u64>>>,
}

impl Bests {
    /// True if `objective` is shorter than the stored best, or there is none.
    pub fn beaten_by(&self, store: &DirStore, uid: &str, objective: u64) -> bool {
        let mut known = self.known.lock().unwrap_or_else(|p| p.into_inner());
        let best = *known.entry(uid.to_owned()).or_insert_with(|| {
            store.best_objective(uid).unwrap_or_else(|e| {
                eprintln!("cannot read the best of {uid}: {e}");
                None
            })
        });

        best.map_or(true, |best| objective < best)
    }

    pub fn improved(&self, uid: &str, objective: u64) {
        let mut known = self.known.lock().unwrap_or_else(|p| p.into_inner());
        let entry = known.entry(uid.to_owned()).or_insert(None);
        *entry = Some(entry.map_or(objective, |best| best.min(objective)));
    }
}

/// Takes jobs until the queue has none left. A panic anywhere in a job
/// costs that job only.
pub fn work(queue: &Queue, context: &Context) {
    while let Some(task) = queue.next() {
        let job = task.job.clone();

        let result = catch_unwind(AssertUnwindSafe(|| run(&task, context)));
        let (record, outcome) = result.unwrap_or_else(|panic| {
            let record = failure(&job, context, 0.0, format!("panic: {}", panic_message(&panic)));
            (record, Outcome::Failed)
        });

        // the problem goes as soon as no job holds it
        drop(task);

        if let Err(error) = context.log.append(&record) {
            eprintln!("cannot log {}: {error}", record.job_id);
        }
        context.progress.finished(&record, outcome);
    }
}

fn run(task: &Task, context: &Context) -> (JobRecord, Outcome) {
    let job = &task.job;
    let failed =
        |seconds: f64, error: String| (failure(job, context, seconds, clip(&error)), Outcome::Failed);

    let problem = match task.slot.problem(&task.path) {
        Ok(problem) => problem,
        Err(error) => return failed(0.0, error),
    };
    if job.config.method == super::polish::METHOD {
        return super::polish::run(job, context, &problem);
    }

    let kit = match Kit::new(&job.config, context.kick_scale) {
        Ok(kit) => kit,
        Err(error) => return failed(0.0, error),
    };

    let time_limit = context.time.unwrap_or(job.time_limit);
    let attempt = solve(
        &problem,
        &kit.plugins(&NoObserver),
        &job.config,
        kit.strategy(),
        duration(time_limit),
    );
    let seconds = attempt.seconds;

    let (Some(solution), Some(objective)) = (&attempt.solution, attempt.objective()) else {
        let error = attempt
            .errors
            .first()
            .cloned()
            .unwrap_or_else(|| "infeasible".into());
        return failed(seconds, error);
    };

    let record = record(job, context, Some(objective), seconds, None);
    if !context.bests.beaten_by(context.store, &job.instance, objective) {
        return (record, Outcome::Done);
    }

    match offer(job, context, solution, objective, time_limit, seconds, None) {
        Ok(Offer::Best) => (record, Outcome::Best),
        Ok(_) => (record, Outcome::Done),
        Err(error) => failed(seconds, error),
    }
}

/// Verifies a new best the official way, then stores it.
pub(super) fn offer(
    job: &Job,
    context: &Context,
    solution: &Solution,
    objective: u64,
    time_limit: f64,
    seconds: f64,
    polished_from: Option<String>,
) -> Result<Offer, String> {
    let made = Made {
        job: job.job_id(&context.version),
        config: job.label(),
        round: Some(job.round),
        kick_scale: context.kick_scale,
        time_limit,
        seconds,
        polished_from,
    };
    let file = solution_file(solution, &made);

    confirm(context.official, &file, objective)?;

    let offer = context
        .store
        .offer_file(&file)
        .map_err(|e| format!("store: {e}"))?;
    if offer == Offer::Best {
        context.bests.improved(&job.instance, objective);
    }

    Ok(offer)
}

pub(super) fn record(
    job: &Job,
    context: &Context,
    objective: Option<u64>,
    seconds: f64,
    error: Option<String>,
) -> JobRecord {
    JobRecord {
        job_id: job.job_id(&context.version),
        instance: job.instance.clone(),
        round: job.round,
        label: job.label(),
        objective,
        seconds: (seconds * 100.0).round() / 100.0,
        error,
    }
}

pub(super) fn failure(job: &Job, context: &Context, seconds: f64, error: String) -> JobRecord {
    record(job, context, None, seconds, Some(error))
}

/// Seconds as a duration; negative or NaN is zero.
pub fn duration(seconds: f64) -> Duration {
    match seconds.is_finite() && seconds > 0.0 {
        true => Duration::from_secs_f64(seconds.min(1e9)),
        false => Duration::ZERO,
    }
}

fn panic_message(panic: &Box<dyn Any + Send>) -> String {
    panic
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown panic".to_owned())
}
