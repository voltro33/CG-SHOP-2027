//! `pipeline.solve`: one problem, one config, one attempt.

use std::any::Any;
use std::fmt::Display;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::{Duration, Instant};

use mow_core::{Budget, Event, Point, Problem, Route, Solution};
use mow_coverage::{is_covered, uncovered_count};

use crate::attempt::Attempt;
use crate::config::SolveConfig;
use crate::plugins::{seconds, Construction, Plugins, MAX_SECONDS};
use crate::strategy::Strategy;
use crate::{partitioned, paths};

/// Runs one config through the whole pipeline. Failures, panics of a
/// plugin included, come back as strings in [`Attempt::errors`], never as a
/// panic or an `Err`. The routes are checked with
/// [`mow_coverage::is_covered`] as a fast pre-check; the official verifier
/// runs later, outside this crate.
///
/// `time_limit` is the job's whole deadline, counted from the start, as in
/// Python. Ordering and assigning get their own budget ([`Construction`],
/// from the config's `time_limit` and `kick_seconds` params) which is not
/// capped by that deadline, so the time they take is time the improver
/// loses, and a partitioned run, which constructs once per band and round,
/// can overrun the deadline just as Python's could.
pub fn solve(
    problem: &Problem,
    plugins: &Plugins<'_>,
    config: &SolveConfig,
    strategy: Strategy,
    time_limit: Duration,
) -> Attempt {
    let started = Instant::now();
    let label = config.label();
    plugins.observer.observe(&Event::Started {
        problem,
        label: &label,
    });

    let run = Run {
        problem,
        plugins,
        construction: Construction::of(config),
        seed: config.seed(),
        deadline: started + time_limit.min(seconds(MAX_SECONDS)),
    };

    let routes =
        catch_unwind(AssertUnwindSafe(|| run.routes(strategy))).unwrap_or_else(|panic| Err(panicked(panic)));

    let (solution, errors) = match routes {
        Ok(routes) => {
            let (solution, errors) = finish(problem, routes);
            plugins.observer.observe(&Event::Finished {
                problem,
                solution: &solution,
            });
            (Some(solution), errors)
        }
        Err(error) => (None, vec![error]),
    };

    Attempt {
        label,
        solution,
        errors,
        seconds: started.elapsed().as_secs_f64(),
    }
}

/// One run in progress: what every path of the pipeline needs.
pub(crate) struct Run<'r, 'a> {
    pub problem: &'r Problem,
    pub plugins: &'r Plugins<'a>,
    pub construction: Construction,
    pub seed: u64,
    /// When improving must stop.
    pub deadline: Instant,
}

impl Run<'_, '_> {
    fn routes(&self, strategy: Strategy) -> Result<Vec<Route>, String> {
        match strategy {
            Strategy::SingleWalk => paths::single_walk(self),
            Strategy::Grouped => paths::grouped(self),
            Strategy::Partitioned { rounds } => partitioned::partitioned(self, rounds),
        }
    }

    /// Until the job's deadline.
    pub fn budget(&self) -> Budget {
        Budget {
            deadline: self.deadline,
            seed: self.seed,
        }
    }

    pub fn out_of_time(&self) -> bool {
        Instant::now() >= self.deadline
    }

    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }

    pub fn observe_routes(&self, problem: &Problem, step: &str, routes: &[Route]) {
        self.plugins.observer.observe(&Event::Routes {
            problem,
            step,
            routes,
        });
    }
}

/// Names the step an error came from: `step: message`.
pub(crate) trait At<T> {
    fn at(self, step: &str) -> Result<T, String>;
}

impl<T, E: Display> At<T> for Result<T, E> {
    fn at(self, step: &str) -> Result<T, String> {
        self.map_err(|error| format!("{step}: {error}"))
    }
}

/// Adds one point routes for cutters that got no work, since a solution
/// must always have exactly k routes (`_pad_with_idle`).
pub(crate) fn pad_with_idle(routes: Vec<Route>, k: usize, home: Point) -> Result<Vec<Route>, String> {
    let mut padded = routes;

    while padded.len() < k {
        padded.push(Route::new(vec![home]).at("pad")?);
    }

    Ok(padded)
}

/// The solution, and what the fast checks find wrong with it.
fn finish(problem: &Problem, routes: Vec<Route>) -> (Solution, Vec<String>) {
    let mut errors = Vec::new();

    if routes.len() != problem.k() {
        errors.push(format!("{} routes for {} cutters", routes.len(), problem.k()));
    }

    match is_covered(problem, &routes) {
        Ok(true) => {}
        Ok(false) => {
            let count = uncovered_count(problem, &routes).unwrap_or(0);
            errors.push(format!("{count} uncovered cells"));
        }
        Err(error) => errors.push(format!("coverage: {error}")),
    }

    (Solution::new(problem.uid(), routes), errors)
}

fn panicked(panic: Box<dyn Any + Send>) -> String {
    let message = panic
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown panic".to_string());

    format!("panic: {message}")
}
