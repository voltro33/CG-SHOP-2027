//! The unit step positions of walks and routes as straight stretches, so the
//! position nearest a cell is found per edge instead of per position, with
//! the same answer as Python's `argmin` over the dense positions.

use mow_core::{Point, Route, Walk};

/// The positions `start + s * (dx, dy)` for `s` in `0..count`, which are
/// positions `first..first + count` of the path `owner`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stretch {
    pub start: Point,
    pub dx: i32,
    pub dy: i32,
    pub count: usize,
    pub owner: usize,
    pub first: usize,
}

impl Stretch {
    pub fn at(&self, s: usize) -> Point {
        let s = s as i32;
        self.start.shifted(self.dx * s, self.dy * s)
    }

    pub fn last(&self) -> Point {
        self.at(self.count - 1)
    }

    /// The step whose position is closest to `cell` by L1, and its distance.
    /// The moving coordinate's distance is `|linear in s|`, so clamping the
    /// exact target gives the unique best step.
    fn nearest(&self, cell: Point) -> (usize, u64) {
        let target = i64::from(cell.x - self.start.x) * i64::from(self.dx)
            + i64::from(cell.y - self.start.y) * i64::from(self.dy);
        let s = target.clamp(0, self.count as i64 - 1) as usize;

        (s, self.at(s).l1(cell))
    }
}

/// Where the nearest position is: which path, which of its positions, and
/// the position itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Nearest {
    pub owner: usize,
    pub index: usize,
    pub point: Point,
}

/// The stretches of one or more paths, in position order.
#[derive(Debug, Clone, Default)]
pub struct Stretches {
    pub items: Vec<Stretch>,
}

impl Stretches {
    /// `densify(walk)`: the start, then every step of every edge.
    pub fn of_walk(walk: &Walk) -> Self {
        let mut stretches = Self::default();
        stretches.push(walk.start(), (0, 0), 1, 0, 0);

        let mut first = 1;
        for (a, b) in walk.edges() {
            let (dx, dy) = ((b.x - a.x).signum(), (b.y - a.y).signum());
            let count = a.l1(b) as usize;

            stretches.push(a.shifted(dx, dy), (dx, dy), count, 0, first);
            first += count;
        }

        stretches
    }

    /// `densify(route, closed=True)` for each route, joined in order: every
    /// edge from its first corner, never back to the start.
    pub fn of_routes(routes: &[Route]) -> Self {
        let mut stretches = Self::default();

        for (owner, route) in routes.iter().enumerate() {
            if route.corners().len() == 1 {
                stretches.push(route.start(), (0, 0), 1, owner, 0);
                continue;
            }

            let mut first = 0;
            for (a, b) in route.edges() {
                let (dx, dy) = ((b.x - a.x).signum(), (b.y - a.y).signum());
                let count = a.l1(b) as usize;

                stretches.push(a, (dx, dy), count, owner, first);
                first += count;
            }
        }

        stretches
    }

    fn push(&mut self, start: Point, (dx, dy): (i32, i32), count: usize, owner: usize, first: usize) {
        if count > 0 {
            self.items.push(Stretch {
                start,
                dx,
                dy,
                count,
                owner,
                first,
            });
        }
    }

    /// The first position, in position order, of smallest L1 distance to
    /// `cell` (`np.argmin` over the dense positions). `None` if there are no
    /// positions at all.
    pub fn nearest(&self, cell: Point) -> Option<Nearest> {
        let mut best: Option<(u64, Nearest)> = None;

        for stretch in &self.items {
            let (s, distance) = stretch.nearest(cell);
            if best.is_some_and(|(d, _)| d <= distance) {
                continue;
            }

            let found = Nearest {
                owner: stretch.owner,
                index: stretch.first + s,
                point: stretch.at(s),
            };
            best = Some((distance, found));
        }

        best.map(|(_, found)| found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pts(raw: &[(i32, i32)]) -> Vec<Point> {
        raw.iter().map(|&p| p.into()).collect()
    }

    /// `argmin` over the dense positions, as Python does it.
    fn brute(positions: &[Point], cell: Point) -> usize {
        let mut best = 0;
        for (i, p) in positions.iter().enumerate() {
            if p.l1(cell) < positions[best].l1(cell) {
                best = i;
            }
        }
        best
    }

    #[test]
    fn walk_nearest_equals_argmin_with_first_ties() {
        let walk = Walk::new(pts(&[(0, 0), (6, 0), (6, 4), (0, 4), (0, 0), (3, 0)])).unwrap();
        let positions: Vec<Point> = walk.positions().collect();
        let stretches = Stretches::of_walk(&walk);

        for x in -2..9 {
            for y in -2..7 {
                let cell = Point::new(x, y);
                let found = stretches.nearest(cell).unwrap();

                assert_eq!(found.index, brute(&positions, cell), "{cell:?}");
                assert_eq!(found.point, positions[found.index]);
            }
        }
    }

    #[test]
    fn routes_are_joined_in_order() {
        let routes = vec![
            Route::new(pts(&[(0, 0), (4, 0), (4, 2), (0, 2)])).unwrap(),
            Route::new(pts(&[(2, 1)])).unwrap(),
            Route::new(pts(&[(0, 0), (4, 0)])).unwrap(),
        ];
        let joined: Vec<(usize, usize, Point)> = routes
            .iter()
            .enumerate()
            .flat_map(|(o, r)| r.positions().enumerate().map(move |(i, p)| (o, i, p)))
            .collect();
        let positions: Vec<Point> = joined.iter().map(|j| j.2).collect();
        let stretches = Stretches::of_routes(&routes);

        for x in -1..6 {
            for y in -1..4 {
                let cell = Point::new(x, y);
                let found = stretches.nearest(cell).unwrap();
                let (owner, index, point) = joined[brute(&positions, cell)];

                assert_eq!((found.owner, found.index, found.point), (owner, index, point));
            }
        }
    }
}
