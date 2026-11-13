//! Coverage repair: out-and-back detours to the cells a walk or a set of
//! routes misses (`solver/repair.py`).
//!
//! For each missed cell, in row major order and skipping cells an earlier
//! detour already sweeps: the position closest to it (first on ties) gets a
//! detour to the anchor sweeping the cell that is closest to that position
//! (first cutter cell on ties). The detours are spliced in as corners, so the
//! dense walk is never built.

use std::collections::BTreeMap;
use std::slice;

use mow_core::{BitGrid, Point, Problem, Repairer, Route, SolveError, Walk};
use mow_coverage::{uncovered, uncovered_walks};

use crate::normalize::{normalize_cycle, normalize_open};
use crate::stretches::Stretches;

/// Anchors to visit, per path, keyed by the position the trip leaves from.
type Trips = Vec<BTreeMap<usize, Vec<Point>>>;

/// The `Repairer` of `repair.py`, label `detours`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Detours;

impl Repairer for Detours {
    fn label(&self) -> String {
        "detours".to_string()
    }

    /// `repair_walk`, returned with repeated and straight-through corners
    /// dropped, i.e. as `compress` of Python's positions.
    fn repair_walk(&self, problem: &Problem, walk: Walk) -> Result<Walk, SolveError> {
        let missing = uncovered_walks(problem, slice::from_ref(&walk))?;
        let stretches = Stretches::of_walk(&walk);
        let detours = plan(problem, &missing, &stretches, 1)?;

        let corners = rebuild(&stretches, 0, &detours[0], false)?;
        Ok(Walk::new(normalize_open(&corners))?)
    }

    /// `repair_tours`, each route returned as `close_route(compress(..))`
    /// of Python's positions.
    fn repair(&self, problem: &Problem, routes: Vec<Route>) -> Result<Vec<Route>, SolveError> {
        let missing = uncovered(problem, &routes)?;
        let stretches = Stretches::of_routes(&routes);
        let detours = plan(problem, &missing, &stretches, routes.len())?;

        detours
            .iter()
            .enumerate()
            .map(|(owner, trips)| {
                let corners = rebuild(&stretches, owner, trips, true)?;
                if corners.is_empty() {
                    // a route of zero length has no positions to repair from
                    return Ok(routes[owner].clone());
                }
                Ok(Route::new(normalize_cycle(&normalize_open(&corners)))?)
            })
            .collect()
    }
}

/// Picks the detours, as the loop over `missing` in Python does.
fn plan(
    problem: &Problem,
    missing: &BitGrid,
    stretches: &Stretches,
    paths: usize,
) -> Result<Trips, SolveError> {
    let offsets = problem.cutter().offsets();
    let mut detours: Trips = vec![BTreeMap::new(); paths];
    let mut handled = BitGrid::empty(problem.region().frame())?;

    for cell in missing.cells() {
        if handled.contains(cell) {
            continue;
        }

        let Some(near) = stretches.nearest(cell) else {
            return Err(unsupported("there is no position to leave from"));
        };

        // the anchors whose footprint holds the cell, the closest one first
        let mut anchor = cell.shifted(-offsets[0].x, -offsets[0].y);
        for o in &offsets[1..] {
            let candidate = cell.shifted(-o.x, -o.y);
            if candidate.l1(near.point) < anchor.l1(near.point) {
                anchor = candidate;
            }
        }

        detours[near.owner].entry(near.index).or_default().push(anchor);
        for o in offsets {
            handled.insert(anchor.shifted(o.x, o.y));
        }
    }

    Ok(detours)
}

/// The corners of path `owner` with its detours spliced in: every position
/// a trip leaves from becomes a corner, followed by the trip there and back.
/// A walk ends at its last position; a route's list ends at its last
/// position too, the way home is implied.
fn rebuild(
    stretches: &Stretches,
    owner: usize,
    trips: &BTreeMap<usize, Vec<Point>>,
    closed: bool,
) -> Result<Vec<Point>, SolveError> {
    let own: Vec<_> = stretches.items.iter().filter(|s| s.owner == owner).collect();
    let mut corners: Vec<Point> = Vec::with_capacity(own.len() + 4 * trips.len() + 1);

    if let Some(first) = own.first() {
        corners.push(first.start);
    }

    for (k, stretch) in own.iter().enumerate() {
        for (&index, anchors) in trips.range(stretch.first..stretch.first + stretch.count) {
            let home = stretch.at(index - stretch.first);
            corners.push(home);

            let mut stops = Vec::with_capacity(anchors.len() + 2);
            stops.push(home);
            stops.extend_from_slice(anchors);
            stops.push(home);
            corners.extend_from_slice(&Walk::through(&stops)?.corners()[1..]);
        }

        // a route's edge ends on the next corner, except the closing one
        let last_of_route = k + 1 == own.len();
        let end = if closed && !last_of_route {
            stretch.at(stretch.count)
        } else {
            stretch.last()
        };
        corners.push(end);
    }

    Ok(corners)
}

fn unsupported(reason: &str) -> SolveError {
    SolveError::Unsupported {
        step: "detours",
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mow_core::{Bounds, Cutter};
    use mow_coverage::{is_covered, uncovered_walks};

    fn pts(raw: &[(i32, i32)]) -> Vec<Point> {
        raw.iter().map(|&p| p.into()).collect()
    }

    /// A `width x height` rectangle mowed by a 3 x 3 square.
    fn rectangle(width: i32, height: i32) -> Problem {
        let mut region = BitGrid::empty(Bounds {
            x_min: 0,
            y_min: 0,
            x_max: width - 1,
            y_max: height - 1,
        })
        .unwrap();
        for y in 0..height {
            region.insert_run(y, 0, width - 1);
        }

        let offsets: Vec<Point> = (-1..=1)
            .flat_map(|y| (-1..=1).map(move |x| Point::new(x, y)))
            .collect();
        let cutter = Cutter::new(BitGrid::from_cells(&offsets).unwrap().unwrap()).unwrap();
        Problem::new("rect", region, cutter, 2).unwrap()
    }

    #[test]
    fn a_walk_gets_detours_and_keeps_its_ends() {
        let problem = rectangle(12, 6);
        let walk = Walk::new(pts(&[(1, 1), (10, 1)])).unwrap();

        let repaired = Detours.repair_walk(&problem, walk).unwrap();
        let left = uncovered_walks(&problem, slice::from_ref(&repaired)).unwrap();

        assert!(left.is_empty());
        assert_eq!(repaired.start(), Point::new(1, 1));
        assert_eq!(repaired.end(), Point::new(10, 1));
    }

    #[test]
    fn routes_stay_closed_and_cover_together() {
        let problem = rectangle(12, 8);
        let routes = vec![
            Route::new(pts(&[(1, 1), (10, 1)])).unwrap(),
            Route::new(pts(&[(1, 6)])).unwrap(),
        ];

        let repaired = Detours.repair(&problem, routes).unwrap();

        assert!(is_covered(&problem, &repaired).unwrap());
        assert_eq!(repaired.len(), 2);
    }

    #[test]
    fn a_covered_walk_comes_back_normalized() {
        let problem = rectangle(3, 3);
        let walk = Walk::new(pts(&[(1, 1), (1, 1), (1, 1)])).unwrap();

        assert_eq!(
            Detours.repair_walk(&problem, walk).unwrap().corners(),
            pts(&[(1, 1)])
        );
    }
}
