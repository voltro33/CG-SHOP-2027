//! Small hand made problems for the unit tests.

use mow_core::{BitGrid, Cutter, Point, Problem, Segment};

/// A region from row runs `(y, x_min, x_max)`, mowed by a cutter with the
/// given cells (relative to its center).
pub(crate) fn problem(runs: &[(i32, i32, i32)], cutter: &[(i32, i32)]) -> Problem {
    let cells: Vec<Point> = runs
        .iter()
        .flat_map(|&(y, a, b)| (a..=b).map(move |x| Point::new(x, y)))
        .collect();
    let region = BitGrid::from_cells(&cells).unwrap().unwrap();

    let offsets: Vec<Point> = cutter.iter().map(|&c| c.into()).collect();
    let cutter = Cutter::new(BitGrid::from_cells(&offsets).unwrap().unwrap()).unwrap();

    Problem::new("test", region, cutter, 1).unwrap()
}

/// The cells of a centered `side x side` square, `side` odd.
pub(crate) fn square(side: i32) -> Vec<(i32, i32)> {
    let half = side / 2;

    (-half..=half)
        .flat_map(|y| (-half..=half).map(move |x| (x, y)))
        .collect()
}

/// An X of five cells: the middle row is a single cell, the others have a gap.
pub(crate) fn x_cutter() -> Vec<(i32, i32)> {
    vec![(-1, -1), (1, -1), (0, 0), (-1, 1), (1, 1)]
}

/// A `width x height` rectangle of row runs at the origin.
pub(crate) fn rectangle(width: i32, height: i32) -> Vec<(i32, i32, i32)> {
    (0..height).map(|y| (y, 0, width - 1)).collect()
}

pub(crate) fn horizontal(segments: &[(i32, i32, i32)]) -> Vec<Segment> {
    segments
        .iter()
        .map(|&(y, a, b)| Segment::horizontal(y, a, b).unwrap())
        .collect()
}

pub(crate) fn vertical(segments: &[(i32, i32, i32)]) -> Vec<Segment> {
    segments
        .iter()
        .map(|&(x, a, b)| Segment::vertical(x, a, b).unwrap())
        .collect()
}
