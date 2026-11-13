//! Square tiles on the lane grid, and which way each one sweeps.

use std::ops::Range;

use mow_core::{Problem, SolveError};

use crate::segments::truncated;

use super::STEP;

/// Tile borders along one axis of the region's mask: tile `t` holds the mask
/// indices `first + t * size .. first + (t + 1) * size`, clipped to the mask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Tiling {
    /// Where the first tile starts, zero or negative (`first_row` in Python).
    pub(super) first: i64,
    pub(super) size: i64,
    pub(super) count: usize,
    /// The mask's length along this axis.
    pub(super) length: i64,
}

impl Tiling {
    /// Tiles of `lanes` lanes, each `band` long, moved back by `shift` lanes.
    fn new(length: i64, band: i64, lanes: i64, shift: f64) -> Result<Self, SolveError> {
        let size = lanes * band;
        let moved = truncated(STEP, shift, band)?;

        // Python indexes numpy arrays with these; outside this range numpy
        // wraps negative indices around, which is not worth copying
        if !(0..size).contains(&moved) {
            return Err(SolveError::Unsupported {
                step: STEP,
                reason: format!("shift {shift} must move the tiles by 0 to {lanes} lanes"),
            });
        }

        let first = -moved;
        let count = (length - first + size - 1) / size;

        Ok(Self {
            first,
            size,
            count: count as usize,
            length,
        })
    }

    /// The tile holding mask index `i`, which must lie inside the mask.
    pub(super) fn of(self, i: i64) -> usize {
        (i - self.first).div_euclid(self.size) as usize
    }

    /// The mask indices of tile `t`, clipped to the mask.
    pub(super) fn slice(self, t: usize) -> Range<i64> {
        let start = self.first + t as i64 * self.size;

        start.max(0)..(start + self.size).min(self.length)
    }

    /// The first mask index of the tile after the one holding `i`.
    pub(super) fn next_border(self, i: i64) -> i64 {
        self.first + (self.of(i) as i64 + 1) * self.size
    }
}

/// Rows and columns of tiles over the region's mask (`TileGrid` in Python).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TileGrid {
    pub(super) rows: Tiling,
    pub(super) columns: Tiling,
}

impl TileGrid {
    /// Tiles `lanes` lanes high and wide. A horizontal lane is as high as the
    /// cutter, a vertical one as wide. The mask is the box of the region.
    pub(super) fn new(problem: &Problem, lanes: u32, shift: f64) -> Result<Self, SolveError> {
        if lanes == 0 {
            return Err(SolveError::Unsupported {
                step: STEP,
                reason: "tiles must be at least one lane wide".to_string(),
            });
        }

        let mask = problem.region_bounds();
        let cutter = problem.cutter().bounds();
        let span = |low: i32, high: i32| i64::from(high) - i64::from(low) + 1;

        let lanes = i64::from(lanes);
        let rows = Tiling::new(
            span(mask.y_min, mask.y_max),
            span(cutter.y_min, cutter.y_max),
            lanes,
            shift,
        )?;
        let columns = Tiling::new(
            span(mask.x_min, mask.x_max),
            span(cutter.x_min, cutter.x_max),
            lanes,
            shift,
        )?;

        Ok(Self { rows, columns })
    }

    /// The same tiles seen in the mirrored problem.
    pub(super) const fn transposed(self) -> Self {
        Self {
            rows: self.columns,
            columns: self.rows,
        }
    }
}

/// For every tile, whether it sweeps vertically. Row major, in the tiles of
/// the unmirrored problem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Directions {
    columns: usize,
    vertical: Vec<bool>,
}

impl Directions {
    pub(super) fn horizontal(grid: TileGrid) -> Self {
        Self {
            columns: grid.columns.count,
            vertical: vec![false; grid.rows.count * grid.columns.count],
        }
    }

    pub(super) fn get(&self, row: usize, column: usize) -> bool {
        self.vertical[row * self.columns + column]
    }

    pub(super) fn set(&mut self, row: usize, column: usize, vertical: bool) {
        self.vertical[row * self.columns + column] = vertical;
    }

    /// The tiles marked vertical, row by row, as `np.argwhere` lists them.
    pub(super) fn marked(&self) -> Vec<(usize, usize)> {
        let columns = self.columns;

        self.vertical
            .iter()
            .enumerate()
            .filter(|(_, &v)| v)
            .map(|(i, _)| (i / columns, i % columns))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{problem, rectangle, square};

    #[test]
    fn tiles_cover_the_mask_from_the_first_border() {
        let tiling = Tiling::new(10, 3, 2, 0.0).unwrap();

        assert_eq!((tiling.first, tiling.size, tiling.count), (0, 6, 2));
        assert_eq!(tiling.slice(1), 6..10);
        assert_eq!((tiling.of(5), tiling.of(6)), (0, 1));
        assert_eq!(tiling.next_border(7), 12);
    }

    #[test]
    fn a_shift_moves_the_first_border_back() {
        let tiling = Tiling::new(10, 3, 2, 0.5).unwrap();

        // int(0.5 * 3) = 1 row back, so one more tile is needed
        assert_eq!((tiling.first, tiling.count), (-1, 2));
        assert_eq!(tiling.slice(0), 0..5);
        assert_eq!(tiling.of(5), 1);
    }

    #[test]
    fn bad_knobs_are_refused() {
        let p = problem(&rectangle(10, 6), &square(3));

        assert!(TileGrid::new(&p, 0, 0.0).is_err());
        assert!(TileGrid::new(&p, 4, -0.5).is_err());
        assert!(TileGrid::new(&p, 4, 4.0).is_err());
        assert!(TileGrid::new(&p, 4, f64::NAN).is_err());
        assert!(TileGrid::new(&p, 4, 3.9).is_ok());
    }

    #[test]
    fn transposed_swaps_rows_and_columns() {
        let p = problem(&rectangle(10, 6), &square(3));
        let grid = TileGrid::new(&p, 1, 0.0).unwrap();

        assert_eq!((grid.rows.count, grid.columns.count), (2, 4));
        assert_eq!(grid.transposed().rows, grid.columns);
    }

    #[test]
    fn marked_tiles_come_row_by_row() {
        let p = problem(&rectangle(10, 6), &square(3));
        let mut directions = Directions::horizontal(TileGrid::new(&p, 1, 0.0).unwrap());

        directions.set(1, 0, true);
        directions.set(0, 3, true);

        assert_eq!(directions.marked(), vec![(0, 3), (1, 0)]);
        assert!(directions.get(1, 0));
    }
}
