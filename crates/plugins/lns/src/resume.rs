//! A search that continues where the last run of the same instance left
//! off, and leaves its own state for the next.

use std::path::Path;
use std::time::Instant;

use mow_core::{Problem, Route};

use crate::error::LnsError;
use crate::saved::{same_shape, Saved};
use crate::search::{default_wheels, search, LnsOptions, LnsOutcome};
use crate::weights::Wheel;

/// How far above the stored best a saved walk may end and still be
/// continued, as a share of the best. The temperature scales with the
/// whole objective, so on large instances the walk drifts well above the
/// best from one run to the next. In the pass of 2026-11-16, 17 of 25
/// walks within 0.36% of their best got under it, and 0 of 14 walks 0.46%
/// or more above it did.
pub const DRIFT: f64 = 0.005;

/// Searches `problem` until `deadline` and saves the state in `dir`.
///
/// It continues from the saved walk if the stored best is still the one
/// the last run saw (`stored_best`, the objective of `best`) and the walk
/// stands within [`DRIFT`] of it, else from `best`. Weights come from the
/// saved state, else from the instances of the same cutter shape, else all
/// equal.
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

    let fresh = saved.as_ref().filter(|s| continues(s, stored_best));
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

/// Whether the saved walk is worth continuing: it saw the same stored best
/// and has not drifted more than [`DRIFT`] above it.
fn continues(saved: &Saved, stored_best: u64) -> bool {
    let near = saved.current_objective as f64 <= stored_best as f64 * (1.0 + DRIFT);

    saved.stored_best == stored_best && near
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
