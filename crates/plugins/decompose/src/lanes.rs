//! Plain boustrophedon lanes, edge to edge (port of `construct/lanes.py`).

use mow_core::{Axis, Decomposer, Problem, RowExtent, Segment, SolveError};

use crate::axis::{along, segment, suffix};
use crate::rows::{Band, RowRuns};

const STEP: &str = "lanes";

/// Lanes one cutter height apart, bottom to top. Each lane runs from the
/// leftmost to the rightmost region cell of its band, gaps included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lanes {
    pub axis: Axis,
}

impl Lanes {
    pub const fn new(axis: Axis) -> Self {
        Self { axis }
    }
}

impl Decomposer for Lanes {
    /// `lanes-h` or `lanes-v`, as the Python method names.
    fn label(&self) -> String {
        format!("lanes-{}", suffix(self.axis))
    }

    fn decompose(&self, problem: &Problem) -> Result<Vec<Segment>, SolveError> {
        along(self.axis, problem, horizontal_lanes)
    }
}

fn horizontal_lanes(problem: &Problem) -> Result<Vec<Segment>, SolveError> {
    let band = Band::of(problem.cutter());
    let extents = problem.cutter().row_extents();

    let rows = RowRuns::new(problem.region());
    let Some((lowest, highest)) = rows.span() else {
        return Ok(Vec::new());
    };

    // the first lane reaches the lowest row with its top cutter row; lanes go
    // on until one lane height past the one that reaches the highest row with
    // its bottom cutter row, as Python's range does
    let first = lowest - band.low;
    let last = first.max(highest - band.high + band.height - 1);

    let mut lanes = Vec::new();
    let mut y = first;
    while y <= last {
        if let Some((a, b)) = lane_span(&rows, extents, y) {
            lanes.push(segment(STEP, y, a, b)?);
        }
        y += band.height;
    }

    Ok(lanes)
}

/// The anchors `a..=b` a lane at height `y` must travel, or `None` if its
/// band holds no region cells. A band narrower than the cutter keeps `a`.
fn lane_span(rows: &RowRuns, extents: &[RowExtent], y: i64) -> Option<(i64, i64)> {
    let mut span: Option<(i64, i64)> = None;

    for extent in extents {
        let row = rows.row(y + i64::from(extent.dy));
        let (Some(left), Some(right)) = (row.first(), row.last()) else {
            continue;
        };

        let start = i64::from(left.x_min) - i64::from(extent.low);
        let end = i64::from(right.x_max) - i64::from(extent.high);

        span = Some(match span {
            Some((a, b)) => (a.min(start), b.max(end)),
            None => (start, end),
        });
    }

    span.map(|(a, b)| (a, a.max(b)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{horizontal, problem, rectangle, square, vertical, x_cutter};

    #[test]
    fn rectangle_gets_one_lane_per_band() {
        let p = problem(&rectangle(10, 6), &square(3));

        let h = Lanes::new(Axis::Horizontal).decompose(&p).unwrap();
        let v = Lanes::new(Axis::Vertical).decompose(&p).unwrap();

        assert_eq!(h, horizontal(&[(1, 1, 8), (4, 1, 8)]));
        // the last lane only reaches the top row with its bottom cutter row
        assert_eq!(v, vertical(&[(1, 1, 4), (4, 1, 4), (7, 1, 4), (10, 1, 4)]));
    }

    #[test]
    fn lanes_cross_gaps() {
        let u = [(0, 0, 9), (1, 0, 1), (1, 8, 9)];
        let p = problem(&u, &[(0, 0)]);

        let lanes = Lanes::new(Axis::Horizontal).decompose(&p).unwrap();

        assert_eq!(lanes, horizontal(&[(0, 0, 9), (1, 0, 9)]));
    }

    #[test]
    fn narrow_band_keeps_a_single_anchor() {
        let p = problem(&[(0, 5, 5), (1, 5, 5), (2, 5, 5)], &square(3));

        let lanes = Lanes::new(Axis::Horizontal).decompose(&p).unwrap();

        assert_eq!(lanes, horizontal(&[(1, 6, 6)]));
    }

    #[test]
    fn x_cutter_uses_the_outer_extent_of_each_row() {
        let p = problem(&rectangle(10, 3), &x_cutter());

        let lanes = Lanes::new(Axis::Horizontal).decompose(&p).unwrap();

        assert_eq!(lanes, horizontal(&[(1, 0, 9)]));
    }

    #[test]
    fn labels_name_the_axis() {
        assert_eq!(Lanes::new(Axis::Horizontal).label(), "lanes-h");
        assert_eq!(Lanes::new(Axis::Vertical).label(), "lanes-v");
    }
}
