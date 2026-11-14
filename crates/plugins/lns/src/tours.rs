//! The solution being searched: one dense closed tour per cutter, their
//! lengths, and the visit counts of all of them together.

use mow_core::{Point, Problem, Route, Walk};
use mow_coverage::{Scratch, VisitCounts};
use mow_polish::cutter_span;

use crate::error::LnsError;

/// How much a step of a tour that is not the longest counts, against one
/// of the longest: enough that the search shortens the other tours too
/// (room for a later transfer), never enough to lengthen the longest.
const OTHERS_WEIGHT: f64 = 0.1;

pub struct Tours<'p> {
    pub problem: &'p Problem,
    pub tours: Vec<Vec<Point>>,
    pub counts: VisitCounts,
    pub scratch: Scratch,
    /// The cutter's span in anchors, the unit of every window.
    pub span: i32,
}

impl<'p> Tours<'p> {
    /// Starts from `routes`, which must sweep every cell together.
    pub fn new(problem: &'p Problem, routes: &[Route]) -> Result<Self, LnsError> {
        if routes.is_empty() {
            return Err(LnsError::NoTours);
        }

        let tours: Vec<Vec<Point>> = routes.iter().map(|r| r.positions().collect()).collect();
        let counts = VisitCounts::from_routes(problem, routes)?;
        if !counts.is_covered() {
            return Err(LnsError::NotCovered(counts.uncovered_count()));
        }

        Ok(Self {
            problem,
            scratch: Scratch::for_counts(&counts),
            counts,
            tours,
            span: cutter_span(problem),
        })
    }

    pub fn lengths(&self) -> Vec<u64> {
        self.tours.iter().map(|t| dense_length(t)).collect()
    }

    pub fn objective(&self) -> u64 {
        self.lengths().into_iter().max().unwrap_or(0)
    }

    /// The first longest tour.
    pub fn longest(&self) -> usize {
        let lengths = self.lengths();
        let max = lengths.iter().copied().max().unwrap_or(0);
        lengths.iter().position(|&l| l == max).unwrap_or(0)
    }

    /// The tours as routes, corners only.
    pub fn routes(&self) -> Result<Vec<Route>, LnsError> {
        self.tours
            .iter()
            .map(|t| Ok(Walk::from_positions(t)?.closed()?))
            .collect()
    }

    /// The other tour with a position nearest to `p`, if there is one.
    pub fn nearest_other(&self, tour: usize, p: Point) -> Option<(usize, usize)> {
        let mut best: Option<(u64, usize, usize)> = None;

        for (t, positions) in self.tours.iter().enumerate().filter(|&(t, _)| t != tour) {
            for (i, &q) in positions.iter().enumerate() {
                let d = q.l1(p);
                if best.map_or(true, |(b, _, _)| d < b) {
                    best = Some((d, t, i));
                }
            }
        }

        best.map(|(_, t, i)| (t, i))
    }
}

/// What the search minimizes: the longest tour, plus a little of the rest.
pub fn cost(lengths: &[u64]) -> f64 {
    let max = lengths.iter().copied().max().unwrap_or(0);
    let sum: u64 = lengths.iter().sum();

    max as f64 + OTHERS_WEIGHT * (sum - max) as f64
}

/// The length of a dense closed tour: one step per position, none for a
/// tour standing on one point.
pub fn dense_length(positions: &[Point]) -> u64 {
    match positions.len() {
        0 | 1 => 0,
        n => n as u64,
    }
}

/// True if `positions` is a closed tour of unit steps, the way home included.
pub fn is_unit_cycle(positions: &[Point]) -> bool {
    let n = positions.len();
    n == 1 || (n >= 2 && (0..n).all(|i| positions[i].l1(positions[(i + 1) % n]) == 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_is_the_longest_plus_a_little() {
        assert_eq!(cost(&[10]), 10.0);
        assert_eq!(cost(&[10, 20]), 21.0);
    }

    #[test]
    fn unit_cycles() {
        let square = [(0, 0), (1, 0), (1, 1), (0, 1)].map(Point::from);

        assert!(is_unit_cycle(&square));
        assert!(!is_unit_cycle(&square[..3]));
        assert!(is_unit_cycle(&square[..2]));
        assert_eq!(dense_length(&square), 4);
        assert_eq!(dense_length(&square[..1]), 0);
    }
}
