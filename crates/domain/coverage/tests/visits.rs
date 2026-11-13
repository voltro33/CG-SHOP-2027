//! Incremental counts against full rechecks.

mod common;

use common::{rectangle, route, square, x_shape, Rng};
use mow_core::{Point, Problem, Route};
use mow_coverage::{uncovered_count, CoverageError, Scratch, VisitCounts};

/// Uncovered cells of every position of `routes` with `removed` taken out
/// once each, by a full sweep of what is left.
fn full_recheck(problem: &Problem, routes: &[Route], removed: &[Point]) -> usize {
    let mut left: Vec<Point> = routes.iter().flat_map(|r| r.positions()).collect();
    for p in removed {
        if let Some(i) = left.iter().position(|q| q == p) {
            left.swap_remove(i);
        }
    }

    // one single corner route per remaining position
    let stands: Vec<Route> = left.iter().map(|&p| Route::new(vec![p]).unwrap()).collect();
    if stands.is_empty() {
        return problem.num_cells();
    }
    uncovered_count(problem, &stands).unwrap()
}

#[test]
fn counts_track_coverage_as_positions_come_and_go() {
    let p = rectangle(6, 3, square(3), 1);
    let lane = route(&[(1, 1), (4, 1)]);
    let mut counts = VisitCounts::from_routes(&p, std::slice::from_ref(&lane)).unwrap();

    assert!(counts.is_covered());
    assert_eq!(counts.visits(Point::new(1, 1)), 1);
    assert_eq!(counts.visits(Point::new(2, 1)), 2);

    counts.remove_positions([Point::new(4, 1)]).unwrap();
    assert_eq!(counts.uncovered_count(), 3);

    counts.add_positions([Point::new(4, 1)]).unwrap();
    assert!(counts.is_covered());
}

#[test]
fn removing_an_unvisited_anchor_is_an_error() {
    let p = rectangle(6, 3, square(3), 1);
    let mut counts = VisitCounts::new(&p).unwrap();

    let err = counts.remove_positions([Point::new(2, 2)]).unwrap_err();
    assert_eq!(err, CoverageError::NotVisited(Point::new(2, 2)));
    assert_eq!(counts.uncovered_count(), 18);
}

#[test]
fn queries_count_anchors_visited_twice() {
    // the lane runs (1,1) -> (4,1) and back to (2,1): (2,1) and (3,1) are visited twice
    let p = rectangle(6, 3, square(3), 1);
    let lane = route(&[(1, 1), (4, 1)]);
    let counts = VisitCounts::from_routes(&p, &[lane]).unwrap();
    let mut scratch = Scratch::default();

    let end = Point::new(4, 1);
    assert_eq!(counts.cells_lost_if_removed(&mut scratch, [end]), 3);
    assert!(!counts.still_covered_without(&mut scratch, [end]));

    // (3,1) is visited twice: removing one visit keeps it, removing both loses column 4
    let once = [end, Point::new(3, 1)];
    assert_eq!(counts.cells_lost_if_removed(&mut scratch, once), 3);

    let twice = [end, Point::new(3, 1), Point::new(3, 1)];
    assert_eq!(counts.cells_lost_if_removed(&mut scratch, twice), 6);

    // a third removal of (3,1) is one more than it has, and changes nothing
    let thrice = [end, Point::new(3, 1), Point::new(3, 1), Point::new(3, 1)];
    assert_eq!(counts.cells_lost_if_removed(&mut scratch, thrice), 6);

    // outside the frame and unvisited anchors change nothing
    let nothing = [Point::new(-90, 4), Point::new(2, 2)];
    assert!(counts.still_covered_without(&mut scratch, nothing));
}

#[test]
fn the_query_leaves_no_trace_in_the_scratch() {
    let p = rectangle(6, 3, square(3), 1);
    let counts = VisitCounts::from_routes(&p, &[route(&[(1, 1), (4, 1)])]).unwrap();
    let mut scratch = Scratch::for_counts(&counts);

    for _ in 0..3 {
        assert_eq!(counts.cells_lost_if_removed(&mut scratch, [Point::new(4, 1)]), 3);
    }
}

#[test]
fn random_removals_match_a_full_recheck() {
    let mut rng = Rng::new(42);
    let cutters = [square(3), x_shape(), square(2)];

    for case in 0..150 {
        let p = rectangle(12, 7, cutters[case % cutters.len()].clone(), 1);
        let routes = vec![
            route(&[(1, 1), (10, 1), (10, 3), (1, 3), (1, 5), (10, 5), (10, 6), (1, 6)]),
            route(&[(0, 0), (11, 0), (11, 6), (0, 6)]),
        ];
        let counts = VisitCounts::from_routes(&p, &routes).unwrap();
        let mut scratch = Scratch::default();

        let all: Vec<Point> = routes.iter().flat_map(|r| r.positions()).collect();
        let start = rng.below(all.len());
        let len = 1 + rng.below(12);
        let removed: Vec<Point> = all.iter().cycle().skip(start).take(len).copied().collect();

        let before = counts.uncovered_count();
        let lost = counts.cells_lost_if_removed(&mut scratch, removed.iter().copied());
        assert_eq!(before + lost, full_recheck(&p, &routes, &removed), "case {case}");
        assert_eq!(
            counts.still_covered_without(&mut scratch, removed.iter().copied()),
            lost == 0
        );

        let mut applied = counts.clone();
        applied.remove_positions(removed.iter().copied()).unwrap();
        assert_eq!(applied.uncovered_count(), before + lost, "case {case}");
    }
}
