//! Polishing a finished solution window by window: `polish_tours` of
//! `solver/polish.py`, and the part of `run_polish` between loading the
//! best and verifying the result.
//!
//! The longest tour is cut into square windows of anchors, laid as tiles
//! with a random offset. The windows are re-solved in random order (or,
//! with `aim`, most room first), and every shorter walk is spliced in at
//! once. When another tour becomes the longest, the tiles move to it.
//!
//! The random numbers come from `rand`'s seeded `SmallRng`, not Python's
//! Mersenne Twister: the same seed gives the same polish run to run, but
//! not the tiles Python would draw.

use std::time::{Duration, Instant};

use rand::rngs::SmallRng;
use rand::SeedableRng;

use mow_core::{CpSolver, Point, Problem, Route, Walk};

use crate::error::PolishError;
use crate::solve::solve_window;
use crate::tiles::{aimed, tiles};
use crate::window_map::dense_tours;

/// Window side in anchors; the pilot proved most 20 x 20 windows optimal in seconds.
pub const DEFAULT_SIDE: i32 = 20;
pub const DEFAULT_WINDOW_SECONDS: f64 = 3.0;

/// A window error in a row this often means the solver is gone: the job
/// stops and keeps what it has.
const MAX_ERRORS_IN_A_ROW: usize = 3;

/// Passes in a row without one window to solve (the longest tour fits in
/// one tile) end the job instead of drawing tiles until the deadline.
const MAX_IDLE_PASSES: usize = 3;

/// A round number no mining round uses.
pub const POLISH_ROUND: i32 = -2;

/// Polishing time per instance, by size: up to so many cells, so many
/// seconds. A full pass over the windows of a big instance did not fit in 5 minutes.
pub const POLISH_SECONDS: [(usize, f64); 2] = [(400_000, 600.0), (2_000_000, 1200.0)];
pub const POLISH_SECONDS_LARGEST: f64 = 1800.0;

/// The polishing time for an instance of `cells` cells.
pub fn polish_seconds(cells: usize) -> f64 {
    POLISH_SECONDS
        .iter()
        .find(|&&(limit, _)| cells <= limit)
        .map_or(POLISH_SECONDS_LARGEST, |&(_, seconds)| seconds)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PolishOptions {
    pub side: i32,
    /// The most one window may take.
    pub window_seconds: f64,
    pub workers: u32,
    pub seed: u64,
    /// Windows with the most room first, instead of in random order.
    pub aim: bool,
}

impl Default for PolishOptions {
    fn default() -> Self {
        Self {
            side: DEFAULT_SIDE,
            window_seconds: DEFAULT_WINDOW_SECONDS,
            workers: 1,
            seed: 0,
            aim: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PolishStats {
    /// Windows solved.
    pub windows: usize,
    /// Windows that gave a shorter walk.
    pub improved: usize,
    pub saved_steps: u64,
    pub passes: usize,
    /// Windows whose solve or splice failed; each costs only that window.
    pub errors: usize,
}

/// Shortens the longest of the dense closed `tours` until `deadline`.
/// Returns new tours, never longer than the old ones.
///
/// Unlike Python, a window that fails (the solver dies, an answer does not
/// splice) is counted and skipped, so it never costs the shorter walks found
/// before it; [`MAX_ERRORS_IN_A_ROW`] stops the job with what it has.
pub fn polish_tours(
    problem: &Problem,
    mut tours: Vec<Vec<Point>>,
    deadline: Instant,
    options: &PolishOptions,
    solver: &dyn CpSolver,
) -> Result<(Vec<Vec<Point>>, PolishStats), PolishError> {
    if options.side < 1 {
        return Err(PolishError::BadSide(options.side));
    }
    if tours.is_empty() {
        return Err(PolishError::NoTours);
    }

    let mut rng = SmallRng::seed_from_u64(options.seed);
    let mut stats = PolishStats::default();
    let mut errors_in_a_row = 0;
    let mut idle_passes = 0;

    while Instant::now() < deadline && errors_in_a_row < MAX_ERRORS_IN_A_ROW && idle_passes < MAX_IDLE_PASSES
    {
        stats.passes += 1;
        let longest = longest(&tours);
        let solved_before = stats.windows + stats.errors;

        let mut windows = tiles(&tours[longest], options.side, &mut rng);
        if options.aim {
            windows = aimed(problem, &tours, longest, &windows);
        }

        for window in windows {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() || errors_in_a_row >= MAX_ERRORS_IN_A_ROW {
                break;
            }

            let limit = seconds(options.window_seconds).min(left);
            let result = match solve_window(problem, &tours, longest, window, limit, options.workers, solver)
            {
                Ok(Some(result)) => result,
                Ok(None) => continue,
                Err(error) => {
                    eprintln!("polish {}: window {window:?} skipped: {error}", problem.uid());
                    stats.errors += 1;
                    errors_in_a_row += 1;
                    continue;
                }
            };

            errors_in_a_row = 0;
            stats.windows += 1;

            let saved = result.saved();
            let Some(positions) = result
                .positions
                .filter(|p| saved > 0 && is_shorter_tour(p, &tours[longest]))
            else {
                continue;
            };

            tours[longest] = positions;
            stats.improved += 1;
            stats.saved_steps += saved as u64;

            // once another tour is the longest, this one no longer sets the objective
            if self::longest(&tours) != longest {
                break;
            }
        }

        idle_passes = match stats.windows + stats.errors == solved_before {
            true => idle_passes + 1,
            false => 0,
        };
    }

    Ok((tours, stats))
}

/// A spliced walk is taken only if it really is shorter and moves one unit
/// step at a time, so a wrong objective can never lengthen or break a tour.
fn is_shorter_tour(new: &[Point], old: &[Point]) -> bool {
    let unit_steps = new.windows(2).all(|pair| pair[0].l1(pair[1]) == 1);
    let closes = match (new.first(), new.last()) {
        (Some(&first), Some(&last)) => first.l1(last) <= 1,
        _ => false,
    };

    new.len() < old.len() && unit_steps && closes
}

/// Polishes the routes of a solution: densified, polished, compressed and
/// closed again, as `run_polish` does between reading the best and handing
/// the result to the verifier.
pub fn polish_routes(
    problem: &Problem,
    routes: &[Route],
    deadline: Instant,
    options: &PolishOptions,
    solver: &dyn CpSolver,
) -> Result<(Vec<Route>, PolishStats), PolishError> {
    let (tours, stats) = polish_tours(problem, dense_tours(routes), deadline, options, solver)?;

    let routes = tours
        .iter()
        .map(|positions| Walk::from_positions(positions)?.closed())
        .collect::<Result<Vec<_>, _>>()?;

    Ok((routes, stats))
}

/// The index of the longest tour, the first of equals as `np.argmax`.
pub fn longest(tours: &[Vec<Point>]) -> usize {
    let mut best = 0;

    for (index, tour) in tours.iter().enumerate() {
        if tour.len() > tours[best].len() {
            best = index;
        }
    }

    best
}

/// Seconds as a duration; nothing for a negative or broken number.
fn seconds(value: f64) -> Duration {
    Duration::try_from_secs_f64(value).unwrap_or(Duration::ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polishing_time_grows_with_the_instance() {
        assert_eq!(polish_seconds(10_000), 600.0);
        assert_eq!(polish_seconds(400_000), 600.0);
        assert_eq!(polish_seconds(400_001), 1200.0);
        assert_eq!(polish_seconds(8_000_000), 1800.0);
    }

    #[test]
    fn longest_takes_the_first_of_equals() {
        let tour = |n: usize| vec![Point::new(0, 0); n];

        assert_eq!(longest(&[tour(3), tour(5), tour(5)]), 1);
        assert_eq!(longest(&[tour(2)]), 0);
    }

    #[test]
    fn broken_seconds_give_no_time() {
        assert_eq!(seconds(-1.0), Duration::ZERO);
        assert_eq!(seconds(f64::NAN), Duration::ZERO);
        assert_eq!(seconds(1.5), Duration::from_millis(1500));
    }
}
