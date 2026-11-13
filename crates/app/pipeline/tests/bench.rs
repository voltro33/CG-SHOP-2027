//! The single walk path on real bench instances, with the real lane segments
//! and order search, and doubles for split, repair and improve. Without
//! repair a few cells may stay unmowed, but never more than the segments
//! alone leave, since the routes drive every segment. Needs `data/` (not in
//! git); skipped without it.

use std::path::PathBuf;
use std::time::Duration;

use mow_core::{
    lower_bound, Axis, Budget, Decomposer, Improver, NoObserver, Orderer, Problem, Repairer, Route,
    SolveError, Splitter, Walk,
};
use mow_coverage::uncovered_count;
use mow_decompose::LaneSegments;
use mow_order::IteratedLocalSearch;
use mow_pipeline::{solve, Plugins, SolveConfig, Strategy};

/// Small enough for a debug build, all three cutter counts.
const INSTANCES: [&str; 5] = [
    "fjs3649_17k_1x7",
    "dka1376_23k_1p7",
    "lta3140_54k_2x5",
    "srpg133_44k_2l5",
    "bgf4475_195k_3l3",
];

fn instance_path(uid: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../data/instances/challenge_v0/instances")
        .join(format!("{uid}.instance.json"))
}

/// Cuts a walk at its corners into at most k pieces of whole edges;
/// neighbouring pieces share their cut corner, so no position is lost.
struct AtCorners;

impl Splitter for AtCorners {
    fn label(&self) -> String {
        "corners".into()
    }

    fn split(&self, walk: &Walk, k: usize) -> Result<Vec<Walk>, SolveError> {
        let corners = walk.corners();
        let edges = corners.len() - 1;
        let per_piece = edges.div_ceil(k).max(1);

        if edges == 0 {
            return Ok(vec![walk.clone()]);
        }

        let pieces = (0..edges)
            .step_by(per_piece)
            .map(|first| Walk::new(corners[first..=(first + per_piece).min(edges)].to_vec()))
            .collect::<Result<_, _>>()?;

        Ok(pieces)
    }
}

/// Leaves walks and routes as they are.
struct Nothing;

impl Repairer for Nothing {
    fn label(&self) -> String {
        "none".into()
    }

    fn repair_walk(&self, _problem: &Problem, walk: Walk) -> Result<Walk, SolveError> {
        Ok(walk)
    }

    fn repair(&self, _problem: &Problem, routes: Vec<Route>) -> Result<Vec<Route>, SolveError> {
        Ok(routes)
    }
}

impl Improver for Nothing {
    fn label(&self) -> String {
        "none".into()
    }

    fn improve_walk(&self, _problem: &Problem, walk: Walk, _budget: Budget) -> Result<Walk, SolveError> {
        Ok(walk)
    }

    fn improve(
        &self,
        _problem: &Problem,
        routes: Vec<Route>,
        _budget: Budget,
    ) -> Result<Vec<Route>, SolveError> {
        Ok(routes)
    }
}

#[test]
fn single_walk_covers_bench_instances() {
    let present: Vec<&str> = INSTANCES
        .into_iter()
        .filter(|uid| instance_path(uid).exists())
        .collect();
    if present.is_empty() {
        println!("skipped: the bench instances are missing");
        return;
    }

    for uid in present {
        let problem = mow_io::load_problem(&instance_path(uid)).expect("instance loads");

        for (method, axis) in [("segments-h", Axis::Horizontal), ("segments-v", Axis::Vertical)] {
            check(&problem, method, axis);
        }
    }
}

fn check(problem: &Problem, method: &str, axis: Axis) {
    // a short order search keeps a debug run quick
    let config = SolveConfig::new(method)
        .with_param("seed", 1.0)
        .with_param("kick_seconds", 0.2)
        .with_param("time_limit", 0.0);
    let kick_seconds = config.param_or("kick_seconds", 0.0);
    let orderer = move |_: usize| -> Box<dyn Orderer> {
        Box::new(IteratedLocalSearch {
            kick_seconds,
            ..IteratedLocalSearch::default()
        })
    };

    let decomposer = LaneSegments::new(axis);
    let plugins = Plugins {
        decomposer: &decomposer,
        orderer: &orderer,
        splitter: &AtCorners,
        assigner: None,
        partitioning: None,
        repairer: &Nothing,
        improver: &Nothing,
        observer: &NoObserver,
    };

    let strategy = Strategy::of(&config).unwrap();
    let attempt = solve(problem, &plugins, &config, strategy, Duration::from_secs(1));

    let uid = problem.uid();
    let solution = attempt.solution.as_ref().expect("routes were built");
    assert_eq!(solution.routes().len(), problem.k(), "{uid} {method}");
    assert!(solution.objective() >= lower_bound(problem), "{uid} {method}");

    let missed = uncovered_count(problem, solution.routes()).unwrap();
    let segments_miss = segments_alone_miss(problem, &decomposer);
    assert!(
        missed <= segments_miss,
        "{uid} {method}: {missed} > {segments_miss}"
    );

    let expected: Vec<String> = Some(format!("{missed} uncovered cells"))
        .filter(|_| missed > 0)
        .into_iter()
        .collect();
    assert_eq!(attempt.errors, expected, "{uid} {method}");

    println!(
        "{uid} {method}: objective {} in {:.2} s, {missed} cells left to repair, segments alone miss {segments_miss}",
        solution.objective(),
        attempt.seconds
    );
}

/// Cells no segment sweeps, each segment driven out and back on its own.
fn segments_alone_miss(problem: &Problem, decomposer: &dyn Decomposer) -> usize {
    let routes: Vec<Route> = decomposer
        .decompose(problem)
        .unwrap()
        .iter()
        .map(|s| Route::new(vec![s.start(), s.end()]).unwrap())
        .collect();

    uncovered_count(problem, &routes).unwrap()
}
