//! The search: passes of every move over the corners of the longest tour,
//! until a pass saves nothing or the deadline comes, then the corner cut.
//!
//! The counts hold every tour, so a cell swept by another tour lets the
//! longest one give it up. The longest tour is shortened first: that is
//! the objective. When it is stuck the others are shortened too, and a
//! transfer may lengthen one of them, never past the longest.

use std::time::Instant;

use mow_core::{Budget, Improver, Point, Problem, Route, SolveError};
use mow_coverage::{CoverageError, VisitCounts};
use mow_improve::CornerCut;

use crate::moves::{transfer, Ctx, Move};
use crate::path::{closed_length, normalize};
use crate::TurnsOptions;

/// What one search did, per move in the order of [`Move::ALL`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TurnsStats {
    pub passes: usize,
    /// Coverage checks made.
    pub tries: u64,
    pub kept: [usize; 4],
    pub saved: [u64; 4],
    /// Steps the closing corner cut saved on the objective.
    pub cut_saved: u64,
    /// Blocks moved from the longest tour to another, and the steps that
    /// lowered the longer of the two by.
    pub transfers: usize,
    pub transfer_lowered: u64,
}

impl TurnsStats {
    /// Steps saved by the moves on any tour, and by the corner cut on the objective.
    pub fn saved_steps(&self) -> u64 {
        self.saved.iter().sum::<u64>() + self.cut_saved
    }

    /// `slide 3/12 shortcut 0/0 ... transfer 1/40`: moves kept and steps
    /// saved per move.
    pub fn per_move(&self) -> String {
        let mut parts: Vec<String> = Move::ALL
            .iter()
            .enumerate()
            .map(|(m, mv)| format!("{} {}/{}", mv.name(), self.kept[m], self.saved[m]))
            .collect();
        parts.push(format!("transfer {}/{}", self.transfers, self.transfer_lowered));

        parts.join(" ")
    }
}

/// Shortens the longest of `routes` until `deadline`, keeping every cell
/// the routes sweep swept. The routes come back in the same order.
pub fn turns_routes(
    problem: &Problem,
    routes: &[Route],
    deadline: Instant,
    options: TurnsOptions,
) -> Result<(Vec<Route>, TurnsStats), SolveError> {
    let mut counts = VisitCounts::from_routes(problem, routes).map_err(coverage)?;
    let mut tours: Vec<Vec<Point>> = routes.iter().map(|r| r.corners().to_vec()).collect();
    tours.iter_mut().for_each(normalize);

    let mut stats = TurnsStats::default();
    let mut ctx = Ctx::new(&mut counts, options);
    descend(&mut tours, &mut ctx, deadline, &mut stats).map_err(coverage)?;
    stats.tries = ctx.tries;

    let routes = tours.into_iter().map(Route::new).collect::<Result<Vec<_>, _>>()?;
    let before = objective(&routes);
    let cut = CornerCut.improve(problem, routes, Budget { deadline, seed: 0 })?;
    stats.cut_saved = before - objective(&cut);

    Ok((cut, stats))
}

/// Shortens the longest tour until it is stuck. Then, with `balance` and
/// more than one tour, shortens the others and moves blocks from the
/// longest to them, and starts over while that lowers the longest.
fn descend(
    tours: &mut [Vec<Point>],
    ctx: &mut Ctx,
    deadline: Instant,
    stats: &mut TurnsStats,
) -> Result<(), CoverageError> {
    let mut settled = vec![false; tours.len()];

    while Instant::now() < deadline {
        let Some(top) = longest(tours) else { return Ok(()) };
        if settle(&mut tours[top], ctx, deadline, stats)? {
            // the longest may be another tour now
            continue;
        }
        settled[top] = true;

        if !ctx.options.balance || tours.len() < 2 {
            return Ok(());
        }
        for t in 0..tours.len() {
            if !settled[t] {
                settle(&mut tours[t], ctx, deadline, stats)?;
                settled[t] = true;
            }
        }

        let Some(other) = balance(tours, top, ctx, stats)? else {
            return Ok(());
        };
        settled[top] = false;
        settled[other] = false;
    }

    Ok(())
}

/// Passes over `tour` until one saves nothing. True if any saved steps.
fn settle(
    tour: &mut Vec<Point>,
    ctx: &mut Ctx,
    deadline: Instant,
    stats: &mut TurnsStats,
) -> Result<bool, CoverageError> {
    let mut any = false;

    while Instant::now() < deadline {
        stats.passes += 1;
        if pass(tour, ctx, deadline, stats)? == 0 {
            break;
        }
        any = true;
    }

    Ok(any)
}

/// One transfer from the longest tour `top` to any other. Returns the tour
/// that took the block.
fn balance(
    tours: &mut [Vec<Point>],
    top: usize,
    ctx: &mut Ctx,
    stats: &mut TurnsStats,
) -> Result<Option<usize>, CoverageError> {
    for other in (0..tours.len()).filter(|&t| t != top) {
        let (from, to) = pair(tours, top, other);
        let lowered = transfer(from, to, ctx)?;
        if lowered > 0 {
            normalize(from);
            normalize(to);
            stats.transfers += 1;
            stats.transfer_lowered += lowered;
            return Ok(Some(other));
        }
    }

    Ok(None)
}

/// Mutable borrows of two different tours.
fn pair(tours: &mut [Vec<Point>], a: usize, b: usize) -> (&mut Vec<Point>, &mut Vec<Point>) {
    match a < b {
        true => {
            let (low, high) = tours.split_at_mut(b);
            (&mut low[a], &mut high[0])
        }
        false => {
            let (low, high) = tours.split_at_mut(a);
            (&mut high[0], &mut low[b])
        }
    }
}

/// Every move at every corner of `tour`, first improvement, staying on a
/// corner while it improves. Returns the steps saved.
fn pass(
    tour: &mut Vec<Point>,
    ctx: &mut Ctx,
    deadline: Instant,
    stats: &mut TurnsStats,
) -> Result<u64, CoverageError> {
    let mut total = 0;

    for (m, mv) in Move::ALL.into_iter().enumerate() {
        if !ctx.options.moves[m] {
            continue;
        }

        let mut i = 0;
        while i < tour.len() {
            if Instant::now() >= deadline {
                return Ok(total);
            }

            let saved = mv.try_at(tour, i, ctx)?;
            if saved == 0 {
                i += 1;
                continue;
            }

            normalize(tour);
            stats.kept[m] += 1;
            stats.saved[m] += saved;
            total += saved;
        }
    }

    Ok(total)
}

/// The index of the longest tour, the first of equals.
fn longest(tours: &[Vec<Point>]) -> Option<usize> {
    let lengths: Vec<u64> = tours.iter().map(|t| closed_length(t)).collect();
    let top = *lengths.iter().max()?;

    lengths.iter().position(|&l| l == top)
}

fn objective(routes: &[Route]) -> u64 {
    routes.iter().map(Route::length).max().unwrap_or(0)
}

fn coverage(error: CoverageError) -> SolveError {
    match error {
        CoverageError::Model(model) => SolveError::Model(model),
        other => SolveError::Unsupported {
            step: "turns",
            reason: other.to_string(),
        },
    }
}
