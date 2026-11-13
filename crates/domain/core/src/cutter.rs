//! The cutter: its cells relative to its center, and what follows from them.

use crate::error::ModelError;
use crate::geom::{Bounds, Point};
use crate::grid::BitGrid;

/// For one row `dy` of the cutter, the smallest and largest `dx` it covers.
/// A row with a gap (the X cutter) still reports its outer extent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowExtent {
    pub dy: i32,
    pub low: i32,
    pub high: i32,
}

/// The cutter's cells, translated so that its center is the origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cutter {
    cells: BitGrid,
    offsets: Vec<Point>,
    rows: Vec<RowExtent>,
    bounds: Bounds,
    step_gain: u32,
}

impl Cutter {
    /// `cells` must already be relative to the cutter's center.
    pub fn new(cells: BitGrid) -> Result<Self, ModelError> {
        let bounds = cells.cell_bounds().ok_or(ModelError::EmptyCutter)?;

        let offsets: Vec<Point> = cells.cells().collect();
        let rows = row_extents(&cells);
        let step_gain = step_gain(&cells, &offsets);

        Ok(Self {
            cells,
            offsets,
            rows,
            bounds,
            step_gain,
        })
    }

    pub const fn cells(&self) -> &BitGrid {
        &self.cells
    }

    /// The cells as offsets from the center, row by row from the bottom.
    pub fn offsets(&self) -> &[Point] {
        &self.offsets
    }

    pub fn area(&self) -> usize {
        self.offsets.len()
    }

    /// One entry per cutter row, sorted by `dy`.
    pub fn row_extents(&self) -> &[RowExtent] {
        &self.rows
    }

    /// The box of the cells, relative to the center.
    pub const fn bounds(&self) -> Bounds {
        self.bounds
    }

    /// The most cells one unit step can newly sweep: `|C \ (C + e)|`
    /// maximized over the four unit directions `e`.
    pub const fn step_gain(&self) -> u32 {
        self.step_gain
    }

    /// Mirrored across the diagonal `x = y`.
    pub fn transposed(&self) -> Self {
        Self::new(self.cells.transposed()).expect("a transposed cutter keeps its cells")
    }
}

fn row_extents(cells: &BitGrid) -> Vec<RowExtent> {
    let mut rows: Vec<RowExtent> = Vec::new();

    // runs come row by row from the bottom, so a new dy starts a new entry
    for run in cells.runs() {
        match rows.last_mut() {
            Some(row) if row.dy == run.y => row.high = run.x_max,
            _ => rows.push(RowExtent {
                dy: run.y,
                low: run.x_min,
                high: run.x_max,
            }),
        }
    }

    rows
}

fn step_gain(cells: &BitGrid, offsets: &[Point]) -> u32 {
    let directions = [(1, 0), (-1, 0), (0, 1), (0, -1)];

    directions
        .iter()
        .map(|&(dx, dy)| {
            offsets
                .iter()
                .filter(|o| !cells.contains(o.shifted(-dx, -dy)))
                .count() as u32
        })
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cutter(cells: &[(i32, i32)]) -> Cutter {
        let points: Vec<Point> = cells.iter().map(|&c| c.into()).collect();
        Cutter::new(BitGrid::from_cells(&points).unwrap().unwrap()).unwrap()
    }

    #[test]
    fn square_of_side_three() {
        let cells: Vec<(i32, i32)> = (-1..=1).flat_map(|y| (-1..=1).map(move |x| (x, y))).collect();
        let c = cutter(&cells);

        assert_eq!(c.area(), 9);
        assert_eq!(c.step_gain(), 3);
        assert_eq!(c.row_extents().len(), 3);
        assert_eq!(
            c.row_extents()[0],
            RowExtent {
                dy: -1,
                low: -1,
                high: 1
            }
        );
    }

    #[test]
    fn x_shape_reports_outer_extent_and_large_gain() {
        let c = cutter(&[(-1, -1), (1, -1), (0, 0), (-1, 1), (1, 1)]);

        assert_eq!(
            c.row_extents()[0],
            RowExtent {
                dy: -1,
                low: -1,
                high: 1
            }
        );
        assert_eq!(c.step_gain(), 5);
    }

    #[test]
    fn transposed_bar_turns_rows_into_one_row_each() {
        let c = cutter(&[(0, -1), (0, 0), (0, 1)]).transposed();

        assert_eq!(
            c.row_extents(),
            &[RowExtent {
                dy: 0,
                low: -1,
                high: 1
            }]
        );
    }
}
