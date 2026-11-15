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
    let mut baring_checked = 0;

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

        let bare = applied.unswept_around(removed.iter().copied());
        if before == 0 {
            assert_eq!(bare.len(), lost, "case {case}");

            let mut baring = counts.clone();
            let bared = baring.remove_positions_baring(removed.iter().copied()).unwrap();
            assert_eq!(bared, bare, "case {case}");
            assert_eq!(baring.uncovered_count(), applied.uncovered_count(), "case {case}");
            baring_checked += 1;
        }
    }

    assert!(baring_checked > 50, "only {baring_checked} cases started covered");
}

#[test]
fn unswept_around_lists_the_cells_a_removal_left_bare() {
    let p = rectangle(6, 3, square(3), 1);
    let lane = route(&[(1, 1), (4, 1)]);
    let mut counts = VisitCounts::from_routes(&p, std::slice::from_ref(&lane)).unwrap();

    assert!(counts.unswept_around([Point::new(4, 1)]).is_empty());

    counts.remove_positions([Point::new(4, 1)]).unwrap();
    let mut bare = counts.unswept_around([Point::new(4, 1), Point::new(4, 1)]);
    bare.sort_by_key(|c| (c.y, c.x));

    assert_eq!(bare, vec![Point::new(5, 0), Point::new(5, 1), Point::new(5, 2)]);
    assert!(!counts.all_swept_around([Point::new(4, 1)]));
}

/// A 20 x 14 rectangle with a hole and a notch, so footprints cross cells
/// off the region.
fn holey(cutter: mow_core::Cutter) -> Problem {
    let mut cells: Vec<Point> = (0..14)
        .flat_map(|y| (0..20).map(move |x| Point::new(x, y)))
        .collect();
    let hole = |c: &Point| (6..9).contains(&c.x) && (4..7).contains(&c.y);
    let notch = |c: &Point| c.x > 15 && c.y < 3;
    cells.retain(|c| !hole(c) && !notch(c));
    let region = mow_core::BitGrid::from_cells(&cells).unwrap().unwrap();

    Problem::new("holey", region, cutter, 1).unwrap()
}

/// Unit steps from a random start along up to four random legs, some of
/// them doubling back over anchors already walked.
fn random_walk(rng: &mut Rng) -> Vec<Point> {
    let mut at = Point::new(rng.below(24) as i32 - 2, rng.below(18) as i32 - 2);
    let mut walk = vec![at];

    for _ in 0..1 + rng.below(4) {
        let (dx, dy) = [(1, 0), (-1, 0), (0, 1), (0, -1)][rng.below(4)];
        for _ in 0..rng.below(9) {
            at = at.shifted(dx, dy);
            walk.push(at);
        }
    }

    walk
}

fn row_major(mut cells: Vec<Point>) -> Vec<Point> {
    cells.sort_unstable_by_key(|c| (c.y, c.x));
    cells
}

#[test]
fn stamping_whole_walks_matches_one_position_at_a_time() {
    let mut rng = Rng::new(11);
    let wide_x = common::cutter(&[
        (-2, -2),
        (2, -2),
        (-1, -1),
        (1, -1),
        (0, 0),
        (-1, 1),
        (1, 1),
        (-2, 2),
        (2, 2),
    ]);
    let mut scratch = Scratch::default();

    for cutter in [square(3), x_shape(), wide_x, common::plus()] {
        let p = holey(cutter);
        let mut runs = VisitCounts::new(&p).unwrap();
        let mut single = runs.clone();
        let mut live: Vec<Vec<Point>> = Vec::new();

        for step in 0..300 {
            if live.is_empty() || rng.below(3) > 0 {
                let walk = random_walk(&mut rng);
                runs.add_positions(walk.iter().copied()).unwrap();
                walk.iter().for_each(|&q| single.add_positions([q]).unwrap());
                live.push(walk);
            } else {
                let walk = live.swap_remove(rng.below(live.len()));
                let together = runs.remove_positions_baring(walk.iter().copied()).unwrap();
                let one_by_one: Vec<Point> = walk
                    .iter()
                    .flat_map(|&q| single.remove_positions_baring([q]).unwrap())
                    .collect();
                assert_eq!(row_major(together), row_major(one_by_one), "step {step}");
            }

            assert_eq!(runs.uncovered_count(), single.uncovered_count(), "step {step}");
            for y in -3..17 {
                for x in -3..23 {
                    let q = [Point::new(x, y)];
                    assert_eq!(runs.visits(q[0]), single.visits(q[0]));
                    assert_eq!(
                        runs.cells_lost_if_removed(&mut scratch, q),
                        single.cells_lost_if_removed(&mut scratch, q),
                        "step {step} at {x},{y}"
                    );
                }
            }
        }
    }
}
