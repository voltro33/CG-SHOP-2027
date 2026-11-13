//! The anchor intervals one lane must drive, and how they are merged.
//!
//! Coordinates are `i64` here so trimming and gap sums never overflow; they
//! are checked back into `i32` when segments are built.

use mow_core::RowExtent;

use crate::rows::RowRuns;

/// An inclusive stretch of anchors `start..=end` along a lane.
pub(crate) type Interval = (i64, i64);

/// Appends to `out` the anchor intervals a lane at height `y` must drive so
/// that every region run in its band gets swept, unsorted.
///
/// Cutter row `dy` spans `low..high` around the center, so driving from `a`
/// to `b` sweeps `a + low .. b + high` in grid row `y + dy`. A run `s..e`
/// therefore needs the anchors `s - low .. e - high`. When the run is shorter
/// than the cutter row that interval is backwards and any single anchor in
/// between does the job; the middle one is taken, rounded down.
pub(crate) fn lane_intervals(rows: &RowRuns, extents: &[RowExtent], y: i64, out: &mut Vec<Interval>) {
    for extent in extents {
        let (low, high) = (i64::from(extent.low), i64::from(extent.high));

        for run in rows.row(y + i64::from(extent.dy)) {
            let a = i64::from(run.x_min) - low;
            let b = i64::from(run.x_max) - high;

            if a > b {
                let middle = (a + b).div_euclid(2);
                out.push((middle, middle));
            } else {
                out.push((a, b));
            }
        }
    }
}

/// Sorts `intervals` and appends to `out` the merged ones: any two that
/// overlap or whose gap is at most `max_gap` become one.
///
/// As in Python, the end of a merged interval is the furthest end reached so
/// far, so a long interval swallows the short ones inside it.
pub(crate) fn merge(intervals: &mut [Interval], max_gap: i64, out: &mut Vec<Interval>) {
    // stable like numpy's argsort; ties only matter for negative gaps
    intervals.sort_by_key(|&(start, _)| start);

    let Some((&(first_start, first_end), rest)) = intervals.split_first() else {
        return;
    };

    // reach never resets, like numpy's running max over every interval
    let (mut start, mut reach) = (first_start, first_end);
    for &(next_start, next_end) in rest {
        if next_start > reach.saturating_add(max_gap) {
            out.push((start, reach));
            start = next_start;
        }
        reach = reach.max(next_end);
    }

    out.push((start, reach));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{problem, square, x_cutter};

    fn merged(mut intervals: Vec<Interval>, max_gap: i64) -> Vec<Interval> {
        let mut out = Vec::new();
        merge(&mut intervals, max_gap, &mut out);
        out
    }

    #[test]
    fn merge_joins_overlaps_and_short_gaps() {
        let intervals = vec![(12, 14), (0, 10), (2, 3)];

        assert_eq!(merged(intervals.clone(), 0), vec![(0, 10), (12, 14)]);
        assert_eq!(merged(intervals.clone(), 1), vec![(0, 10), (12, 14)]);
        assert_eq!(merged(intervals.clone(), 2), vec![(0, 14)]);
        assert_eq!(merged(vec![(0, 3), (3, 5)], 0), vec![(0, 5)]);
        assert_eq!(merged(vec![(0, 3), (4, 5)], 0), vec![(0, 3), (4, 5)]);
        assert!(merged(Vec::new(), 3).is_empty());
    }

    #[test]
    fn merge_keeps_the_furthest_reach_even_for_negative_gaps() {
        // numpy's running max spans every interval seen, not just the group
        assert_eq!(merged(vec![(0, 10), (7, 8)], -5), vec![(0, 10), (7, 10)]);
    }

    #[test]
    fn intervals_are_trimmed_by_each_cutter_row() {
        let p = problem(&[(0, 1, 9), (1, 0, 9), (2, 2, 9)], &x_cutter());
        let rows = RowRuns::new(p.region());

        let mut out = Vec::new();
        lane_intervals(&rows, p.cutter().row_extents(), 1, &mut out);
        out.sort_unstable();

        assert_eq!(out, vec![(0, 9), (2, 8), (3, 8)]);
    }

    #[test]
    fn short_runs_take_the_middle_anchor_rounded_down() {
        let p = problem(&[(0, -6, -5)], &square(3));
        let rows = RowRuns::new(p.region());

        let mut out = Vec::new();
        lane_intervals(&rows, p.cutter().row_extents(), 1, &mut out);

        assert_eq!(out, vec![(-6, -6)]);
    }
}
