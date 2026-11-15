//! A tour small enough for the terminal: the lawn as a grid of dots and
//! each cutter's path through its corners, both coarsened to about the
//! resolution a braille canvas can show.

use mow_core::{Bounds, Point, Problem, Route};

/// Dots across the lawn's wider side.
const REGION_DOTS: i64 = 160;

/// Corners closer than this many dots to the last kept one are dropped.
const PATH_DOTS: i64 = 4 * REGION_DOTS;

/// What the canvas draws, in world coordinates (y up).
#[derive(Debug, Clone, PartialEq)]
pub struct Preview {
    pub x: [f64; 2],
    pub y: [f64; 2],
    /// The centers of the dots holding lawn.
    pub region: Vec<(f64, f64)>,
    /// One closed path per cutter.
    pub routes: Vec<Vec<(f64, f64)>>,
}

impl Preview {
    pub fn new(problem: &Problem, routes: &[Route]) -> Self {
        let b = problem.region_bounds();
        let side = i64::from(b.width().max(b.height())).max(1);

        Self {
            x: [f64::from(b.x_min), f64::from(b.x_max) + 1.0],
            y: [f64::from(b.y_min), f64::from(b.y_max) + 1.0],
            region: region_dots(problem, b, side),
            routes: routes.iter().map(|r| path(r, side)).collect(),
        }
    }
}

/// The lawn on a grid of cells `side / REGION_DOTS` wide: a dot wherever
/// some lawn cell lies.
fn region_dots(problem: &Problem, b: Bounds, side: i64) -> Vec<(f64, f64)> {
    let cell = (side + REGION_DOTS - 1) / REGION_DOTS;
    let columns = (i64::from(b.width()) + cell - 1) / cell;
    let rows = (i64::from(b.height()) + cell - 1) / cell;
    let mut marked = vec![false; (columns * rows) as usize];

    for run in problem.region().runs() {
        let row = i64::from(run.y - b.y_min) / cell;
        let first = i64::from(run.x_min - b.x_min) / cell;
        let last = i64::from(run.x_max - b.x_min) / cell;
        let start = (row * columns) as usize;
        marked[start + first as usize..=start + last as usize].fill(true);
    }

    let center = |i: i64, low: i32| f64::from(low) + (i as f64 + 0.5) * cell as f64;
    (0..rows)
        .flat_map(|r| (0..columns).map(move |c| (r, c)))
        .filter(|&(r, c)| marked[(r * columns + c) as usize])
        .map(|(r, c)| (center(c, b.x_min), center(r, b.y_min)))
        .collect()
}

/// The route's corners, closed, keeping only corners that move at least
/// one path dot from the last one kept.
fn path(route: &Route, side: i64) -> Vec<(f64, f64)> {
    let step = (side / PATH_DOTS).max(1) as u64;
    let corners = route.corners();
    let first = corners.first().copied().unwrap_or(Point::new(0, 0));

    let mut kept: Vec<Point> = Vec::new();
    for &p in corners.iter().chain(std::iter::once(&first)) {
        let far = kept.last().map_or(true, |q| q.l1(p) >= step);
        if far {
            kept.push(p);
        }
    }
    if kept.last() != Some(&first) {
        kept.push(first);
    }

    kept.iter()
        .map(|p| (f64::from(p.x) + 0.5, f64::from(p.y) + 0.5))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mow_core::{BitGrid, Cutter};

    fn problem(width: i32, height: i32) -> Problem {
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
        let cutter = Cutter::new(BitGrid::from_cells(&[Point::new(0, 0)]).unwrap().unwrap()).unwrap();

        Problem::new("p", region, cutter, 1).unwrap()
    }

    #[test]
    fn a_small_lawn_gets_a_dot_per_cell_and_a_closed_path() {
        let p = problem(4, 3);
        let corners = vec![
            Point::new(0, 0),
            Point::new(3, 0),
            Point::new(3, 2),
            Point::new(0, 2),
        ];
        let route = Route::new(corners).unwrap();

        let preview = Preview::new(&p, &[route]);

        assert_eq!(preview.region.len(), 12);
        assert_eq!(preview.x, [0.0, 4.0]);
        let path = &preview.routes[0];
        assert_eq!(path.first(), path.last());
        assert_eq!(path.len(), 5);
    }

    #[test]
    fn a_large_lawn_is_coarsened() {
        let p = problem(2000, 500);

        let preview = Preview::new(&p, &[]);

        // 2000 / 160 rounds up to 13 cells a dot: 154 x 39 dots
        assert_eq!(preview.region.len(), 154 * 39);
        assert!(preview.routes.is_empty());
    }
}
