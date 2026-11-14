//! A search that continues where the last run of the same instance left
//! off, and leaves its own state for the next.

use std::path::Path;
use std::time::Instant;

use mow_core::{Problem, Route};

use crate::error::LnsError;
use crate::saved::{same_shape, Saved};
use crate::search::{default_wheels, search, LnsOptions, LnsOutcome};
use crate::weights::Wheel;

/// Searches `problem` until `deadline` and saves the state in `dir`.
///
/// It continues from the saved walk if the stored best is still the one
/// the last run saw (`stored_best`, the objective of `best`), else from
/// `best`. Weights come from the saved state, else from the instances of
/// the same cutter shape, else all equal.
pub fn resume(
    problem: &Problem,
    best: &[Route],
    stored_best: u64,
    dir: &Path,
    deadline: Instant,
    options: &LnsOptions,
) -> Result<LnsOutcome, LnsError> {
    let saved = Saved::load(dir, problem.uid())?;
    let wheels = wheels_for(problem.uid(), saved.as_ref(), dir);

    let fresh = saved.as_ref().filter(|s| s.stored_best == stored_best);
    let start = match fresh.map(Saved::routes) {
        Some(Ok(routes)) => routes,
        _ => best.to_vec(),
    };

    let outcome = match search(problem, &start, deadline, options, wheels.clone()) {
        Ok(outcome) => outcome,
        // a saved walk that no longer covers falls back to the best
        Err(LnsError::NotCovered(_)) if fresh.is_some() => search(problem, best, deadline, options, wheels)?,
        Err(error) => return Err(error),
    };

    let state = Saved {
        uid: problem.uid().to_owned(),
        runs: saved.as_ref().map_or(0, |s| s.runs) + 1,
        seconds: saved.as_ref().map_or(0.0, |s| s.seconds) + outcome.stats.seconds,
        stored_best: stored_best.min(outcome.stats.best_objective),
        current_objective: current_objective(&outcome.current),
        current: Saved::corners_of(&outcome.current),
        destroy: outcome.destroy.clone(),
        recreate: outcome.recreate.clone(),
    };
    state.save(dir)?;

    Ok(outcome)
}

fn wheels_for(uid: &str, saved: Option<&Saved>, dir: &Path) -> (Wheel, Wheel) {
    let (destroy, recreate) = default_wheels();
    if let Some(saved) = saved {
        return (
            destroy.with_weights(&saved.destroy),
            recreate.with_weights(&saved.recreate),
        );
    }

    let others = same_shape(dir, uid);
    if others.is_empty() {
        return (destroy, recreate);
    }

    let names = |w: &Wheel| w.operators.iter().map(|o| o.name.clone()).collect::<Vec<_>>();
    let (ds, rs): (Vec<Wheel>, Vec<Wheel>) = others.into_iter().unzip();
    (
        Wheel::pooled(&names(&destroy), &ds),
        Wheel::pooled(&names(&recreate), &rs),
    )
}

fn current_objective(routes: &[Route]) -> u64 {
    routes.iter().map(Route::length).max().unwrap_or(0)
}
