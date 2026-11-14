//! How far apart the anchor sets of two cells are.
//!
//! The anchors that sweep cell `w` are `w - C`. For two cells `a` and `b`,
//! the nearest pair of such anchors is `min |(a - b) - δ|` over `δ` in the
//! difference set `D = C - C`, so one L1 distance transform of `D` answers
//! every pair.

use crate::cutter::Cutter;
use crate::geom::{Bounds, Point};
use crate::grid::BitGrid;

/// The L1 distance from any vector to the cutter's difference set.
#[derive(Debug, Clone)]
pub struct Reach {
    frame: Bounds,
    width: usize,
    distance: Vec<u32>,
}

impl Reach {
    pub fn new(cutter: &Cutter) -> Self {
        let differences = difference_set(cutter);
        let frame = differences.frame();
        let width = frame.width() as usize;

        Self {
            frame,
            width,
            distance: distance_transform(&differences),
        }
    }

    /// The shortest L1 distance between an anchor sweeping `a` and an
    /// anchor sweeping `b`; 0 when one anchor sweeps both.
    pub fn between(&self, a: Point, b: Point) -> u64 {
        let v = (i64::from(a.x) - i64::from(b.x), i64::from(a.y) - i64::from(b.y));
        let f = self.frame;

        // outside the frame, the way in is straight to the nearest frame point
        let x = v.0.clamp(i64::from(f.x_min), i64::from(f.x_max));
        let y = v.1.clamp(i64::from(f.y_min), i64::from(f.y_max));
        let outside = (v.0 - x).unsigned_abs() + (v.1 - y).unsigned_abs();

        let column = (x - i64::from(f.x_min)) as usize;
        let row = (y - i64::from(f.y_min)) as usize;

        outside + u64::from(self.distance[row * self.width + column])
    }
}

/// `C - C`, one interval per pair of cutter runs.
fn difference_set(cutter: &Cutter) -> BitGrid {
    let b = cutter.bounds();
    let (span_x, span_y) = (b.x_max - b.x_min, b.y_max - b.y_min);

    let mut set = BitGrid::empty(Bounds {
        x_min: -span_x,
        y_min: -span_y,
        x_max: span_x,
        y_max: span_y,
    })
    .expect("a cutter's difference set fits a grid");

    let runs: Vec<_> = cutter.cells().runs().collect();
    for first in &runs {
        for second in &runs {
            set.insert_run(
                first.y - second.y,
                first.x_min - second.x_max,
                first.x_max - second.x_min,
            );
        }
    }

    set
}

/// Two raster passes; exact for the L1 metric on a box.
fn distance_transform(set: &BitGrid) -> Vec<u32> {
    let frame = set.frame();
    let (width, height) = (frame.width() as usize, frame.height() as usize);
    let far = (width + height) as u32;

    let mut distance: Vec<u32> = (0..width * height)
        .map(|i| {
            let p = Point::new(frame.x_min + (i % width) as i32, frame.y_min + (i / width) as i32);
            if set.contains(p) {
                0
            } else {
                far
            }
        })
        .collect();

    for i in 0..width * height {
        let (column, row) = (i % width, i / width);
        if column > 0 {
            distance[i] = distance[i].min(distance[i - 1] + 1);
        }
        if row > 0 {
            distance[i] = distance[i].min(distance[i - width] + 1);
        }
    }

    for i in (0..width * height).rev() {
        let (column, row) = (i % width, i / width);
        if column + 1 < width {
            distance[i] = distance[i].min(distance[i + 1] + 1);
        }
        if row + 1 < height {
            distance[i] = distance[i].min(distance[i + width] + 1);
        }
    }

    distance
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(side: i32) -> Cutter {
        let half = side / 2;
        let cells: Vec<Point> = (-half..side - half)
            .flat_map(|y| (-half..side - half).map(move |x| Point::new(x, y)))
            .collect();
        Cutter::new(BitGrid::from_cells(&cells).unwrap().unwrap()).unwrap()
    }

    #[test]
    fn single_cell_cutter_is_plain_l1() {
        let reach = Reach::new(&square(1));

        assert_eq!(reach.between(Point::new(0, 0), Point::new(3, -4)), 7);
        assert_eq!(reach.between(Point::new(2, 2), Point::new(2, 2)), 0);
    }

    #[test]
    fn square_cutter_reaches_its_side_minus_one_each_way() {
        let reach = Reach::new(&square(3));

        // anchors x in -1..=1 and 9..=11: 8 apart
        assert_eq!(reach.between(Point::new(0, 0), Point::new(10, 0)), 8);
        assert_eq!(reach.between(Point::new(0, 0), Point::new(10, 5)), 8 + 3);
        assert_eq!(reach.between(Point::new(0, 0), Point::new(2, -2)), 0);
    }

    #[test]
    fn x_cutter_reach_follows_its_arms() {
        // the X of side 3: one cutter covers (-1,-1) and (1,1), never (0,1) and (1,1)
        let cells: Vec<Point> = [(-1, -1), (1, -1), (0, 0), (-1, 1), (1, 1)]
            .iter()
            .map(|&c| c.into())
            .collect();
        let reach = Reach::new(&Cutter::new(BitGrid::from_cells(&cells).unwrap().unwrap()).unwrap());

        assert_eq!(reach.between(Point::new(0, 0), Point::new(2, 2)), 0);
        assert_eq!(reach.between(Point::new(0, 0), Point::new(1, 0)), 1);
    }
}
