//! One window re-solved, as `solve_window` in `solver/window.py`.

use std::time::{Duration, Instant};

use mow_core::{CpParams, CpSolver, CpStatus, Point, Problem};

use crate::error::PolishError;
use crate::model::WindowModel;
use crate::required::required_cells;
use crate::window::{cut_pieces, old_steps, Piece, Window};

#[derive(Debug, Clone, PartialEq)]
pub struct WindowResult {
    pub old_steps: u64,
    pub new_steps: u64,
    pub status: CpStatus,
    /// Wall time, rounded to hundredths as Python rounds it.
    pub seconds: f64,
    pub nodes: usize,
    pub pieces: usize,
    pub required: usize,
    /// The new dense closed tour, if the solver found one.
    pub positions: Option<Vec<Point>>,
}

impl WindowResult {
    pub fn saved(&self) -> i64 {
        self.old_steps as i64 - self.new_steps as i64
    }
}

/// A window ready to solve: its pieces, the cells it must cover and the
/// model with the hint and the step limit in.
#[derive(Debug, Clone)]
pub struct PreparedWindow {
    pub pieces: Vec<Piece>,
    pub old_steps: u64,
    pub required: Vec<Point>,
    pub model: WindowModel,
}

/// Everything `solve_window` does before it calls the solver. `None` if
/// the window holds no piece of tour `tour_index` or the whole tour.
pub fn prepare_window(
    problem: &Problem,
    tours: &[Vec<Point>],
    tour_index: usize,
    window: Window,
) -> Option<PreparedWindow> {
    let positions = &tours[tour_index];

    let pieces = cut_pieces(positions, window).filter(|pieces| !pieces.is_empty())?;
    let old_steps = old_steps(positions.len(), &pieces);
    let required = required_cells(problem, tours, tour_index, &pieces, window);

    let mut model = WindowModel::new(problem, window, positions, &pieces, &required);
    model.add_hint(positions, &pieces);
    model.limit_steps(old_steps);

    Some(PreparedWindow {
        pieces,
        old_steps,
        required,
        model,
    })
}

/// Re-solves the part of tour `tour_index` inside `window`. `tours` are the
/// dense closed tours of a covering solution. `Ok(None)` if the window
/// holds no piece of that tour or the whole tour.
pub fn solve_window(
    problem: &Problem,
    tours: &[Vec<Point>],
    tour_index: usize,
    window: Window,
    time_limit: Duration,
    workers: u32,
    solver: &dyn CpSolver,
) -> Result<Option<WindowResult>, PolishError> {
    let started = Instant::now();

    let Some(prepared) = prepare_window(problem, tours, tour_index, window) else {
        return Ok(None);
    };

    let params = CpParams { time_limit, workers };
    let response = solver.solve(prepared.model.model(), &params)?;

    let mut new_steps = prepared.old_steps;
    let mut positions = None;
    if response.status.found() {
        new_steps = response.objective.round().max(0.0) as u64;
        let spliced = prepared
            .model
            .splice(&response, &tours[tour_index], &prepared.pieces)?;
        positions = Some(spliced);
    }

    let seconds = (started.elapsed().as_secs_f64() * 100.0).round() / 100.0;

    Ok(Some(WindowResult {
        old_steps: prepared.old_steps,
        new_steps,
        status: response.status,
        seconds,
        nodes: window.nodes(),
        pieces: prepared.pieces.len(),
        required: prepared.required.len(),
        positions,
    }))
}
