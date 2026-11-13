//! Shared helpers: hand made problems, a tiny instance and solution loader,
//! the fixture recipes, and a cell by cell reference sweep.

#![allow(dead_code)]

pub mod load;

use std::collections::HashSet;

use mow_core::{BitGrid, Bounds, Cutter, Point, Problem, Route};

pub fn pts(raw: &[(i32, i32)]) -> Vec<Point> {
    raw.iter().map(|&p| p.into()).collect()
}

pub fn route(raw: &[(i32, i32)]) -> Route {
    Route::new(pts(raw)).unwrap()
}

pub fn cutter(offsets: &[(i32, i32)]) -> Cutter {
    Cutter::new(BitGrid::from_cells(&pts(offsets)).unwrap().unwrap()).unwrap()
}

pub fn square(side: i32) -> Cutter {
    let half = side / 2;
    let offsets: Vec<(i32, i32)> = (-half..side - half)
        .flat_map(|y| (-half..side - half).map(move |x| (x, y)))
        .collect();
    cutter(&offsets)
}

pub fn x_shape() -> Cutter {
    cutter(&[(-1, -1), (1, -1), (0, 0), (-1, 1), (1, 1)])
}

pub fn plus() -> Cutter {
    cutter(&[(0, -1), (-1, 0), (0, 0), (1, 0), (0, 1)])
}

/// A `width x height` rectangle at the origin.
pub fn rectangle(width: i32, height: i32, cutter: Cutter, k: usize) -> Problem {
    let frame = Bounds {
        x_min: 0,
        y_min: 0,
        x_max: width - 1,
        y_max: height - 1,
    };
    let mut region = BitGrid::empty(frame).unwrap();
    for y in 0..height {
        region.insert_run(y, 0, width - 1);
    }

    Problem::new("rect", region, cutter, k).unwrap()
}

/// The region cells no position of any route sweeps, one cell at a time.
pub fn reference_uncovered(problem: &Problem, routes: &[Route]) -> HashSet<Point> {
    let mut left: HashSet<Point> = problem.region().cells().collect();

    for route in routes {
        for p in route.positions() {
            for o in problem.cutter().offsets() {
                left.remove(&p.shifted(o.x, o.y));
            }
        }
    }

    left
}

/// A small xorshift, so the tests need no random crate.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    pub fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n.max(1) as u64) as usize
    }
}
