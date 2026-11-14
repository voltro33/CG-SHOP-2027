//! Where the waste of a tour sits, window by window, and how much a perfect
//! re-solve of each window could save at most: `window_map` of
//! `solver/window_map.py`.
//!
//! The anchor positions of all tours are cut into square windows of `side`
//! anchors. For one window:
//!
//! - length: the unit steps whose anchor lies in the window
//! - need: the region cells that only anchors in this window sweep
//! - room: `length - need / gain`, the length above what those cells cost
//!   at the best rate
//! - pieces: how often the tours enter the window

use mow_core::{Point, Problem, Route};

use crate::error::PolishError;
use crate::ids::{exclusive_cells, AnchorIds};

/// Per window numbers of one tiling, row by row, `rows * columns` each.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowMap {
    pub side: i32,
    pub gain: u32,
    pub total_length: u64,
    pub rows: usize,
    pub columns: usize,
    pub length: Vec<u64>,
    pub need: Vec<u64>,
    pub pieces: Vec<u64>,
}

impl WindowMap {
    /// `length - need / gain` per window, as numpy computes it.
    pub fn room(&self) -> Vec<f64> {
        let gain = f64::from(self.gain);

        self.length
            .iter()
            .zip(&self.need)
            .map(|(&length, &need)| length as f64 - need as f64 / gain)
            .collect()
    }
}

/// The dense closed tours of `routes`, as `densify(corners, closed=True)`.
pub fn dense_tours(routes: &[Route]) -> Vec<Vec<Point>> {
    routes.iter().map(|route| route.positions().collect()).collect()
}

/// The map of the tours of `routes` for windows of `side` anchors.
pub fn window_map(problem: &Problem, routes: &[Route], side: i32) -> Result<WindowMap, PolishError> {
    if side < 1 {
        return Err(PolishError::BadSide(side));
    }

    let tours = dense_tours(routes);
    let mut ids = AnchorIds::over(&tours).ok_or(PolishError::NoTours)?;

    let low = ids.low();
    let high = tours
        .iter()
        .flatten()
        .fold(low, |h, p| Point::new(h.x.max(p.x), h.y.max(p.y)));
    let columns = ((high.x - low.x) / side) as usize + 1;
    let rows = ((high.y - low.y) / side) as usize + 1;

    let mut length = vec![0; rows * columns];
    let mut pieces = vec![0; rows * columns];

    for positions in &tours {
        let window = window_ids(positions, low, side, columns);

        // a piece starts where the tour comes in from another window
        let mut entered = false;
        for (at, &id) in window.iter().enumerate() {
            length[id] += 1;

            let before = window[(at + window.len() - 1) % window.len()];
            if id != before {
                pieces[id] += 1;
                entered = true;
            }
        }

        if !entered {
            pieces[window[0]] += 1;
        }

        for (&p, &id) in positions.iter().zip(&window) {
            ids.record(p, id as u32);
        }
    }

    Ok(WindowMap {
        side,
        gain: problem.cutter().step_gain(),
        total_length: tours.iter().map(|t| t.len() as u64).sum(),
        rows,
        columns,
        length,
        need: exclusive_cells(problem, &ids, rows * columns),
        pieces,
    })
}

/// The window of every position, on a grid of `columns` windows a row
/// whose lower left corner is `low`.
pub(crate) fn window_ids(positions: &[Point], low: Point, side: i32, columns: usize) -> Vec<usize> {
    positions
        .iter()
        .map(|p| {
            let column = (p.x - low.x).div_euclid(side) as usize;
            let row = (p.y - low.y).div_euclid(side) as usize;

            row * columns + column
        })
        .collect()
}

/// The largest offset coordinate less the smallest, over both axes, plus
/// one: the cutter's extent along its wider axis for the shapes in use.
pub fn cutter_span(problem: &Problem) -> i32 {
    let offsets = problem.cutter().offsets();
    let coordinates = offsets.iter().flat_map(|o| [o.x, o.y]);

    let (low, high) = coordinates.fold((i32::MAX, i32::MIN), |(l, h), c| (l.min(c), h.max(c)));
    high - low + 1
}
