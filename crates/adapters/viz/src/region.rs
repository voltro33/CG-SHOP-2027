//! Cell sets as few rectangles: row runs, with identical runs on adjacent
//! rows merged into one taller rectangle.

use mow_core::{BitGrid, Run};

/// The cells `x_min..=x_max` by `y_min..=y_max`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellRect {
    pub x_min: i32,
    pub x_max: i32,
    pub y_min: i32,
    pub y_max: i32,
}

impl CellRect {
    const fn of(run: Run) -> Self {
        Self {
            x_min: run.x_min,
            x_max: run.x_max,
            y_min: run.y,
            y_max: run.y,
        }
    }

    pub const fn cells(self) -> u64 {
        (self.x_max - self.x_min + 1) as u64 * (self.y_max - self.y_min + 1) as u64
    }
}

/// Hands every rectangle to `emit`, each cell in exactly one. Runs that
/// repeat row after row with the same ends become one rectangle. A
/// rectangle is emitted as soon as a row stops continuing it, so roughly
/// bottom up. Memory is two rows of runs, whatever the size of the grid.
pub fn merged_rects(grid: &BitGrid, mut emit: impl FnMut(CellRect)) {
    let mut open: Vec<CellRect> = Vec::new();
    let mut row: Vec<Run> = Vec::new();

    for run in grid.runs() {
        if row.first().is_some_and(|r| r.y != run.y) {
            open = advance(open, &row, &mut emit);
            row.clear();
        }
        row.push(run);
    }

    if !row.is_empty() {
        open = advance(open, &row, &mut emit);
    }
    open.into_iter().for_each(emit);
}

/// Extends the open rectangles that `row` continues, emits the others, and
/// returns the rectangles open after `row`. Both lists are sorted by `x_min`.
fn advance(open: Vec<CellRect>, row: &[Run], emit: &mut impl FnMut(CellRect)) -> Vec<CellRect> {
    let mut next = Vec::with_capacity(row.len());
    let mut pending = open.into_iter().peekable();

    for &run in row {
        while let Some(rect) = pending.next_if(|r| r.x_min < run.x_min) {
            emit(rect);
        }

        let continues = |r: &CellRect| r.x_min == run.x_min && r.x_max == run.x_max && r.y_max + 1 == run.y;
        match pending.next_if(continues) {
            Some(rect) => next.push(CellRect { y_max: run.y, ..rect }),
            None => next.push(CellRect::of(run)),
        }
    }

    pending.for_each(emit);
    next
}

#[cfg(test)]
mod tests {
    use super::*;
    use mow_core::{Bounds, Point};

    fn grid(width: i32, height: i32, holes: &[(i32, i32)]) -> BitGrid {
        let frame = Bounds {
            x_min: 0,
            y_min: 0,
            x_max: width - 1,
            y_max: height - 1,
        };
        let mut grid = BitGrid::empty(frame).unwrap();
        for y in 0..height {
            grid.insert_run(y, 0, width - 1);
        }
        for &(x, y) in holes {
            grid.remove(Point::new(x, y));
        }
        grid
    }

    fn rects(grid: &BitGrid) -> Vec<CellRect> {
        let mut found = Vec::new();
        merged_rects(grid, |r| found.push(r));
        found.sort_by_key(|r| (r.y_min, r.x_min));
        found
    }

    fn rect(x_min: i32, x_max: i32, y_min: i32, y_max: i32) -> CellRect {
        CellRect {
            x_min,
            x_max,
            y_min,
            y_max,
        }
    }

    #[test]
    fn a_full_rectangle_is_one() {
        assert_eq!(rects(&grid(10, 4, &[])), vec![rect(0, 9, 0, 3)]);
    }

    #[test]
    fn a_hole_splits_its_row_only() {
        let found = rects(&grid(10, 4, &[(3, 1)]));

        assert_eq!(
            found,
            vec![
                rect(0, 9, 0, 0),
                rect(0, 2, 1, 1),
                rect(4, 9, 1, 1),
                rect(0, 9, 2, 3)
            ]
        );
        assert_eq!(found.iter().map(|r| r.cells()).sum::<u64>(), 39);
    }

    #[test]
    fn empty_rows_break_rectangles() {
        let mut g = grid(4, 5, &[]);
        for x in 0..4 {
            g.remove(Point::new(x, 2));
        }

        assert_eq!(rects(&g), vec![rect(0, 3, 0, 1), rect(0, 3, 3, 4)]);
    }

    #[test]
    fn staircase_keeps_every_cell_once() {
        let mut g = grid(6, 6, &[]);
        for y in 0..6 {
            for x in 0..y {
                g.remove(Point::new(x, y));
            }
        }
        let found = rects(&g);

        assert_eq!(found.len(), 6);
        assert_eq!(found.iter().map(|r| r.cells()).sum::<u64>(), g.len() as u64);
    }

    #[test]
    fn empty_grid_has_none() {
        assert!(rects(&grid(
            3,
            3,
            &[
                (0, 0),
                (1, 0),
                (2, 0),
                (0, 1),
                (1, 1),
                (2, 1),
                (0, 2),
                (1, 2),
                (2, 2)
            ]
        ))
        .is_empty());
    }
}
