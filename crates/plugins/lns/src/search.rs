//! The search loop: draw a destroy and a recreate operator by their
//! weights, try the move under a simulated annealing threshold, book the
//! verdict, keep the best solution seen.
//!
//! The temperature falls geometrically with the time used, from
//! `temperature` times the starting objective to a hundredth of that.

use std::time::Instant;

use mow_core::{Point, Problem, Route, Walk};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use crate::destroy::Destroy;
use crate::error::LnsError;
use crate::moves::Tried;
use crate::recreate::Recreate;
use crate::tours::{cost, Tours};
use crate::weights::{Verdict, Wheel};

/// The end temperature, against the start.
const COOLING: f64 = 0.01;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LnsOptions {
    pub seed: u64,
    /// Start temperature as a share of the objective: a move this much
    /// worse than the current solution is kept with probability `1 / e`.
    pub temperature: f64,
}

impl Default for LnsOptions {
    fn default() -> Self {
        Self {
            seed: 0,
            temperature: 0.0003,
        }
    }
}

/// What one search did.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SearchStats {
    pub moves: u64,
    pub skipped: u64,
    pub kept: u64,
    pub bests: u64,
    pub start_objective: u64,
    pub best_objective: u64,
    pub seconds: f64,
}

pub struct LnsOutcome {
    /// The shortest solution seen.
    pub best: Vec<Route>,
    /// Where the walk stood at the end, to continue from.
    pub current: Vec<Route>,
    pub destroy: Wheel,
    pub recreate: Wheel,
    pub stats: SearchStats,
}

/// Fresh wheels, every operator at the same weight.
pub fn default_wheels() -> (Wheel, Wheel) {
    let destroy = Wheel::new(Destroy::ALL.iter().map(|d| d.name()));
    let recreate = Wheel::new(Recreate::ALL.iter().map(|r| r.name()));
    (destroy, recreate)
}

/// Searches from `routes`, which must cover every cell, until `deadline`.
pub fn search(
    problem: &Problem,
    routes: &[Route],
    deadline: Instant,
    options: &LnsOptions,
    wheels: (Wheel, Wheel),
) -> Result<LnsOutcome, LnsError> {
    let started = Instant::now();
    let mut tours = Tours::new(problem, routes)?;
    let mut rng = SmallRng::seed_from_u64(options.seed);
    let (mut destroy, mut recreate) = wheels;

    let start = tours.objective();
    let mut stats = SearchStats {
        start_objective: start,
        best_objective: start,
        ..SearchStats::default()
    };
    let mut best: Vec<Vec<Point>> = tours.tours.clone();
    let mut best_key = (start, cost(&tours.lengths()));

    let t0 = options.temperature.max(0.0) * start as f64;
    let total = deadline
        .saturating_duration_since(started)
        .as_secs_f64()
        .max(1e-9);
    let several = tours.tours.len() > 1;

    while Instant::now() < deadline {
        let used = started.elapsed().as_secs_f64() / total;
        let temperature = t0 * COOLING.powf(used.min(1.0));
        let threshold = -temperature * (1.0 - rng.gen::<f64>()).ln();

        let d = destroy.draw(rng.gen(), |i| several || Destroy::ALL[i] != Destroy::Transfer);
        let r = recreate.draw(rng.gen(), |_| true);
        let tried = tours.try_move(Destroy::ALL[d], Recreate::ALL[r], threshold, deadline, &mut rng)?;

        let verdict = match tried {
            Tried::Skipped => {
                stats.skipped += 1;
                continue;
            }
            Tried::Rejected => Verdict::Rejected,
            Tried::Kept { delta } => {
                stats.kept += 1;
                let key = (tours.objective(), cost(&tours.lengths()));
                if key < best_key {
                    best_key = key;
                    best.clone_from(&tours.tours);
                    stats.bests += 1;
                    Verdict::Best
                } else if delta < 0.0 {
                    Verdict::Better
                } else if delta > 0.0 {
                    Verdict::Accepted
                } else {
                    Verdict::Rejected
                }
            }
        };

        stats.moves += 1;
        for (wheel, i) in [(&mut destroy, d), (&mut recreate, r)] {
            wheel.record(i, verdict);
            wheel.tick();
        }
    }

    stats.best_objective = best_key.0;
    stats.seconds = started.elapsed().as_secs_f64();

    Ok(LnsOutcome {
        best: closed_routes(&best)?,
        current: tours.routes()?,
        destroy,
        recreate,
        stats,
    })
}

fn closed_routes(tours: &[Vec<Point>]) -> Result<Vec<Route>, LnsError> {
    tours
        .iter()
        .map(|t| Ok(Walk::from_positions(t)?.closed()?))
        .collect()
}
