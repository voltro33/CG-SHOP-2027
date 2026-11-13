//! The single walk and grouped paths of `pipeline.py`, and `build_walk`,
//! which the partitioned path shares.

use std::time::Instant;

use mow_core::{Axis, Budget, Closure, Event, ModelError, Order, Point, Problem, Route, Segment, Walk};

use crate::solve::{pad_with_idle, At, Run};

/// Construct plus repair (`build_walk`): decompose, order the segments,
/// drive them in order, repair. The open walk that comes back sweeps every
/// cell of `problem` if the repairer did its job.
///
/// The order is closed for a single cutter, since its walk becomes a tour
/// and the way home counts too, and open otherwise (`walk_segments`).
/// Ordering gets the construction time, not the job's.
pub(crate) fn build_walk(run: &Run, problem: &Problem) -> Result<Walk, String> {
    let plugins = run.plugins;
    let segments = segments(run, problem)?;

    let closure = if problem.k() == 1 {
        Closure::Closed
    } else {
        Closure::Open
    };
    let orderer = (plugins.orderer)(segments.len());
    let budget = Budget::new(run.construction.walk_time(segments.len()), run.seed);

    let order = orderer.order(&segments, closure, budget).at("order")?;
    plugins.observer.observe(&Event::Ordered {
        problem,
        segments: &segments,
        order: &order,
    });

    let walk = drive(&order, &segments).at("walk")?;
    observe_walk(run, problem, "walk", &walk);

    let walk = plugins.repairer.repair_walk(problem, walk).at("repair")?;
    observe_walk(run, problem, "repair", &walk);

    Ok(walk)
}

/// One walk, improved with half the time left, split into k pieces, each
/// closed, then all improved until the deadline (`_tours_single_walk`).
pub(crate) fn single_walk(run: &Run) -> Result<Vec<Route>, String> {
    let (problem, plugins) = (run.problem, run.plugins);
    let walk = build_walk(run, problem)?;

    // polish the single walk first so the split balances the pruned work
    let walk_budget = Budget {
        deadline: Instant::now() + run.remaining() / 2,
        seed: run.seed,
    };
    let walk = plugins
        .improver
        .improve_walk(problem, walk, walk_budget)
        .at("improve")?;
    observe_walk(run, problem, "improve", &walk);

    let home = walk.start();
    let pieces = plugins.splitter.split(&walk, problem.k()).at("split")?;
    let routes = pieces
        .iter()
        .map(Walk::closed)
        .collect::<Result<Vec<Route>, _>>()
        .at("close")?;
    run.observe_routes(problem, "split", &routes);

    let routes = plugins
        .improver
        .improve(problem, routes, run.budget())
        .at("improve")?;
    run.observe_routes(problem, "improve", &routes);

    pad_with_idle(routes, problem.k(), home)
}

/// k routes straight from the segments, repaired together, improved until
/// the deadline (`_tours_grouped`).
pub(crate) fn grouped(run: &Run) -> Result<Vec<Route>, String> {
    let (problem, plugins) = (run.problem, run.plugins);
    let assigner = plugins.assigner.ok_or("the grouped strategy needs an assigner")?;
    let segments = segments(run, problem)?;

    let orderer = (plugins.orderer)(segments.len());
    let budget = Budget::new(run.construction.groups_time(segments.len()), run.seed);
    let routes = assigner
        .assign(problem, &segments, orderer.as_ref(), budget)
        .at("assign")?;
    if routes.is_empty() {
        return Err(format!("{} produced no routes", assigner.label()));
    }
    run.observe_routes(problem, "assign", &routes);

    let routes = plugins.repairer.repair(problem, routes).at("repair")?;
    let routes: Vec<Route> = routes.into_iter().map(Route::normalized).collect();
    run.observe_routes(problem, "repair", &routes);

    let routes = plugins
        .improver
        .improve(problem, routes, run.budget())
        .at("improve")?;
    run.observe_routes(problem, "improve", &routes);

    let home = routes.first().ok_or("improve dropped every route")?.start();
    pad_with_idle(routes, problem.k(), home)
}

/// The walk through every segment in order. Links bend horizontal first,
/// except when every segment is vertical: Python builds vertical lanes on
/// the mirrored problem and mirrors the walk back, so their links bend
/// vertical first, and the cells a link happens to sweep depend on that.
fn drive(order: &Order, segments: &[Segment]) -> Result<Walk, ModelError> {
    let waypoints = order.waypoints(segments);

    if !segments.iter().all(|s| s.axis() == Axis::Vertical) {
        return Walk::through(&waypoints);
    }

    let mirrored: Vec<Point> = waypoints.iter().map(|p| p.transposed()).collect();
    let walk = Walk::through(&mirrored)?;

    Walk::new(walk.corners().iter().map(|p| p.transposed()).collect())
}

/// The decomposer's segments; none at all is an error, as an empty walk
/// is in Python.
fn segments(run: &Run, problem: &Problem) -> Result<Vec<Segment>, String> {
    let decomposer = run.plugins.decomposer;
    let segments = decomposer.decompose(problem).at("decompose")?;

    if segments.is_empty() {
        return Err(format!("{} produced no segments", decomposer.label()));
    }

    run.plugins.observer.observe(&Event::Segments {
        problem,
        segments: &segments,
    });

    Ok(segments)
}

fn observe_walk(run: &Run, problem: &Problem, step: &str, walk: &Walk) {
    run.plugins.observer.observe(&Event::Walk { problem, step, walk });
}
