//! Window ids of anchors and of the cells they sweep, and the cells only
//! one window sweeps: `exclusive_cells` in `solver/window_map.py`.
//!
//! Python takes the minimum and maximum over one shifted copy of the
//! anchor grid per cutter offset, `area` passes over the grid. Here each
//! cutter row is a few runs, and a run of length `L` takes the minimum over
//! `L` neighbouring anchors, which grows one anchor at a time: one pass per
//! run length plus one per run. Same cells, same ids.

use mow_core::{Point, Problem};

/// No window yet: larger than every id.
pub const NO_WINDOW: u32 = u32::MAX;

/// For every anchor in a box, the lowest and the highest window id of the
/// positions on it. An anchor no tour visits has [`NO_WINDOW`] as lowest.
#[derive(Debug, Clone)]
pub struct AnchorIds {
    low: Point,
    width: usize,
    height: usize,
    lowest: Vec<u32>,
    highest: Vec<u32>,
}

impl AnchorIds {
    /// Over the box of every position of `tours`. `None` if there is none.
    pub fn over(tours: &[Vec<Point>]) -> Option<Self> {
        let mut points = tours.iter().flatten();
        let first = *points.next()?;

        let (mut low, mut high) = (first, first);
        for p in points {
            low = Point::new(low.x.min(p.x), low.y.min(p.y));
            high = Point::new(high.x.max(p.x), high.y.max(p.y));
        }

        let width = (high.x - low.x + 1) as usize;
        let height = (high.y - low.y + 1) as usize;

        Some(Self {
            low,
            width,
            height,
            lowest: vec![NO_WINDOW; width * height],
            highest: vec![0; width * height],
        })
    }

    /// The lower left corner of the box.
    pub const fn low(&self) -> Point {
        self.low
    }

    /// Notes that anchor `p`, inside the box, has a position in window `id`.
    pub fn record(&mut self, p: Point, id: u32) {
        let index = (p.y - self.low.y) as usize * self.width + (p.x - self.low.x) as usize;

        self.lowest[index] = self.lowest[index].min(id);
        self.highest[index] = self.highest[index].max(id);
    }
}

/// Per window, the region cells whose every sweeping anchor lies in it: the
/// lowest and the highest id over those anchors are equal exactly then.
pub fn exclusive_cells(problem: &Problem, ids: &AnchorIds, windows: usize) -> Vec<u64> {
    let cutter = problem.cutter().bounds();
    let cell_low = ids.low.shifted(cutter.x_min, cutter.y_min);
    let cell_width = ids.width + (cutter.x_max - cutter.x_min) as usize;
    let cell_height = ids.height + (cutter.y_max - cutter.y_min) as usize;

    let mut cell_lowest = vec![NO_WINDOW; cell_width * cell_height];
    let mut cell_highest = vec![0; cell_width * cell_height];
    let spread = Spread::new(problem);

    for row in 0..ids.height {
        let span = row * ids.width..(row + 1) * ids.width;
        let (lowest, highest) = (&ids.lowest[span.clone()], &ids.highest[span]);

        if lowest.iter().all(|&id| id == NO_WINDOW) {
            continue;
        }

        spread.row(lowest, row, cell_width, &mut cell_lowest, NO_WINDOW, u32::min);
        spread.row(highest, row, cell_width, &mut cell_highest, 0, u32::max);
    }

    let mut need = vec![0; windows];
    for cell in problem.region().cells() {
        let column = usize::try_from(i64::from(cell.x) - i64::from(cell_low.x)).ok();
        let row = usize::try_from(i64::from(cell.y) - i64::from(cell_low.y)).ok();

        // a cell outside the frame has no sweeping anchor at all
        let Some((column, row)) = column
            .zip(row)
            .filter(|&(c, r)| c < cell_width && r < cell_height)
        else {
            continue;
        };

        let (low, high) = (
            cell_lowest[row * cell_width + column],
            cell_highest[row * cell_width + column],
        );
        if low != NO_WINDOW && low == high {
            if let Some(count) = need.get_mut(low as usize) {
                *count += 1;
            }
        }
    }

    need
}

/// The cutter as runs, grouped by length: for each length, the cell row and
/// the first cell column (in the cell frame) of every run that long.
struct Spread {
    by_length: Vec<Vec<(usize, usize)>>,
}

impl Spread {
    fn new(problem: &Problem) -> Self {
        let cells = problem.cutter().cells();
        let bounds = problem.cutter().bounds();

        let mut by_length: Vec<Vec<(usize, usize)>> = Vec::new();
        for run in cells.runs() {
            let length = run.len() as usize;
            if by_length.len() <= length {
                by_length.resize(length + 1, Vec::new());
            }

            let dy = (run.y - bounds.y_min) as usize;
            let dx = (run.x_min - bounds.x_min) as usize;
            by_length[length].push((dy, dx));
        }

        Self { by_length }
    }

    /// Folds anchor row `row` (`values`) into the cell rows it sweeps.
    /// `window[j]` holds the fold of the `length` anchors ending at `j`.
    fn row(
        &self,
        values: &[u32],
        row: usize,
        cell_width: usize,
        cells: &mut [u32],
        empty: u32,
        fold: impl Fn(u32, u32) -> u32,
    ) {
        let width = values.len();
        let longest = self.by_length.len().saturating_sub(1);

        let mut window = vec![empty; width + longest];
        window[..width].copy_from_slice(values);

        for length in 1..=longest {
            if length > 1 {
                let grown = &mut window[length - 1..length - 1 + width];
                for (slot, &value) in grown.iter_mut().zip(values) {
                    *slot = fold(*slot, value);
                }
            }

            let reach = width + length - 1;
            for &(dy, dx) in &self.by_length[length] {
                let start = (row + dy) * cell_width + dx;

                for (cell, &value) in cells[start..start + reach].iter_mut().zip(&window[..reach]) {
                    *cell = fold(*cell, value);
                }
            }
        }
    }
}
