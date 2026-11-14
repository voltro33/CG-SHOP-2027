//! Turn-shape jobs in the miner: a job whose method is [`METHOD`] runs the
//! corner local search of `mow-turns` on the stored best of its instance,
//! the way a polish job runs CP-SAT windows on it.
//!
//! A shorter result goes through the official verifier and the store like
//! any mined one; its `meta.polished_from` names the job of the best it
//! started from.

use std::time::Instant;

use mow_core::{Offer, Problem, Solution};
use mow_coverage::is_covered;
use mow_pipeline::SolveConfig;
use mow_store::JobRecord;
use mow_turns::{turns_routes, TurnsOptions, TURNS_ROUND};

use super::job::{duration, failure, offer, record, Context};
use super::progress::Outcome;
use crate::grid::{Instance, Job};
use crate::meta::meta_field;
use crate::official::clip;

pub const METHOD: &str = "turns";

/// Every move on: the bit of move `m` (in the order of `mow_turns::MOVES`) is `1 << m`.
pub const ALL_MOVES: u32 = 0b1111;

/// `turns/window=6[/moves=M][/balance=0]`: the moves only when not all of
/// them run, the balance only when off.
pub fn config(window: usize, moves: u32, balance: bool) -> SolveConfig {
    let mut config = SolveConfig::new(METHOD).with_param("window", window as f64);

    if moves != ALL_MOVES {
        config = config.with_param("moves", f64::from(moves));
    }
    if !balance {
        config = config.with_param("balance", 0.0);
    }

    config
}

/// What a turns config asks of [`turns_routes`].
pub fn options_of(config: &SolveConfig) -> TurnsOptions {
    let defaults = TurnsOptions::default();
    let mask = config.param_or("moves", f64::from(ALL_MOVES)) as u32;

    TurnsOptions {
        window: config.param_or("window", defaults.window as f64) as usize,
        moves: std::array::from_fn(|m| mask & (1 << m) != 0),
        balance: config.param_or("balance", 1.0) != 0.0,
        ..defaults
    }
}

/// One turns job per instance, biggest first, `seconds` each.
pub fn turns_jobs(instances: &[Instance], config: &SolveConfig, seconds: f64) -> Vec<Job> {
    let mut jobs: Vec<Job> = instances
        .iter()
        .map(|instance| Job {
            instance: instance.uid.clone(),
            cells: instance.cells,
            round: TURNS_ROUND,
            config: config.clone(),
            time_limit: seconds,
        })
        .collect();

    jobs.sort_by(|a, b| b.cells.cmp(&a.cells).then_with(|| a.instance.cmp(&b.instance)));

    jobs
}

/// Searches the stored best of `job`'s instance until a pass saves nothing
/// or its time is up.
pub(super) fn run(job: &Job, context: &Context, problem: &Problem) -> (JobRecord, Outcome) {
    let started = Instant::now();
    let failed =
        |seconds: f64, error: String| (failure(job, context, seconds, clip(&error)), Outcome::Failed);

    let file = match context.store.best_file(&job.instance) {
        Ok(Some(file)) => file,
        Ok(None) => return failed(0.0, "no stored best to search from".to_owned()),
        Err(error) => return failed(0.0, error.to_string()),
    };
    let best = match file.to_solution() {
        Ok(best) => best,
        Err(error) => return failed(0.0, error.to_string()),
    };
    let started_from = meta_field(&file, "job").unwrap_or_else(|| "?".to_owned());

    let time_limit = context.time.unwrap_or(job.time_limit);
    let deadline = started + duration(time_limit);
    let searched = turns_routes(problem, best.routes(), deadline, options_of(&job.config));
    let seconds = started.elapsed().as_secs_f64();

    let (routes, stats) = match searched {
        Ok(searched) => searched,
        Err(error) => return failed(seconds, error.to_string()),
    };
    eprintln!(
        "turns {}: {} steps saved in {} passes, {} checks; {}",
        job.instance,
        stats.saved_steps(),
        stats.passes,
        stats.tries,
        stats.per_move()
    );

    let solution = Solution::new(problem.uid(), routes);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_name_the_moves_only_when_some_are_off() {
        assert_eq!(config(6, ALL_MOVES, true).label(), "turns/window=6");
        assert_eq!(
            config(4, 0b0011, false).label(),
            "turns/window=4/moves=3/balance=0"
        );
    }

    #[test]
    fn options_follow_the_config() {
        let options = options_of(&config(4, 0b0101, false));

        assert_eq!(options.window, 4);
        assert_eq!(options.moves, [true, false, true, false]);
        assert!(!options.balance);
        assert_eq!(
            options_of(&config(6, ALL_MOVES, true)),
            mow_turns::TurnsOptions::default()
        );
    }
}
