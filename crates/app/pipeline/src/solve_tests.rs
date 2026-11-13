//! Every strategy end to end on small rectangles, with test doubles.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use mow_core::{Budget, Closure, Order, Orderer, Point, Problem, Segment, SolveError};

use crate::config::SolveConfig;
use crate::solve::solve;
use crate::strategy::Strategy;
use crate::testing::{doubles, rectangle, Fails, NoSegments, Panics, Recorder, RowLanes, Snake};
use crate::{Attempt, Plugins};

const SECOND: Duration = Duration::from_secs(1);

fn run(problem: &Problem, plugins: &Plugins, strategy: Strategy) -> Attempt {
    solve(
        problem,
        plugins,
        &SolveConfig::new("segments-h"),
        strategy,
        SECOND,
    )
}

fn count(names: &[String], name: &str) -> usize {
    names.iter().filter(|n| *n == name).count()
}

#[test]
fn single_walk_steps_in_python_order() {
    let problem = rectangle(5, 4, 1);
    let recorder = Recorder::default();

    let attempt = run(&problem, &doubles(&RowLanes, &recorder), Strategy::SingleWalk);

    assert!(attempt.feasible(), "{:?}", attempt.errors);
    assert_eq!(attempt.label, "segments-h");
    assert_eq!(attempt.solution.unwrap().routes().len(), 1);
    assert_eq!(
        recorder.names(),
        [
            "started",
            "segments",
            "ordered",
            "walk:walk",
            "walk:repair",
            "walk:improve",
            "routes:split",
            "routes:improve",
            "finished"
        ]
    );
}

#[test]
fn single_walk_splits_into_k_covering_routes() {
    let problem = rectangle(6, 6, 2);
    let recorder = Recorder::default();

    let attempt = run(&problem, &doubles(&RowLanes, &recorder), Strategy::SingleWalk);

    assert!(attempt.feasible(), "{:?}", attempt.errors);
    assert_eq!(attempt.solution.unwrap().routes().len(), 2);
}

#[test]
fn single_walk_pads_with_idle_routes_at_home() {
    let problem = rectangle(4, 1, 3);
    let recorder = Recorder::default();

    let attempt = run(&problem, &doubles(&RowLanes, &recorder), Strategy::SingleWalk);
    let solution = attempt.solution.clone().unwrap();

    assert!(attempt.feasible(), "{:?}", attempt.errors);
    assert_eq!(solution.routes().len(), 3);
    assert_eq!(solution.routes()[1].corners(), [Point::new(0, 0)]);
    assert_eq!(solution.routes()[2].corners(), [Point::new(0, 0)]);
}

#[test]
fn grouped_assigns_repairs_improves_and_pads() {
    let problem = rectangle(5, 3, 2);
    let recorder = Recorder::default();

    let attempt = run(&problem, &doubles(&RowLanes, &recorder), Strategy::Grouped);

    assert!(attempt.feasible(), "{:?}", attempt.errors);
    assert_eq!(attempt.solution.unwrap().routes().len(), 2);
    assert_eq!(
        recorder.names(),
        [
            "started",
            "segments",
            "routes:assign",
            "routes:repair",
            "routes:improve",
            "finished"
        ]
    );
}

#[test]
fn grouped_needs_an_assigner() {
    let problem = rectangle(5, 3, 2);
    let recorder = Recorder::default();
    let plugins = Plugins {
        assigner: None,
        ..doubles(&RowLanes, &recorder)
    };

    let attempt = run(&problem, &plugins, Strategy::Grouped);

    assert!(attempt.solution.is_none());
    assert_eq!(attempt.errors, ["the grouped strategy needs an assigner"]);
}

#[test]
fn partitioned_runs_every_round_and_keeps_the_best() {
    let problem = rectangle(4, 8, 2);
    let recorder = Recorder::default();

    let attempt = run(
        &problem,
        &doubles(&RowLanes, &recorder),
        Strategy::Partitioned { rounds: 3 },
    );
    let names = recorder.names();

    assert!(attempt.feasible(), "{:?}", attempt.errors);
    assert_eq!(count(&names, "routes:band"), 6);
    assert_eq!(count(&names, "routes:improve"), 1);

    // equal shares give two 4 x 4 bands, snake routes of 18; skewed
    // rounds only get worse
    assert_eq!(attempt.objective(), Some(18));
    assert_eq!(attempt.solution.unwrap().routes().len(), 2);
}

#[test]
fn partitioned_stops_after_one_round_out_of_time() {
    let problem = rectangle(4, 8, 2);
    let recorder = Recorder::default();
    let plugins = doubles(&RowLanes, &recorder);
    let config = SolveConfig::new("segments-h");

    let attempt = solve(
        &problem,
        &plugins,
        &config,
        Strategy::Partitioned { rounds: 3 },
        Duration::ZERO,
    );

    assert!(attempt.feasible(), "{:?}", attempt.errors);
    assert_eq!(count(&recorder.names(), "routes:band"), 2);
}

#[test]
fn partitioned_needs_a_partitioner() {
    let problem = rectangle(4, 8, 2);
    let recorder = Recorder::default();
    let plugins = Plugins {
        partitioning: None,
        ..doubles(&RowLanes, &recorder)
    };

    let attempt = run(&problem, &plugins, Strategy::Partitioned { rounds: 1 });

    assert_eq!(attempt.errors, ["the partitioned strategy needs a partitioner"]);
}

#[test]
fn failures_become_errors_never_panics() {
    let problem = rectangle(4, 4, 1);
    let recorder = Recorder::default();

    let empty = run(&problem, &doubles(&NoSegments, &recorder), Strategy::SingleWalk);
    assert_eq!(empty.errors, ["none produced no segments"]);
    assert!(empty.solution.is_none());

    let panicked = run(&problem, &doubles(&Panics, &recorder), Strategy::SingleWalk);
    assert_eq!(panicked.errors, ["panic: broken invariant"]);

    let failing = Plugins {
        improver: &Fails,
        ..doubles(&RowLanes, &recorder)
    };
    let failed = run(&problem, &failing, Strategy::SingleWalk);
    assert_eq!(failed.errors, ["improve: fails ran out of time"]);
}

/// Covers the first region row only.
struct FirstRow;

impl mow_core::Decomposer for FirstRow {
    fn label(&self) -> String {
        "first".into()
    }

    fn decompose(&self, problem: &Problem) -> Result<Vec<Segment>, SolveError> {
        let b = problem.region_bounds();
        Ok(vec![Segment::horizontal(b.y_min, b.x_min, b.x_max)?])
    }
}

#[test]
fn uncovered_cells_are_an_error_but_keep_the_solution() {
    let problem = rectangle(4, 3, 1);
    let recorder = Recorder::default();

    let attempt = run(&problem, &doubles(&FirstRow, &recorder), Strategy::SingleWalk);

    assert_eq!(attempt.errors, ["8 uncovered cells"]);
    assert!(attempt.solution.is_some());
    assert_eq!(attempt.objective(), None);
    assert_eq!(recorder.names().last().unwrap(), "finished");
}

/// Remembers the budget and closure it was handed.
struct Spy(Arc<Mutex<Vec<(Budget, Closure)>>>);

impl Orderer for Spy {
    fn label(&self) -> String {
        "spy".into()
    }

    fn order(&self, segments: &[Segment], closure: Closure, budget: Budget) -> Result<Order, SolveError> {
        self.0.lock().unwrap().push((budget, closure));
        Snake.order(segments, closure, budget)
    }
}

#[test]
fn ordering_gets_its_own_time_and_the_config_seed() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let factory = |_: usize| -> Box<dyn Orderer> { Box::new(Spy(seen.clone())) };
    let recorder = Recorder::default();
    let plugins = Plugins {
        orderer: &factory,
        ..doubles(&RowLanes, &recorder)
    };
    let config = SolveConfig::new("segments-h").with_param("seed", 7.0);

    for (k, closure) in [(1, Closure::Closed), (2, Closure::Open)] {
        let attempt = solve(
            &rectangle(4, 4, k),
            &plugins,
            &config,
            Strategy::SingleWalk,
            Duration::ZERO,
        );
        assert!(attempt.feasible(), "{:?}", attempt.errors);

        let (budget, seen_closure) = seen.lock().unwrap().pop().unwrap();
        assert_eq!(seen_closure, closure);
        assert_eq!(budget.seed, 7);
        assert!(
            budget.remaining() > Duration::from_secs(10),
            "10 s plus the kick time"
        );
    }
}
