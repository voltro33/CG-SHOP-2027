//! The partitioned path: `_tours_partitioned` and `partition.solve_in_bands`.

use std::time::{Duration, Instant};

use mow_core::{Budget, Problem, Route};

use crate::paths::build_walk;
use crate::plugins::Partitioning;
use crate::solve::{pad_with_idle, At, Run};

/// The best routes of up to `rounds` rebalancing rounds, one route per
/// band, then improved together until the deadline, since bands overlap a
/// little at their borders.
pub(crate) fn partitioned(run: &Run, rounds: usize) -> Result<Vec<Route>, String> {
    let problem = run.problem;
    let partitioning = run
        .plugins
        .partitioning
        .ok_or("the partitioned strategy needs a partitioner")?;

    // each round solves all k bands, so one band gets a slice of the time
    let rounds = rounds.max(1);
    let slices = rounds.saturating_mul(2 * problem.k());
    let band_time = run.remaining().div_f64(slices as f64);

    let routes = solve_in_bands(run, partitioning, rounds, band_time)?;

    let routes = run
        .plugins
        .improver
        .improve(problem, routes, run.budget())
        .at("improve")?;
    run.observe_routes(problem, "improve", &routes);

    let home = routes.first().ok_or("improve dropped every route")?.start();
    pad_with_idle(routes, problem.k(), home)
}

/// Shares start equal; after each round a band whose route came out long
/// gets a smaller share. The round with the shortest longest route wins.
/// Always runs at least one round.
fn solve_in_bands(
    run: &Run,
    partitioning: Partitioning,
    rounds: usize,
    band_time: Duration,
) -> Result<Vec<Route>, String> {
    let k = run.problem.k();
    let mut shares = vec![1.0; k];
    let mut best: Option<(u64, Vec<Route>)> = None;

    for round in 0..rounds {
        if round > 0 && run.out_of_time() {
            break;
        }

        let bands = partitioning
            .partitioner
            .bands(run.problem, &shares)
            .at("partition")?;
        if bands.is_empty() {
            return Err(format!("{} produced no bands", partitioning.partitioner.label()));
        }

        let routes = bands
            .iter()
            .map(|band| solve_band(run, band, band_time))
            .collect::<Result<Vec<Route>, String>>()?;

        let lengths: Vec<u64> = routes.iter().map(|r| r.length().max(1)).collect();
        let longest = lengths.iter().copied().max().unwrap_or(1);
        if best.as_ref().map_or(true, |(length, _)| longest < *length) {
            best = Some((longest, routes));
        }

        // bands may have merged away, then shares no longer line up
        if bands.len() != k {
            break;
        }

        shares = (partitioning.rebalance)(&shares, &lengths);
    }

    let (_, routes) = best.ok_or("no band round ran")?;
    Ok(routes)
}

/// A band as a single-cutter problem: its walk, closed, improved for the
/// band's slice of time.
fn solve_band(run: &Run, band: &Problem, band_time: Duration) -> Result<Route, String> {
    let walk = build_walk(run, band)?;

    let budget = Budget {
        deadline: Instant::now() + band_time,
        seed: run.seed,
    };
    let route = walk.closed().at("close")?;
    let routes = run
        .plugins
        .improver
        .improve(band, vec![route], budget)
        .at("improve")?;
    run.observe_routes(band, "band", &routes);

    let mut routes = routes.into_iter();
    match (routes.next(), routes.next()) {
        (Some(route), None) => Ok(route),
        _ => Err("improve must keep a band's single route".to_string()),
    }
}
