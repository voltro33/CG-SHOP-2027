//! An open walk: the shape a tour has before it is split and closed.

use crate::error::ModelError;
use crate::geom::Point;
use crate::route::{connect, passes_straight, Route};

/// An open path. Every consecutive pair of corners is axis parallel; the
/// last and the first need not be. Both ends always stay where they are.
///
/// Positions are never stored: [`Walk::positions`] walks them lazily.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Walk {
    corners: Vec<Point>,
}

impl Walk {
    /// Takes the corners as they are and checks them.
    pub fn new(corners: Vec<Point>) -> Result<Self, ModelError> {
        if corners.is_empty() {
            return Err(ModelError::EmptyRoute);
        }

        if let Some(pair) = corners.windows(2).find(|w| !w[0].is_axis_parallel_to(w[1])) {
            return Err(ModelError::NotAxisParallel(pair[0], pair[1]));
        }

        Ok(Self { corners })
    }

    /// A walk through `points` in order: repeats dropped, a bend inserted
    /// (horizontal first) where two points differ in both coordinates.
    /// This is `connect` from `solver/tours.py`.
    pub fn through(points: &[Point]) -> Result<Self, ModelError> {
        Self::new(connect(points))
    }

    /// The corners of a unit step path: its ends plus every point where the
    /// direction changes, reversals included. This is `compress`.
    pub fn from_positions(positions: &[Point]) -> Result<Self, ModelError> {
        let Some(&first) = positions.first() else {
            return Err(ModelError::EmptyRoute);
        };

        let mut corners = vec![first];
        for w in positions.windows(3) {
            let before = (w[1].x - w[0].x, w[1].y - w[0].y);
            let after = (w[2].x - w[1].x, w[2].y - w[1].y);

            if before != after {
                corners.push(w[1]);
            }
        }

        if positions.len() > 1 {
            corners.push(positions[positions.len() - 1]);
        }

        Self::new(corners)
    }

    pub fn corners(&self) -> &[Point] {
        &self.corners
    }

    pub fn into_corners(self) -> Vec<Point> {
        self.corners
    }

    pub fn start(&self) -> Point {
        self.corners[0]
    }

    pub fn end(&self) -> Point {
        self.corners[self.corners.len() - 1]
    }

    pub fn length(&self) -> u64 {
        self.edges().map(|(a, b)| a.l1(b)).sum()
    }

    /// Every edge, borrowing the walk. No closing edge.
    pub fn edges(&self) -> impl Iterator<Item = (Point, Point)> + '_ {
        self.corners.windows(2).map(|w| (w[0], w[1]))
    }

    /// Every unit step point, both ends included: `length() + 1` points.
    pub fn positions(&self) -> impl Iterator<Item = Point> + '_ {
        let steps = self.edges().flat_map(|(a, b)| {
            let (dx, dy) = ((b.x - a.x).signum(), (b.y - a.y).signum());
            (1..=a.l1(b) as i32).map(move |t| a.shifted(dx * t, dy * t))
        });

        std::iter::once(self.start()).chain(steps)
    }

    /// The same positions with repeated and straight-through corners
    /// dropped. Both ends stay. This is `_normalize_open` in `improve.py`.
    pub fn normalized(self) -> Self {
        let mut result: Vec<Point> = Vec::with_capacity(self.corners.len());

        for p in self.corners {
            if result.last() == Some(&p) {
                continue;
            }

            while result.len() >= 2 && passes_straight(result[result.len() - 2], result[result.len() - 1], p)
            {
                result.pop();
            }

            result.push(p);
        }

        Self { corners: result }
    }

    /// Back to the start: a bend if the way home is not axis parallel, then
    /// normalized as a cycle. This is `close_route`.
    pub fn closed(&self) -> Result<Route, ModelError> {
        Route::through(&self.corners)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pts(raw: &[(i32, i32)]) -> Vec<Point> {
        raw.iter().map(|&p| p.into()).collect()
    }

    #[test]
    fn positions_include_both_ends() {
        let walk = Walk::new(pts(&[(0, 0), (2, 0), (2, 1)])).unwrap();
        let positions: Vec<Point> = walk.positions().collect();

        assert_eq!(walk.length(), 3);
        assert_eq!(positions, pts(&[(0, 0), (1, 0), (2, 0), (2, 1)]));
        assert_eq!(Walk::new(pts(&[(3, 3)])).unwrap().positions().count(), 1);
    }

    #[test]
    fn from_positions_keeps_turns_and_reversals() {
        let positions = pts(&[(0, 0), (1, 0), (2, 0), (1, 0), (1, 1), (1, 2)]);
        let walk = Walk::from_positions(&positions).unwrap();

        assert_eq!(walk.corners(), pts(&[(0, 0), (2, 0), (1, 0), (1, 2)]).as_slice());
        assert_eq!(walk.positions().collect::<Vec<_>>(), positions);
    }

    #[test]
    fn through_bends_and_rejects_nothing() {
        let walk = Walk::through(&pts(&[(0, 0), (0, 0), (3, 2)])).unwrap();

        assert_eq!(walk.corners(), pts(&[(0, 0), (3, 0), (3, 2)]).as_slice());
        assert!(Walk::new(pts(&[(0, 0), (1, 1)])).is_err());
        assert!(Walk::new(Vec::new()).is_err());
    }

    #[test]
    fn normalized_keeps_the_ends() {
        let walk = Walk::new(pts(&[(0, 0), (1, 0), (1, 0), (3, 0), (3, 2), (3, 4)])).unwrap();

        assert_eq!(
            walk.normalized().corners(),
            pts(&[(0, 0), (3, 0), (3, 4)]).as_slice()
        );
    }

    #[test]
    fn closed_adds_the_way_home() {
        let route = Walk::new(pts(&[(0, 0), (2, 0), (2, 2)]))
            .unwrap()
            .closed()
            .unwrap();

        assert_eq!(route.corners(), pts(&[(0, 0), (2, 0), (2, 2), (0, 2)]).as_slice());
        assert_eq!(route.length(), 8);
    }
}
