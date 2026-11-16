//! The cutter-split measures of `diagnose` on small hand-made lawns: two
//! tours over a rectangle, side by side or overlapping.

use mow_core::{BitGrid, Cutter, Point, Problem, Route, Solution};
use mow_report::diagnose::{diagnose, Breakdown};

/// A `width` by `height` rectangle of cells from the origin, mowed by `k`
/// cutters of the given cells.
fn rectangle(width: i32, height: i32, cutter: &[(i32, i32)], k: usize) -> Problem {
    let cells: Vec<Point> = (0..height)
        .flat_map(|y| (0..width).map(move |x| Point::new(x, y)))
        .collect();
    let cutter: Vec<Point> = cutter.iter().map(|&(x, y)| Point::new(x, y)).collect();

    let region = BitGrid::from_cells(&cells).unwrap().unwrap();
    let cutter = Cutter::new(BitGrid::from_cells(&cutter).unwrap().unwrap()).unwrap();

    Problem::new("rectangle", region, cutter, k).unwrap()
}

/// Diagnoses the tours, each given by its corners.
fn run(problem: &Problem, tours: &[&[(i32, i32)]]) -> Breakdown {
    let routes = tours
        .iter()
        .map(|corners| Route::new(corners.iter().map(|&(x, y)| Point::new(x, y)).collect()).unwrap())
        .collect();

    diagnose(problem, &Solution::new("rectangle", routes)).unwrap()
}

/// Around the `x0..=x1` columns of an eight by two rectangle, one cell cutter.
fn loop_over(x0: i32, x1: i32) -> [(i32, i32); 4] {
    [(x0, 0), (x1, 0), (x1, 1), (x0, 1)]
}

#[test]
fn one_cutter_has_no_border_ends_and_no_overlap() {
    let problem = rectangle(8, 2, &[(0, 0)], 1);
    let result = run(&problem, &[&loop_over(0, 7)]);
    let split = &result.split;

    // three lanes (right, up, left), all ending at the lawn's edge
    assert_eq!(split.lane_ends.counts, [6, 0, 0]);
    // the dead step down closes the tour: a turn, half to each edge end
    assert_eq!(split.lane_ends.waste, [1.0, 0.0, 0.0]);
    assert_eq!(split.cross(), 0.0);
    assert_eq!(split.shared_cells, 0);
    assert_eq!(split.imbalance(), 0.0);
}

#[test]
fn halves_side_by_side_end_lanes_at_their_border() {
    let problem = rectangle(8, 2, &[(0, 0)], 2);
    let result = run(&problem, &[&loop_over(0, 3), &loop_over(4, 7)]);
    let split = &result.split;

    // per tour: the right and left lanes each meet the other half once
    assert_eq!(split.lane_ends.counts, [8, 0, 4]);
    // every lane step mows new cells; only the closing dead steps waste
    // length, the left half's at the lawn's edge, the right half's at the
    // border, where its tour starts and ends
    assert_eq!(split.lane_ends.waste, [1.0, 0.0, 1.0]);
    assert_eq!(split.cross(), 0.0);
    assert_eq!(split.shared_cells, 0);
    assert_eq!(split.tour_lengths, [8, 8]);
}

#[test]
fn a_shared_column_is_cross_tour_overlap() {
    let problem = rectangle(8, 2, &[(0, 0)], 2);
    let result = run(&problem, &[&loop_over(0, 4), &loop_over(4, 7)]);
    let split = &result.split;

    // the second tour steps onto (4, 1), which the first swept: a dead step
    // over the other's cells; its step back onto its own start is its own
    assert_eq!(split.shared_cells, 2);
    assert_eq!(split.dead_cross, 1.0);
    assert_eq!(split.near_resweep_cross + split.far_resweep_cross, 0.0);
    assert_eq!(split.imbalance(), 1.0);
}

#[test]
fn overlapping_bands_resweep_the_other_tour_in_either_order() {
    // a two by two cutter, bands of rows 0..=1 and 1..=2 over six columns
    let problem = rectangle(6, 3, &[(0, 0), (1, 0), (0, 1), (1, 1)], 2);
    let lower: &[(i32, i32)] = &[(0, 0), (4, 0)];
    let upper: &[(i32, i32)] = &[(0, 1), (4, 1)];

    for tours in [[lower, upper], [upper, lower]] {
        let result = run(&problem, &tours);
        let split = &result.split;

        // four steps right, each one new cell and one of the other's
        assert_eq!(split.shared_cells, 6);
        assert_eq!(split.far_resweep_cross, 2.0);
        assert_eq!(split.near_resweep_cross + split.dead_cross, 0.0);
        // the overlap is part of the far resweep, which keeps its meaning
        assert!(result.parts[3] >= split.far_resweep_cross);
        assert_eq!(split.lane_ends.counts[2], 0);
    }
}

#[test]
fn full_json_adds_split_and_keeps_the_rest() {
    let problem = rectangle(8, 2, &[(0, 0)], 2);
    let result = run(&problem, &[&loop_over(0, 3), &loop_over(4, 7)]);

    let plain = result.to_json().dump();
    let full = result.to_full_json().dump();
    let parsed: serde_json::Value = serde_json::from_str(&full).unwrap();

    assert!(full.starts_with(plain.trim_end_matches('}').trim_end()));
    assert_eq!(parsed["split"]["lane_ends_border"], 4);
    assert_eq!(parsed["split"]["cross"], 0.0);
}
