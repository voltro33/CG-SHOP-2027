//! Rings of lattice points and their cells, as `cgshop2027_pyutils.grid._rasterize`.
//!
//! A cell is inside when an odd number of vertical edges lie at or west of
//! its left side, counting only edges that span its row. Holes go through
//! the same count, and orientation does not matter.

use mow_core::{BitGrid, Bounds, Point};

use crate::error::{IoError, RingError};

/// A closed rectilinear boundary, checked: at least 4 points and every
/// edge, the closing one included, horizontal or vertical.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ring {
    points: Vec<Point>,
}

impl Ring {
    pub fn new(points: Vec<Point>) -> Result<Self, RingError> {
        if points.len() < 4 {
            return Err(RingError::TooFewPoints(points.len()));
        }

        if let Some((a, b)) = edges(&points).find(|(a, b)| !a.is_axis_parallel_to(*b)) {
            return Err(RingError::NotOrthogonal(a, b));
        }

        Ok(Self { points })
    }

    /// A ring from the file's separate coordinate lists.
    pub fn from_coordinates(x: &[i32], y: &[i32]) -> Result<Self, RingError> {
        if x.len() != y.len() {
            return Err(RingError::LengthMismatch {
                x: x.len(),
                y: y.len(),
            });
        }

        Self::new(x.iter().zip(y).map(|(&x, &y)| Point::new(x, y)).collect())
    }

    pub fn points(&self) -> &[Point] {
        &self.points
    }

    /// Every edge, the closing one included.
    pub fn edges(&self) -> impl Iterator<Item = (Point, Point)> + '_ {
        edges(&self.points)
    }
}

/// A vertical edge at `x`, crossed by rows `low..high`.
#[derive(Debug, Clone, Copy)]
struct Wall {
    x: i32,
    low: i32,
    high: i32,
}

/// The cells enclosed by `rings`, holes included as rings. The grid's frame
/// is the box of all points, as in Python: `x_min..x_max - 1` by
/// `y_min..y_max - 1`.
///
/// Works row by row with a sorted list of the walls crossing it, so memory
/// is the bitset plus the walls, never one counter per cell.
pub fn rasterize_rings(rings: &[Ring]) -> Result<BitGrid, IoError> {
    let frame = point_box(rings)?;
    let mut grid = BitGrid::empty(frame)?;

    let walls: Vec<Wall> = rings.iter().flat_map(walls).collect();
    let mut starts = walls.clone();
    let mut ends = walls;
    starts.sort_unstable_by_key(|w| w.low);
    ends.sort_unstable_by_key(|w| w.high);

    let (mut next_start, mut next_end) = (0, 0);
    let mut crossing: Vec<i32> = Vec::new();

    for y in frame.y_min..=frame.y_max {
        while let Some(wall) = ends.get(next_end).filter(|w| w.high <= y) {
            remove_sorted(&mut crossing, wall.x);
            next_end += 1;
        }

        while let Some(wall) = starts.get(next_start).filter(|w| w.low <= y) {
            insert_sorted(&mut crossing, wall.x);
            next_start += 1;
        }

        fill_row(&mut grid, y, &crossing, frame.x_max);
    }

    Ok(grid)
}

/// Cells between the 1st and 2nd crossing are inside, between the 3rd and
/// 4th, and so on. An unpaired last crossing runs to the frame's edge, as
/// Python's prefix sum would.
fn fill_row(grid: &mut BitGrid, y: i32, crossing: &[i32], x_last: i32) {
    for pair in crossing.chunks(2) {
        match *pair {
            [from, to] if from < to => grid.insert_run(y, from, to - 1),
            [from] => grid.insert_run(y, from, x_last),
            _ => {}
        }
    }
}

/// The box of the cells: from the smallest coordinates to one less than
/// the largest.
fn point_box(rings: &[Ring]) -> Result<Bounds, IoError> {
    let mut points = rings.iter().flat_map(|r| r.points.iter());
    let first = *points.next().ok_or(IoError::NoRings)?;

    let start = (first.x, first.y, first.x, first.y);
    let (x_min, y_min, x_max, y_max) = points.fold(start, |(x0, y0, x1, y1), p| {
        (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y))
    });

    if x_min == x_max || y_min == y_max {
        return Err(IoError::Ring {
            ring: "rings".into(),
            reason: RingError::Degenerate,
        });
    }

    Ok(Bounds {
        x_min,
        y_min,
        x_max: x_max - 1,
        y_max: y_max - 1,
    })
}

fn walls(ring: &Ring) -> impl Iterator<Item = Wall> + '_ {
    ring.edges().filter(|(a, b)| a.y != b.y).map(|(a, b)| Wall {
        x: a.x,
        low: a.y.min(b.y),
        high: a.y.max(b.y),
    })
}

fn insert_sorted(values: &mut Vec<i32>, value: i32) {
    let index = values.partition_point(|&v| v < value);
    values.insert(index, value);
}

/// Removes one copy of `value`, which is always present: a wall ends only
/// after it started.
fn remove_sorted(values: &mut Vec<i32>, value: i32) {
    if let Ok(index) = values.binary_search(&value) {
        values.remove(index);
    }
}

fn edges(points: &[Point]) -> impl Iterator<Item = (Point, Point)> + '_ {
    let next = points.iter().cycle().skip(1);
    points.iter().zip(next).map(|(&a, &b)| (a, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ring(points: &[(i32, i32)]) -> Ring {
        Ring::new(points.iter().map(|&p| p.into()).collect()).unwrap()
    }

    fn cells(grid: &BitGrid) -> Vec<(i32, i32)> {
        grid.cells().map(|p| (p.x, p.y)).collect()
    }

    fn square(x0: i32, y0: i32, side: i32) -> Ring {
        ring(&[(x0, y0), (x0 + side, y0), (x0 + side, y0 + side), (x0, y0 + side)])
    }

    #[test]
    fn square_fills_its_box() {
        let grid = rasterize_rings(&[square(-1, 2, 3)]).unwrap();

        assert_eq!(grid.len(), 9);
        assert_eq!(
            grid.frame(),
            Bounds {
                x_min: -1,
                y_min: 2,
                x_max: 1,
                y_max: 4
            }
        );
        assert_eq!(grid.cell_bounds(), Some(grid.frame()));
    }

    #[test]
    fn l_shape() {
        // 2 x 2 square with a 1 x 1 bite out of the top right
        let grid = rasterize_rings(&[ring(&[(0, 0), (2, 0), (2, 1), (1, 1), (1, 2), (0, 2)])]).unwrap();

        assert_eq!(cells(&grid), vec![(0, 0), (1, 0), (0, 1)]);
    }

    #[test]
    fn hole_is_left_out() {
        let outer = square(0, 0, 4);
        let hole = ring(&[(1, 1), (1, 3), (3, 3), (3, 1)]);
        let grid = rasterize_rings(&[outer, hole]).unwrap();

        assert_eq!(grid.len(), 12);
        assert!(!grid.contains(Point::new(1, 1)));
        assert!(!grid.contains(Point::new(2, 2)));
        assert!(grid.contains(Point::new(3, 3)));
    }

    #[test]
    fn orientation_does_not_matter() {
        let ccw = [(0, 0), (3, 0), (3, 1), (1, 1), (1, 3), (0, 3)];
        let cw: Vec<(i32, i32)> = ccw.iter().rev().copied().collect();

        let a = rasterize_rings(&[ring(&ccw)]).unwrap();
        let b = rasterize_rings(&[ring(&cw)]).unwrap();

        assert_eq!(cells(&a), cells(&b));
        assert_eq!(a.len(), 5);
    }

    #[test]
    fn walls_sharing_a_column_cancel() {
        // two squares touching along x = 2: the shared wall is crossed twice
        let grid = rasterize_rings(&[square(0, 0, 2), square(2, 0, 2)]).unwrap();

        assert_eq!(grid.len(), 8);
        assert_eq!(grid.runs().count(), 2);
    }

    #[test]
    fn wide_rows_cross_word_borders() {
        let grid = rasterize_rings(&[ring(&[(0, 0), (200, 0), (200, 2), (0, 2)])]).unwrap();

        assert_eq!(grid.len(), 400);
        assert_eq!(grid.stride(), 4);
    }

    #[test]
    fn bad_rings_are_errors_not_panics() {
        let p = |x, y| Point::new(x, y);

        assert_eq!(
            Ring::new(vec![p(0, 0), p(1, 0), p(1, 1)]),
            Err(RingError::TooFewPoints(3))
        );
        assert_eq!(
            Ring::new(vec![p(0, 0), p(2, 0), p(2, 2), p(1, 1)]),
            Err(RingError::NotOrthogonal(p(2, 2), p(1, 1)))
        );
        assert_eq!(
            Ring::from_coordinates(&[0, 1], &[0]),
            Err(RingError::LengthMismatch { x: 2, y: 1 })
        );

        let flat = ring(&[(0, 0), (1, 0), (2, 0), (3, 0)]);
        assert!(matches!(rasterize_rings(&[flat]), Err(IoError::Ring { .. })));
        assert!(matches!(rasterize_rings(&[]), Err(IoError::NoRings)));
    }
}
