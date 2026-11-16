//! LNS jobs in the miner: a job whose method is [`METHOD`] runs the ruin
//! and recreate search of `mow-lns` on the stored best of its instance,
//! continuing from `<out>/state/<uid>.json`.
//!
//! A shorter result goes through the official verifier and the store like
//! any mined one; its `meta.polished_from` names the job of the best it
//! started from. What every operator did goes to `<out>/state/operators.jsonl`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use mow_core::{Offer, Problem, Solution};
use mow_coverage::is_covered;
use mow_lns::{resume, LnsOptions, LnsOutcome, Wheel};
use mow_pipeline::SolveConfig;
use mow_store::JobRecord;

use super::job::{duration, failure, offer, record, Context};
use super::progress::Outcome;
use crate::grid::{Instance, Job};
use crate::meta::meta_field;
use crate::official::clip;

/// The method name of an LNS job.
pub const METHOD: &str = "lns";

/// A round number no mining round uses (polish is -2).
pub const LNS_ROUND: i32 = -3;

/// Where the per instance states and the operator log live, under `<out>`.
pub const STATE_DIR: &str = "state";
pub const OPERATOR_LOG: &str = "operators.jsonl";

/// `lns/temperature=T[/pass=N][/seed=N]`. Each pass is a new job id, so a
/// later run of the same search is never skipped as done.
pub fn config(temperature: f64, pass: u32, seed: u64) -> SolveConfig {
    let mut config = SolveConfig::new(METHOD).with_param("temperature", temperature);

    if pass != 0 {
        config = config.with_param("pass", f64::from(pass));
    }
    if seed != 0 {
        config = config.with_param("seed", seed as f64);
    }

    config
}

/// One LNS job per instance, `seconds[i]` for `instances[i]`. The longest
/// start first, so the threads finish close together; among equals the
/// biggest instance first.
pub fn lns_jobs(instances: &[Instance], config: &SolveConfig, seconds: &[f64]) -> Vec<Job> {
    let mut jobs: Vec<Job> = instances
        .iter()
        .zip(seconds)
        .map(|(instance, &seconds)| Job {
            instance: instance.uid.clone(),
            cells: instance.cells,
            round: LNS_ROUND,
            config: config.clone(),
            time_limit: seconds,
        })
        .collect();

    jobs.sort_by(|a, b| {
        b.time_limit
            .total_cmp(&a.time_limit)
            .then_with(|| b.cells.cmp(&a.cells))
            .then_with(|| a.instance.cmp(&b.instance))
    });
    jobs
}

/// Searches from the stored best of `job`'s instance until its time is up.
pub(super) fn run(job: &Job, context: &Context, problem: &Problem) -> (JobRecord, Outcome) {
    let started = Instant::now();
    let failed =
        |seconds: f64, error: String| (failure(job, context, seconds, clip(&error)), Outcome::Failed);

    let (file, state) = match (
        context.store.best_file(&job.instance),
        state_dir(context, &job.instance),
    ) {
        (Ok(Some(file)), Ok(state)) => (file, state),
        (Ok(None), _) => return failed(0.0, "no stored best to search from".to_owned()),
        (Err(error), _) | (_, Err(error)) => return failed(0.0, error.to_string()),
    };
    let best = match file.to_solution() {
        Ok(best) => best,
        Err(error) => return failed(0.0, error.to_string()),
    };
    let started_from = meta_field(&file, "job").unwrap_or_else(|| "?".to_owned());

    let time_limit = context.time.unwrap_or(job.time_limit);
    let options = LnsOptions {
        seed: job.config.seed(),
        temperature: job
            .config
            .param_or("temperature", LnsOptions::default().temperature),
    };
    let deadline = started + duration(time_limit);
    let searched = resume(
        problem,
        best.routes(),
        best.objective(),
        &state,
        deadline,
        &options,
    );
    let seconds = started.elapsed().as_secs_f64();

    let outcome = match searched {
        Ok(outcome) => outcome,
        Err(error) => return failed(seconds, error.to_string()),
    };
    report(&job.instance, &outcome, &state);

    let solution = Solution::new(problem.uid(), outcome.best);
    match is_covered(problem, solution.routes()) {
        Ok(true) => {}
        Ok(false) => return failed(seconds, "the searched tours miss cells".to_owned()),
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
        Some(started_from),
    ) {
        Ok(Offer::Best) => (record, Outcome::Best),
        Ok(_) => (record, Outcome::Done),
        Err(error) => failed(seconds, error),
    }
}

/// `<out>/state`, found next to the store's `best/` folder.
fn state_dir(context: &Context, uid: &str) -> Result<PathBuf, mow_core::PortError> {
    let best = context.store.best_path(uid)?;
    let out = best.parent().and_then(Path::parent).unwrap_or(Path::new("."));

    Ok(out.join(STATE_DIR))
}

/// One stderr line per search, and one operator log line.
fn report(uid: &str, outcome: &LnsOutcome, state: &Path) {
    let s = outcome.stats;
    eprintln!(
        "lns {uid}: {} -> {} in {} moves, {} kept, {} new bests",
        s.start_objective, s.best_objective, s.moves, s.kept, s.bests
    );

    let line = serde_json::json!({
        "instance": uid,
        "start": s.start_objective,
        "best": s.best_objective,
        "moves": s.moves,
        "seconds": s.seconds,
        "operators": tallies(&outcome.destroy).chain(tallies(&outcome.recreate)).collect::<Vec<_>>(),
    });

    let appended = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(state.join(OPERATOR_LOG))
        .and_then(|mut log| log.write_all(format!("{line}\n").as_bytes()));
    if let Err(error) = appended {
        eprintln!("lns {uid}: cannot log the operators: {error}");
    }
}

fn tallies(wheel: &Wheel) -> impl Iterator<Item = serde_json::Value> + '_ {
    wheel.operators.iter().map(|op| {
        serde_json::json!({
            "name": op.name,
            "weight": op.weight,
            "tries": op.tally.tries,
            "kept": op.tally.accepted,
            "better": op.tally.better,
            "best": op.tally.best,
        })
    })
}
