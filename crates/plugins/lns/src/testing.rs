//! Small problems for the unit tests.

use mow_core::{BitGrid, Bounds, Cutter, Point, Problem};

/// A centered `side x side` square cutter.
pub fn square_cutter(side: i32) -> Cutter {
    let half = side / 2;
    let cells: Vec<Point> = (-half..side - half)
        .flat_map(|y| (-half..side - half).map(move |x| Point::new(x, y)))
        .collect();

    Cutter::new(BitGrid::from_cells(&cells).unwrap().unwrap()).unwrap()
}

/// A `width x height` rectangle at the origin, a square cutter of `side`.
pub fn rectangle(width: i32, height: i32, side: i32) -> Problem {
    rectangle_k(width, height, side, 1)
}

pub fn rectangle_k(width: i32, height: i32, side: i32, k: usize) -> Problem {
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

    Problem::new("rect", region, square_cutter(side), k).unwrap()
}
