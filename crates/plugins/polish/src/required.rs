//! The cells a new walk through a window must sweep, as `_required_cells`
//! in `solver/window.py`.

use mow_core::{Point, Problem};

use crate::window::{in_pieces, Piece, Window};

/// A box of the grid with one flag per point, row by row.
struct Mask {
    x0: i32,
    y0: i32,
    width: usize,
    height: usize,
    flags: Vec<bool>,
}

impl Mask {
    fn new(x0: i32, y0: i32, width: usize, height: usize) -> Self {
        Self {
            x0,
            y0,
            width,
            height,
            flags: vec![false; width * height],
        }
    }

    fn index(&self, p: Point) -> Option<usize> {
        let column = usize::try_from(i64::from(p.x) - i64::from(self.x0)).ok()?;
        let row = usize::try_from(i64::from(p.y) - i64::from(self.y0)).ok()?;

        (column < self.width && row < self.height).then_some(row * self.width + column)
    }
}

/// Region cells that anchors in `window` can sweep and no kept anchor
/// sweeps: kept are every other tour, and tour `tour_index` outside its
/// `pieces`. Row by row from the bottom, left to right, as `np.nonzero`.
pub fn required_cells(
    problem: &Problem,
    tours: &[Vec<Point>],
    tour_index: usize,
    pieces: &[Piece],
    window: Window,
) -> Vec<Point> {
    let cutter = problem.cutter().bounds();
    let reach_x = (cutter.x_max - cutter.x_min) as usize;
    let reach_y = (cutter.y_max - cutter.y_min) as usize;

    // the cells the window can reach, and the anchors that can sweep them
    let cells = Mask::new(
        window.x0 + cutter.x_min,
        window.y0 + cutter.y_min,
        window.width() as usize + reach_x,
        window.height() as usize + reach_y,
    );
    let mut kept = Mask::new(
        cells.x0 - cutter.x_max,
        cells.y0 - cutter.y_max,
        cells.width + reach_x,
        cells.height + reach_y,
    );

    for (index, positions) in tours.iter().enumerate() {
        let cut = (index == tour_index).then(|| in_pieces(positions.len(), pieces));

        for (at, &p) in positions.iter().enumerate() {
            let is_cut = cut.as_ref().is_some_and(|cut| cut[at]);

            if let (false, Some(i)) = (is_cut, kept.index(p)) {
                kept.flags[i] = true;
            }
        }
    }

    let swept = swept_by(problem, &kept, &cells);

    let mut required = Vec::new();
    for row in 0..cells.height {
        for column in 0..cells.width {
            let cell = Point::new(cells.x0 + column as i32, cells.y0 + row as i32);

            if !swept[row * cells.width + column] && problem.region().contains(cell) {
                required.push(cell);
            }
        }
    }

    required
}

/// A cell is swept if a kept anchor sits at the cell minus some offset.
fn swept_by(problem: &Problem, kept: &Mask, cells: &Mask) -> Vec<bool> {
    let mut swept = vec![false; cells.width * cells.height];

    for offset in problem.cutter().offsets() {
        let left = (cells.x0 - offset.x - kept.x0) as usize;
        let top = (cells.y0 - offset.y - kept.y0) as usize;

        for row in 0..cells.height {
            let from = (top + row) * kept.width + left;
            let source = &kept.flags[from..from + cells.width];
            let target = &mut swept[row * cells.width..(row + 1) * cells.width];

            for (cell, &anchor) in target.iter_mut().zip(source) {
                *cell |= anchor;
            }
        }
    }

    swept
}
