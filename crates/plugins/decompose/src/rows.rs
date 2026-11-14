//! The region's runs grouped by row, and the cutter's band height.

use mow_core::{BitGrid, Cutter, Run};

/// The region's maximal runs, row by row from the bottom, with an index so a
/// lane can fetch the runs of any row in constant time.
#[derive(Debug, Clone)]
pub(crate) struct RowRuns {
    runs: Vec<Run>,
    starts: Vec<usize>,
    y_min: i64,
}

impl RowRuns {
    pub(crate) fn new(region: &BitGrid) -> Self {
        Self::from_runs(region.runs().collect())
    }

    /// From runs already sorted by row, then left to right, as
    /// [`BitGrid::runs`] yields them.
    pub(crate) fn from_runs(runs: Vec<Run>) -> Self {
        let y_min = runs.first().map_or(0, |run| i64::from(run.y));

        // starts[r] is the first run of row y_min + r, starts[r + 1] one past its last
        let mut starts = vec![0];
        for (index, run) in runs.iter().enumerate() {
            let row = (i64::from(run.y) - y_min) as usize;
            while starts.len() <= row {
                starts.push(index);
            }
        }
        starts.push(runs.len());

        Self { runs, starts, y_min }
    }

    /// The runs of row `y`, left to right. Empty for rows without cells.
    pub(crate) fn row(&self, y: i64) -> &[Run] {
        let Some(row) = usize::try_from(y - self.y_min).ok() else {
            return &[];
        };

        match (self.starts.get(row), self.starts.get(row + 1)) {
            (Some(&start), Some(&end)) => &self.runs[start..end],
            _ => &[],
        }
    }

    /// The leftmost and rightmost columns holding cells. `None` if there are
    /// none.
    pub(crate) fn x_span(&self) -> Option<(i64, i64)> {
        let low = self.runs.iter().map(|run| run.x_min).min()?;
        let high = self.runs.iter().map(|run| run.x_max).max()?;

        Some((i64::from(low), i64::from(high)))
    }

    /// The same rows cut to the columns `x_min..=x_max`.
    pub(crate) fn clipped(&self, x_min: i64, x_max: i64) -> Self {
        self.window((x_min, x_max), (i64::MIN, i64::MAX))
    }

    /// Only the cells in columns `x_min..=x_max` of rows `y_min..=y_max`.
    pub(crate) fn window(&self, (x_min, x_max): (i64, i64), (y_min, y_max): (i64, i64)) -> Self {
        let runs = self
            .runs
            .iter()
            .filter(|run| (y_min..=y_max).contains(&i64::from(run.y)))
            .filter_map(|run| {
                let low = i64::from(run.x_min).max(x_min);
                let high = i64::from(run.x_max).min(x_max);

                // both ends lie inside the run, so they fit in i32
                (low <= high).then_some(Run {
                    x_min: low as i32,
                    x_max: high as i32,
                    ..*run
                })
            })
            .collect();

        Self::from_runs(runs)
    }

    /// The lowest and highest rows holding cells. `None` if there are none.
    pub(crate) fn span(&self) -> Option<(i64, i64)> {
        let low = self.runs.first()?.y;
        let high = self.runs.last()?.y;

        Some((i64::from(low), i64::from(high)))
    }
}

/// The rows a cutter covers around its center: `low..=high`, `height` rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Band {
    pub(crate) low: i64,
    pub(crate) high: i64,
    pub(crate) height: i64,
}

impl Band {
    pub(crate) fn of(cutter: &Cutter) -> Self {
        let bounds = cutter.bounds();
        let (low, high) = (i64::from(bounds.y_min), i64::from(bounds.y_max));

        Self {
            low,
            high,
            height: high - low + 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{problem, square, x_cutter};

    #[test]
    fn rows_hold_their_runs_and_gaps_are_empty() {
        let p = problem(&[(2, 0, 3), (2, 6, 9), (5, -1, -1)], &[(0, 0)]);
        let rows = RowRuns::new(p.region());

        assert_eq!(rows.span(), Some((2, 5)));
        assert_eq!(rows.row(2).len(), 2);
        assert_eq!(rows.row(2)[1].x_min, 6);
        assert!(rows.row(3).is_empty());
        assert!(rows.row(1).is_empty());
        assert!(rows.row(99).is_empty());
        assert_eq!(rows.row(5)[0].x_max, -1);
    }

    #[test]
    fn clipping_keeps_only_the_given_columns() {
        let p = problem(&[(2, 0, 3), (2, 6, 9), (5, -1, -1)], &[(0, 0)]);
        let rows = RowRuns::new(p.region());

        let clipped = rows.clipped(2, 7);

        assert_eq!(rows.x_span(), Some((-1, 9)));
        assert_eq!(clipped.x_span(), Some((2, 7)));
        assert_eq!(clipped.span(), Some((2, 2)));
        assert_eq!(clipped.row(2).len(), 2);
        assert!(rows.clipped(20, 30).span().is_none());
    }

    #[test]
    fn band_spans_the_cutter_rows() {
        let square = problem(&[(0, 0, 0)], &square(5));
        let x = problem(&[(0, 0, 0)], &x_cutter());

        assert_eq!(
            Band::of(square.cutter()),
            Band {
                low: -2,
                high: 2,
                height: 5
            }
        );
        assert_eq!(Band::of(x.cutter()).height, 3);
    }
}
