//! A closed tour, stored as its corners, as the solution file stores it.

use crate::error::ModelError;
use crate::geom::Point;

/// A closed route. Every consecutive pair of corners, the last and the first
/// included, is axis parallel, so the route is a list of unit steps.
///
/// Positions are never stored: [`Route::positions`] walks them lazily.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    corners: Vec<Point>,
}

impl Route {
    /// Takes the corners as they are and checks them.
    pub fn new(corners: Vec<Point>) -> Result<Self, ModelError> {
        if corners.is_empty() {
            return Err(ModelError::EmptyRoute);
        }

        if let Some((a, b)) = cyclic_pairs(&corners).find(|(a, b)| !a.is_axis_parallel_to(*b)) {
            return Err(ModelError::NotAxisParallel(a, b));
        }

        Ok(Self { corners })
    }

    /// A route through `points` in order, back to the first: bends are
    /// inserted (horizontal first) where two points differ in both
    /// coordinates, then repeated and straight-through corners are dropped.
    /// This is `close_route` from `solver/tours.py`.
    pub fn through(points: &[Point]) -> Result<Self, ModelError> {
        let mut corners = connect(points);

        if let (Some(&first), Some(&last)) = (corners.first(), corners.last()) {
            if !first.is_axis_parallel_to(last) {
                corners.push(Point::new(first.x, last.y));
            }
        }

        Self::new(normalized(corners))
    }

    pub fn corners(&self) -> &[Point] {
        &self.corners
    }

    pub fn into_corners(self) -> Vec<Point> {
        self.corners
    }

    /// The first corner, where the tour starts and ends.
    pub fn start(&self) -> Point {
        self.corners[0]
    }

    pub fn length(&self) -> u64 {
        self.edges().map(|(a, b)| a.l1(b)).sum()
    }

    /// Every edge, the closing one included, borrowing the route.
    pub fn edges(&self) -> impl Iterator<Item = (Point, Point)> + '_ {
        cyclic_pairs(&self.corners)
    }

    /// Every unit step point once, starting at [`Route::start`] and not
    /// returning to it. A route of one corner yields that corner.
    pub fn positions(&self) -> Positions<'_> {
        Positions {
            route: self,
            edge: 0,
            step: 0,
            single: self.corners.len() == 1,
        }
    }

    /// The same positions with repeated and straight-through corners dropped.
    pub fn normalized(self) -> Self {
        Self {
            corners: normalized(self.corners),
        }
    }
}

/// Lazy unit steps of a [`Route`].
pub struct Positions<'a> {
    route: &'a Route,
    edge: usize,
    step: u64,
    single: bool,
}

impl Iterator for Positions<'_> {
    type Item = Point;

    fn next(&mut self) -> Option<Point> {
        let corners = &self.route.corners;

        if self.single {
            self.single = false;
            self.edge = corners.len();
            return Some(corners[0]);
        }

        while self.edge < corners.len() {
            let a = corners[self.edge];
            let b = corners[(self.edge + 1) % corners.len()];

            if self.step < a.l1(b) {
                let t = self.step as i32;
                self.step += 1;
                return Some(a.shifted((b.x - a.x).signum() * t, (b.y - a.y).signum() * t));
            }

            self.edge += 1;
            self.step = 0;
        }

        None
    }
}

/// Consecutive pairs, wrapping around. One corner pairs with itself.
fn cyclic_pairs(corners: &[Point]) -> impl Iterator<Item = (Point, Point)> + '_ {
    let next = corners.iter().cycle().skip(1);
    corners.iter().zip(next).map(|(&a, &b)| (a, b))
}

/// `connect` from `solver/tours.py`: drops repeats, inserts horizontal-first bends.
pub(crate) fn connect(points: &[Point]) -> Vec<Point> {
    let mut result: Vec<Point> = Vec::with_capacity(points.len() * 2);

    for &p in points {
        match result.last() {
            Some(&prev) if prev == p => continue,
            Some(&prev) if !prev.is_axis_parallel_to(p) => result.push(Point::new(p.x, prev.y)),
            _ => {}
        }

        result.push(p);
    }

    result
}

/// Removes, cyclically, corners equal to their successor and corners
/// strictly on a straight run between their neighbours, until none is left.
fn normalized(mut corners: Vec<Point>) -> Vec<Point> {
    loop {
        let n = corners.len();
        if n <= 1 {
            return corners;
        }

        let removable = |i: usize| {
            let prev = corners[(i + n - 1) % n];
            let cur = corners[i];
            let next = corners[(i + 1) % n];

            cur == next || (n > 2 && passes_straight(prev, cur, next))
        };

        match (0..n).find(|&i| removable(i)) {
            Some(i) => {
                corners.remove(i);
            }
            None => return corners,
        }
    }
}

/// True if `b` lies strictly on the way from `a` to `c` along one axis.
pub(crate) fn passes_straight(a: Point, b: Point, c: Point) -> bool {
    let between = |p: i32, q: i32, r: i32| (p < q && q < r) || (p > q && q > r);

    (a.x == b.x && b.x == c.x && between(a.y, b.y, c.y))
        || (a.y == b.y && b.y == c.y && between(a.x, b.x, c.x))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pts(raw: &[(i32, i32)]) -> Vec<Point> {
        raw.iter().map(|&p| p.into()).collect()
    }

    #[test]
    fn square_length_and_positions() {
        let route = Route::new(pts(&[(0, 0), (2, 0), (2, 1), (0, 1)])).unwrap();
        let positions: Vec<Point> = route.positions().collect();

        assert_eq!(route.length(), 6);
        assert_eq!(positions.len(), 6);
        assert_eq!(positions[0], Point::new(0, 0));
        assert_eq!(positions[5], Point::new(0, 1));
    }

    #[test]
    fn single_corner_route_stands_still() {
        let route = Route::new(pts(&[(4, 4)])).unwrap();

        assert_eq!(route.length(), 0);
        assert_eq!(route.positions().collect::<Vec<_>>(), pts(&[(4, 4)]));
    }

    #[test]
    fn rejects_diagonal_corners_including_the_closing_edge() {
        assert!(Route::new(pts(&[(0, 0), (1, 1)])).is_err());
        assert!(Route::new(pts(&[(0, 0), (2, 0), (2, 2)])).is_err());
        assert!(Route::new(Vec::new()).is_err());
    }

    #[test]
    fn through_bends_closes_and_normalizes() {
        // (0,0) -> (3,2) bends at (3,0); the way back from (3,2) bends at (0,2)
        let route = Route::through(&pts(&[(0, 0), (1, 0), (3, 2), (3, 2)])).unwrap();

        assert_eq!(route.corners(), pts(&[(0, 0), (3, 0), (3, 2), (0, 2)]).as_slice());
        assert_eq!(route.length(), 10);
    }

    #[test]
    fn reversals_are_kept_since_they_matter_for_coverage() {
        let corners = pts(&[(0, 0), (5, 0), (2, 0), (2, 3), (0, 3)]);
        let route = Route::new(corners.clone()).unwrap().normalized();

        assert_eq!(route.corners(), corners.as_slice());
        assert_eq!(route.length(), 16);
    }

    #[test]
    fn out_and_back_collapses_to_two_corners() {
        let route = Route::through(&pts(&[(0, 0), (2, 0), (4, 0), (2, 0)])).unwrap();

        assert_eq!(route.corners(), pts(&[(0, 0), (4, 0)]).as_slice());
    }
}
