//! Corner cutting (`prune_routes` in `improve.py`): cut corners of every
//! route while coverage allows, longest route first, with one set of visit
//! counts shared by all routes so a cell swept by another route counts.
//!
//! Same moves in the same order as Python, so with time to spare the result
//! is the same. Python renormalizes the whole route after every cut; here the
//! first cut normalizes it once and every later one only looks around
//! itself, which gives the same corners (see `normalize.rs`).

use std::time::Instant;

use mow_core::{Budget, Improver, Point, Problem, Route, SolveError, Walk};
use mow_coverage::{CoverageError, VisitCounts};

use crate::cut::try_cut;
use crate::normalize::{
    normalize_cycle, normalize_cycle_after_edit, normalize_open, normalize_open_after_edit,
};

/// Moves between two looks at the clock.
const CLOCK_EVERY: u32 = 256;

/// The `Improver` of `improve.py`, label `corner-cut`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CornerCut;

impl Improver for CornerCut {
    fn label(&self) -> String {
        "corner-cut".to_string()
    }

    /// `prune_routes([walk], closed=False)`: both ends stay.
    fn improve_walk(&self, problem: &Problem, walk: Walk, budget: Budget) -> Result<Walk, SolveError> {
        let mut counts = VisitCounts::new(problem).map_err(coverage)?;
        counts.add_positions(walk.positions()).map_err(coverage)?;

        let mut clock = Clock::new(budget.deadline);
        if clock.expired() {
            return Ok(walk);
        }

        let corners = prune(walk.into_corners(), &mut counts, false, &mut clock).map_err(coverage)?;
        Ok(Walk::new(corners)?)
    }

    /// `prune_routes(routes, closed=True)`, longest route first.
    fn improve(
        &self,
        problem: &Problem,
        routes: Vec<Route>,
        budget: Budget,
    ) -> Result<Vec<Route>, SolveError> {
        let mut counts = VisitCounts::from_routes(problem, &routes).map_err(coverage)?;
        let mut clock = Clock::new(budget.deadline);

        let mut order: Vec<usize> = (0..routes.len()).collect();
        order.sort_by_key(|&i| std::cmp::Reverse(routes[i].length()));

        let mut result: Vec<Option<Route>> = routes.into_iter().map(Some).collect();
        for i in order {
            if clock.expired() {
                break;
            }

            let Some(route) = result[i].take() else { continue };
            let corners = prune(route.into_corners(), &mut counts, true, &mut clock).map_err(coverage)?;
            result[i] = Some(Route::new(corners)?);
        }

        Ok(result.into_iter().flatten().collect())
    }
}

/// `_prune_route`: sweeps over the corners until a whole sweep cuts nothing.
/// After a cut the same index is tried again, as in Python.
fn prune(
    mut route: Vec<Point>,
    counts: &mut VisitCounts,
    closed: bool,
    clock: &mut Clock,
) -> Result<Vec<Point>, CoverageError> {
    let mut normalized = false;
    let mut improved = true;

    while improved && !clock.expired() {
        improved = false;
        let mut i = usize::from(!closed);

        while i < route.len() - usize::from(!closed) {
            if route.len() < 3 || clock.expired() {
                break;
            }

            let Some(cut) = try_cut(&route, i, counts)? else {
                i += 1;
                continue;
            };

            let dirty = cut.apply(&mut route);
            match (normalized, closed) {
                (false, true) => route = normalize_cycle(&route),
                (false, false) => route = normalize_open(&route),
                (true, true) => normalize_cycle_after_edit(&mut route, &dirty),
                (true, false) => normalize_open_after_edit(&mut route, i),
            }

            normalized = true;
            improved = true;
        }
    }

    Ok(route)
}

/// The deadline, looked at once every [`CLOCK_EVERY`] questions.
struct Clock {
    deadline: Instant,
    asked: u32,
    expired: bool,
}

impl Clock {
    fn new(deadline: Instant) -> Self {
        Self {
            deadline,
            asked: 0,
            expired: Instant::now() >= deadline,
        }
    }

    fn expired(&mut self) -> bool {
        self.asked += 1;
        if self.asked % CLOCK_EVERY == 0 {
            self.expired = Instant::now() >= self.deadline;
        }

        self.expired
    }
}

fn coverage(error: CoverageError) -> SolveError {
    match error {
        CoverageError::Model(model) => SolveError::Model(model),
        other => SolveError::Unsupported {
            step: "corner-cut",
            reason: other.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use mow_core::{BitGrid, Bounds, Cutter};
    use mow_coverage::{is_covered, uncovered_walks};

    use super::*;

    fn pts(raw: &[(i32, i32)]) -> Vec<Point> {
        raw.iter().map(|&p| p.into()).collect()
    }

    /// A `side x side` square mowed by a 3 x 3 square cutter.
    fn square(side: i32) -> Problem {
        let mut region = BitGrid::empty(Bounds {
            x_min: 0,
            y_min: 0,
            x_max: side - 1,
            y_max: side - 1,
        })
        .unwrap();
        for y in 0..side {
            region.insert_run(y, 0, side - 1);
        }

        let offsets: Vec<Point> = (-1..=1)
            .flat_map(|y| (-1..=1).map(move |x| Point::new(x, y)))
            .collect();
        let cutter = Cutter::new(BitGrid::from_cells(&offsets).unwrap().unwrap()).unwrap();
        Problem::new("square", region, cutter, 1).unwrap()
    }

    fn plenty() -> Budget {
        Budget::new(Duration::from_secs(60), 0)
    }

    #[test]
    fn a_needless_detour_is_cut_and_coverage_kept() {
        let problem = square(6);
        // a tour around the inner ring plus a spike out to the corner
        let route = Route::new(pts(&[(1, 1), (4, 1), (4, 4), (5, 4), (4, 4), (1, 4)])).unwrap();

        let pruned = CornerCut
            .improve(&problem, vec![route.clone()], plenty())
            .unwrap();

        assert!(pruned[0].length() < route.length());
        assert!(is_covered(&problem, &pruned).unwrap());
    }

    #[test]
    fn walk_ends_stay() {
        let problem = square(6);
        let walk = Walk::new(pts(&[(1, 1), (4, 1), (4, 4), (5, 4), (4, 4), (1, 4)])).unwrap();

        let pruned = CornerCut.improve_walk(&problem, walk, plenty()).unwrap();

        assert_eq!(pruned.start(), Point::new(1, 1));
        assert_eq!(pruned.end(), Point::new(1, 4));
        assert_eq!(pruned.length(), 9);
        assert!(uncovered_walks(&problem, &[pruned]).unwrap().is_empty());
    }

    #[test]
    fn an_expired_budget_changes_nothing() {
        let problem = square(6);
        let route = Route::new(pts(&[(1, 1), (4, 1), (4, 4), (5, 4), (4, 4), (1, 4)])).unwrap();
        let spent = Budget::new(Duration::ZERO, 0);

        let kept = CornerCut.improve(&problem, vec![route.clone()], spent).unwrap();

        assert_eq!(kept, vec![route]);
    }
}
