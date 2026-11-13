//! Small cases worked out by hand, and random routes against a cell by cell
//! reference sweep.

mod common;

use std::collections::HashSet;

use common::{plus, pts, rectangle, reference_uncovered, route, square, x_shape, Rng};
use mow_core::{Point, Problem, Route};
use mow_coverage::{anchors, is_covered, swept, uncovered, uncovered_count};

fn uncovered_set(problem: &Problem, routes: &[Route]) -> HashSet<Point> {
    uncovered(problem, routes).unwrap().cells().collect()
}

#[test]
fn square_lanes_cover_a_rectangle() {
    // 6 x 3 cells, side 3: one lane through the middle row does it
    let p = rectangle(6, 3, square(3), 1);
    let lane = route(&[(1, 1), (4, 1)]);

    assert!(is_covered(&p, std::slice::from_ref(&lane)).unwrap());
    assert_eq!(uncovered_count(&p, &[lane]).unwrap(), 0);
}

#[test]
fn a_short_lane_misses_the_far_columns() {
    let p = rectangle(6, 3, square(3), 1);
    let lane = route(&[(1, 1), (3, 1)]);

    let expected: HashSet<Point> = pts(&[(5, 0), (5, 1), (5, 2)]).into_iter().collect();
    assert_eq!(uncovered_set(&p, &[lane]), expected);
}

#[test]
fn single_point_route_sweeps_its_footprint() {
    let p = rectangle(3, 3, square(3), 1);
    let still = route(&[(1, 1)]);

    assert!(is_covered(&p, std::slice::from_ref(&still)).unwrap());

    let anchors = anchors(&p, &[still]).unwrap();
    assert_eq!(anchors.cells().collect::<Vec<_>>(), pts(&[(1, 1)]));
    assert_eq!(anchors.frame(), p.anchor_frame().bounds);
}

#[test]
fn x_cutter_leaves_the_gaps_of_its_cross() {
    let p = rectangle(3, 3, x_shape(), 1);
    let still = route(&[(1, 1)]);

    let expected: HashSet<Point> = pts(&[(1, 0), (0, 1), (2, 1), (1, 2)]).into_iter().collect();
    assert_eq!(uncovered_set(&p, &[still]), expected);
}

#[test]
fn plus_cutter_misses_the_corners() {
    let p = rectangle(3, 3, plus(), 1);
    let still = route(&[(1, 1)]);

    let expected: HashSet<Point> = pts(&[(0, 0), (2, 0), (0, 2), (2, 2)]).into_iter().collect();
    assert_eq!(uncovered_set(&p, &[still]), expected);
}

#[test]
fn routes_far_outside_the_frame_are_clipped() {
    let p = rectangle(4, 4, square(3), 1);
    let far = route(&[(-1000, -1000), (1000, -1000), (1000, 1000), (-1000, 1000)]);

    // no anchor of this square route can touch the region
    assert!(anchors(&p, std::slice::from_ref(&far)).unwrap().is_empty());
    assert_eq!(uncovered_count(&p, &[far]).unwrap(), 16);
}

#[test]
fn routes_crossing_the_frame_keep_their_inside_part() {
    let p = rectangle(4, 3, square(3), 1);
    let crossing = route(&[(-50, 1), (50, 1)]);
    let vertical = route(&[(1, -50), (1, 50)]);

    assert!(is_covered(&p, std::slice::from_ref(&crossing)).unwrap());
    assert_eq!(
        uncovered_set(&p, &[vertical]),
        pts(&[(3, 0), (3, 1), (3, 2)]).into_iter().collect()
    );
}

#[test]
fn several_routes_union_their_sweeps() {
    let p = rectangle(10, 3, square(3), 2);
    let left = route(&[(1, 1), (3, 1)]);
    let right = route(&[(6, 1), (8, 1)]);

    assert_eq!(uncovered_set(&p, std::slice::from_ref(&left)).len(), 15);
    assert!(is_covered(&p, &[left, right]).unwrap());
}

#[test]
fn swept_spans_word_borders() {
    // 150 columns: the dilation has to carry bits across u64 words
    let p = rectangle(150, 5, square(5), 1);
    let lane = route(&[(2, 2), (147, 2)]);

    let anchors = anchors(&p, std::slice::from_ref(&lane)).unwrap();
    let swept = swept(&p, &anchors).unwrap();
    assert!(p.region().cells().all(|c| swept.contains(c)));
    assert!(is_covered(&p, &[lane]).unwrap());
}

/// A random orthogonal route with corners in `-3..size + 3`.
fn random_route(rng: &mut Rng, size: i32) -> Route {
    let span = (size + 6) as usize;
    let mut points = vec![Point::new(rng.below(span) as i32 - 3, rng.below(span) as i32 - 3)];

    for _ in 0..rng.below(6) {
        let last = *points.last().unwrap();
        let v = rng.below(span) as i32 - 3;
        points.push(if rng.below(2) == 0 {
            Point::new(v, last.y)
        } else {
            Point::new(last.x, v)
        });
    }

    Route::through(&points).unwrap()
}

#[test]
fn random_routes_match_the_reference_sweep() {
    let mut rng = Rng::new(7);
    let cutters = [square(3), square(4), x_shape(), plus(), square(1)];

    for case in 0..300 {
        let size = 5 + (case % 7) as i32 * 13;
        let cutter = cutters[case % cutters.len()].clone();
        let p = rectangle(size, size / 2 + 1, cutter, 1);

        let routes: Vec<Route> = (0..1 + rng.below(3))
            .map(|_| random_route(&mut rng, size))
            .collect();

        assert_eq!(
            uncovered_set(&p, &routes),
            reference_uncovered(&p, &routes),
            "case {case}"
        );
    }
}
