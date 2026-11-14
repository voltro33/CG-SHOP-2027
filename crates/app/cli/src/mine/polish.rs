//! Polish jobs in the miner (port of `polish.run_polish_job` and
//! `polish_all_jobs`): a job whose method is [`METHOD`] polishes the stored
//! best of its instance with CP-SAT instead of building a solution.
//!
//! The best is read when the job starts, so it polishes whatever mining
//! left there. A shorter result goes through the official verifier and the
//! store like any mined one; its `meta.polished_from` names the job of the
//! best it started from.

use std::time::Instant;

use mow_core::{Offer, Problem, Solution};
use mow_coverage::is_covered;
use mow_pipeline::SolveConfig;
use mow_polish::{
    polish_routes, polish_seconds, PolishOptions, DEFAULT_SIDE, DEFAULT_WINDOW_SECONDS, POLISH_ROUND,
};
use mow_store::JobRecord;

use super::job::{duration, failure, offer, record, Context};
use super::progress::Outcome;
use crate::grid::{Instance, Job};
use crate::meta::meta_field;
use crate::official::clip;

/// The method name of a polish job, as Python logs it.
pub const METHOD: &str = "polish";

/// `polish/side=20/window_seconds=3[/aim=1][/seed=N]`. Aimed jobs and other
/// seeds get their own labels, so the log never counts them as done.
pub fn config(side: i32, window_seconds: f64, seed: u64, aim: bool) -> SolveConfig {
    let mut config = SolveConfig::new(METHOD)
        .with_param("side", side as f64)
        .with_param("window_seconds", window_seconds);

    if aim {
        config = config.with_param("aim", 1.0);
    }
    if seed != 0 {
        config = config.with_param("seed", seed as f64);
    }

    config
}

/// The default polish config, as `polish_all_jobs` makes it.
pub fn default_config(seed: u64, aim: bool) -> SolveConfig {
    config(DEFAULT_SIDE, DEFAULT_WINDOW_SECONDS, seed, aim)
}

/// What a polish config asks of [`polish_routes`]. One CP-SAT thread per
/// window: the miner runs one job per thread.
pub fn options_of(config: &SolveConfig) -> PolishOptions {
    PolishOptions {
        side: config.param_or("side", DEFAULT_SIDE as f64) as i32,
        window_seconds: config.param_or("window_seconds", DEFAULT_WINDOW_SECONDS),
        workers: 1,
        seed: config.seed(),
        aim: config.param_or("aim", 0.0) != 0.0,
    }
}

/// One polish job per instance, biggest first. Each runs for `seconds`, or
/// for [`polish_seconds`] of its size.
pub fn polish_jobs(instances: &[Instance], config: &SolveConfig, seconds: Option<f64>) -> Vec<Job> {
    let mut jobs: Vec<Job> = instances
        .iter()
        .map(|instance| Job {
            instance: instance.uid.clone(),
            cells: instance.cells,
            round: POLISH_ROUND,
            config: config.clone(),
            time_limit: seconds.unwrap_or_else(|| polish_seconds(instance.cells)),
        })
        .collect();

    jobs.sort_by(|a, b| b.cells.cmp(&a.cells).then_with(|| a.instance.cmp(&b.instance)));

    jobs
}

/// Polishes the stored best of `job`'s instance until its time is up.
pub(super) fn run(job: &Job, context: &Context, problem: &Problem) -> (JobRecord, Outcome) {
    let started = Instant::now();
    let failed =
        |seconds: f64, error: String| (failure(job, context, seconds, clip(&error)), Outcome::Failed);

    let Some(solver) = context.cpsat else {
        return failed(0.0, "no CP-SAT solver for a polish job".to_owned());
    };
    let file = match context.store.best_file(&job.instance) {
        Ok(Some(file)) => file,
        Ok(None) => return failed(0.0, "no stored best to polish".to_owned()),
        Err(error) => return failed(0.0, error.to_string()),
    };
    let best = match file.to_solution() {
        Ok(best) => best,
        Err(error) => return failed(0.0, error.to_string()),
    };
    let polished_from = meta_field(&file, "job").unwrap_or_else(|| "?".to_owned());

    let time_limit = context.time.unwrap_or(job.time_limit);
    let deadline = started + duration(time_limit);
    let polished = polish_routes(problem, best.routes(), deadline, &options_of(&job.config), solver);
    let seconds = started.elapsed().as_secs_f64();

    let (routes, stats) = match polished {
        Ok(polished) => polished,
        Err(error) => return failed(seconds, error.to_string()),
    };
    eprintln!(
        "polish {}: {} windows, {} shorter, {} steps saved in {} passes",
        job.instance, stats.windows, stats.improved, stats.saved_steps, stats.passes
    );

    let solution = Solution::new(problem.uid(), routes);
    match is_covered(problem, solution.routes()) {
        Ok(true) => {}
        Ok(false) => return failed(seconds, "the polished tours miss cells".to_owned()),
        Err(error) => return failed(seconds, format!("coverage: {error}")),
    }

    let objective = solution.objective();
    let record = record(job, context, Some(objective), seconds, None);
    if !context.bests.beaten_by(context.store, &job.instance, objective) {
        return (record, Outcome::Done);
    }

    match offer(
        job,
        context,
        &solution,
        objective,
        time_limit,
        seconds,
        Some(polished_from),
    ) {
        Ok(Offer::Best) => (record, Outcome::Best),
        Ok(_) => (record, Outcome::Done),
        Err(error) => failed(seconds, error),
    }
}
