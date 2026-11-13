//! Lane segments: lanes that skip the gaps (port of `segments.lane_segments`).

use mow_core::{Axis, Decomposer, Problem, RowExtent, Segment, SolveError};

use crate::axis::{along, segment, suffix};
use crate::intervals::{lane_intervals, merge, Interval};
use crate::rows::{Band, RowRuns};

const STEP: &str = "segments";

const DEFAULT_GAP_FACTOR: f64 = 2.0;

/// Lanes one cutter height apart, bottom to top, each cut into the stretches
/// of anchors that have cells to mow, left to right.
///
/// Gaps up to `max_gap` anchors are bridged; without one, up to `gap_factor`
/// lane heights. A factor of 0 only joins intervals that touch or overlap.
/// `offset` moves every lane down by that many rows, `shift` by that
/// fraction of the lane height, and the two add up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LaneSegments {
    pub axis: Axis,
    pub gap_factor: f64,
    pub max_gap: Option<i32>,
    pub offset: i32,
    pub shift: f64,
}

impl LaneSegments {
    /// The Python defaults: gap factor 2, no offset, no shift.
    pub const fn new(axis: Axis) -> Self {
        Self {
            axis,
            gap_factor: DEFAULT_GAP_FACTOR,
            max_gap: None,
            offset: 0,
            shift: 0.0,
        }
    }

    fn horizontal(&self, problem: &Problem) -> Result<Vec<Segment>, SolveError> {
        let band = Band::of(problem.cutter());
        let max_gap = self.max_gap(band)?;
        let offset = i64::from(self.offset).saturating_add(truncated(STEP, self.shift, band.height)?);

        let rows = RowRuns::new(problem.region());
        let extents = problem.cutter().row_extents();

        cut_lanes(&rows, extents, band, offset, max_gap)
            .into_iter()
            .map(|(y, a, b)| segment(STEP, y, a, b))
            .collect()
    }

    /// `max_gap` if set, else `gap_factor` lane heights, truncated like `int()`.
    fn max_gap(&self, band: Band) -> Result<i64, SolveError> {
        match self.max_gap {
            Some(gap) => Ok(i64::from(gap)),
            None => truncated(STEP, self.gap_factor, band.height),
        }
    }
}

/// A lane stretch `(lane, from, to)` in `i64`, before it becomes a [`Segment`].
pub(crate) type Span = (i64, i64, i64);

/// The merged segments of every lane over `rows`, bottom to top, left to
/// right (`lane_segments` in Python). Lanes are `band.height` rows apart and
/// `offset` moves them down by that many rows.
pub(crate) fn cut_lanes(
    rows: &RowRuns,
    extents: &[RowExtent],
    band: Band,
    offset: i64,
    max_gap: i64,
) -> Vec<Span> {
    let Some((lowest, highest)) = rows.span() else {
        return Vec::new();
    };

    // the first lane must reach the lowest row with its top cutter row
    let first = lowest - band.low - offset.rem_euclid(band.height);
    let last = highest - band.low;

    let mut intervals: Vec<Interval> = Vec::new();
    let mut merged: Vec<Interval> = Vec::new();
    let mut spans = Vec::new();

    let mut y = first;
    while y <= last {
        intervals.clear();
        merged.clear();

        lane_intervals(rows, extents, y, &mut intervals);
        merge(&mut intervals, max_gap, &mut merged);
        spans.extend(merged.iter().map(|&(a, b)| (y, a, b)));

        y += band.height;
    }

    spans
}

/// `int(factor * height)` as Python computes it, refusing NaN and infinity.
pub(crate) fn truncated(step: &'static str, factor: f64, height: i64) -> Result<i64, SolveError> {
    let value = factor * height as f64;
    if !value.is_finite() {
        return Err(SolveError::Unsupported {
            step,
            reason: format!("knob {factor} times lane height {height} is not a finite number"),
        });
    }

    // casts truncate toward zero like int(), and saturate far beyond any lane
    Ok(value as i64)
}

impl Decomposer for LaneSegments {
    /// `segments-h` or `segments-v`, then each knob that differs from its
    /// default as `/key=value`, keys sorted as in `SolveConfig.label`.
    fn label(&self) -> String {
        let mut label = format!("segments-{}", suffix(self.axis));

        if self.gap_factor != DEFAULT_GAP_FACTOR {
            label += &format!("/gap_factor={:?}", self.gap_factor);
        }
        if let Some(gap) = self.max_gap {
            label += &format!("/max_gap={gap}");
        }
        if self.offset != 0 {
            label += &format!("/offset={}", self.offset);
        }
        if self.shift != 0.0 {
            label += &format!("/shift={:?}", self.shift);
        }

        label
    }

    fn decompose(&self, problem: &Problem) -> Result<Vec<Segment>, SolveError> {
        along(self.axis, problem, |p| self.horizontal(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{horizontal, problem, rectangle, square, vertical, x_cutter};

    const U: [(i32, i32, i32); 3] = [(0, 0, 9), (1, 0, 1), (1, 8, 9)];

    fn with(change: impl FnOnce(&mut LaneSegments)) -> LaneSegments {
        let mut segments = LaneSegments::new(Axis::Horizontal);
        change(&mut segments);
        segments
    }

    #[test]
    fn rectangle_gets_one_segment_per_band() {
        let p = problem(&rectangle(10, 6), &square(3));

        let h = LaneSegments::new(Axis::Horizontal).decompose(&p).unwrap();
        let v = LaneSegments::new(Axis::Vertical).decompose(&p).unwrap();

        assert_eq!(h, horizontal(&[(1, 1, 8), (4, 1, 8)]));
        assert_eq!(v, vertical(&[(1, 1, 4), (4, 1, 4), (7, 1, 4), (10, 1, 4)]));
    }

    #[test]
    fn u_shape_gap_is_bridged_only_when_short_enough() {
        let p = problem(&U, &[(0, 0)]);

        let bridged = with(|s| s.max_gap = Some(7)).decompose(&p).unwrap();
        let split = with(|s| s.max_gap = Some(6)).decompose(&p).unwrap();
        let by_factor = LaneSegments::new(Axis::Horizontal).decompose(&p).unwrap();

        assert_eq!(bridged, horizontal(&[(0, 0, 9), (1, 0, 9)]));
        assert_eq!(split, horizontal(&[(0, 0, 9), (1, 0, 1), (1, 8, 9)]));
        assert_eq!(by_factor, split);
    }

    #[test]
    fn gap_factor_counts_lane_heights() {
        let p = problem(&U, &[(0, 0)]);

        // one lane height is one row here, so a factor of 7 bridges 7 anchors
        let bridged = with(|s| s.gap_factor = 7.0).decompose(&p).unwrap();

        assert_eq!(bridged.len(), 2);
    }

    #[test]
    fn region_narrower_than_the_cutter_takes_the_middle_anchor() {
        let p = problem(&[(0, -6, -5), (1, -6, -5), (2, -6, -5)], &square(3));

        let segments = LaneSegments::new(Axis::Horizontal).decompose(&p).unwrap();

        assert_eq!(segments, horizontal(&[(1, -6, -6)]));
    }

    #[test]
    fn x_cutter_rows_trim_by_their_own_extent() {
        let p = problem(&rectangle(10, 3), &x_cutter());

        let segments = LaneSegments::new(Axis::Horizontal).decompose(&p).unwrap();

        assert_eq!(segments, horizontal(&[(1, 0, 9)]));
    }

    #[test]
    fn offset_and_shift_move_the_lanes_down() {
        let p = problem(&rectangle(10, 6), &square(3));
        let expected = horizontal(&[(0, 1, 8), (3, 1, 8), (6, 1, 8)]);

        let by_offset = with(|s| s.offset = 1).decompose(&p).unwrap();
        let by_shift = with(|s| s.shift = 0.34).decompose(&p).unwrap();
        let negative = with(|s| s.offset = -2).decompose(&p).unwrap();

        assert_eq!(by_offset, expected);
        assert_eq!(by_shift, expected);
        assert_eq!(negative, expected);
    }

    #[test]
    fn non_finite_knobs_are_refused() {
        let p = problem(&rectangle(4, 4), &square(3));

        assert!(with(|s| s.gap_factor = f64::NAN).decompose(&p).is_err());
        assert!(with(|s| s.shift = f64::INFINITY).decompose(&p).is_err());
    }

    #[test]
    fn huge_gaps_do_not_overflow() {
        let p = problem(&U, &[(0, 0)]);

        let segments = with(|s| s.max_gap = Some(i32::MAX)).decompose(&p).unwrap();

        assert_eq!(segments.len(), 2);
    }

    #[test]
    fn labels_show_only_changed_knobs() {
        assert_eq!(LaneSegments::new(Axis::Horizontal).label(), "segments-h");
        assert_eq!(LaneSegments::new(Axis::Vertical).label(), "segments-v");

        let changed = LaneSegments {
            gap_factor: 0.0,
            max_gap: Some(3),
            offset: 1,
            shift: 0.5,
            ..LaneSegments::new(Axis::Vertical)
        };
        assert_eq!(
            changed.label(),
            "segments-v/gap_factor=0.0/max_gap=3/offset=1/shift=0.5"
        );
    }
}
