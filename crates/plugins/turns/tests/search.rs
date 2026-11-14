//! The whole search on scrambled lane tours with random detours: the
//! longest tour gets shorter, no tour grows past the old longest, every
//! cell stays swept.

use std::time::{Duration, Instant};

use mow_core::{BitGrid, Bounds, Cutter, Point, Problem, Route};
use mow_coverage::is_covered;
use mow_turns::{turns_routes, TurnsOptions};

/// A small xorshift, so the test needs no random crate.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n.max(1) as u64) as usize
    }
}

/// A `width x height` rectangle with a square hole, mowed by a 3 x 3 square.
fn holed(width: i32, height: i32, k: usize) -> Problem {
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
    for y in height / 3..height / 2 {
        for x in width / 3..width / 2 {
            region.remove(Point::new(x, y));
        }
    }

    let offsets: Vec<Point> = (-1..=1)
        .flat_map(|y| (-1..=1).map(move |x| Point::new(x, y)))
        .collect();
    let cutter = Cutter::new(BitGrid::from_cells(&offsets).unwrap().unwrap()).unwrap();
    Problem::new("holed", region, cutter, k).unwrap()
}

/// Lanes three rows apart over the whole width, dealt to `k` tours in a
/// shuffled order, each driven a random way, with random spikes outward.
fn scrambled(width: i32, height: i32, k: usize, rng: &mut Rng) -> Vec<Route> {
    let mut lanes: Vec<i32> = (1..height - 1).step_by(3).collect();
    if lanes.last().is_some_and(|&y| y + 1 < height - 1) {
        lanes.push(height - 2);
    }
    for i in (1..lanes.len()).rev() {
        lanes.swap(i, rng.below(i + 1));
    }

    let mut points: Vec<Vec<Point>> = vec![Vec::new(); k];
    for (t, &y) in lanes.iter().enumerate() {
        let (from, to) = match rng.below(2) {
            0 => (1, width - 2),
            _ => (width - 2, 1),
        };
        let tour = &mut points[t % k];
        tour.push(Point::new(from, y));
        if rng.below(3) == 0 {
            let x = 1 + rng.below((width - 2) as usize) as i32;
            tour.extend([
                Point::new(x, y),
                Point::new(x, y + 1 + rng.below(3) as i32),
                Point::new(x, y),
            ]);
        }
        tour.push(Point::new(to, y));
    }

    points.iter().map(|p| Route::through(p).unwrap()).collect()
}

fn objective(routes: &[Route]) -> u64 {
    routes.iter().map(Route::length).max().unwrap()
}

#[test]
fn scrambled_tours_get_shorter_and_stay_covering() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let mut saved = 0;

    for case in 0..24 {
        let (width, height, k) = (12 + rng.below(20) as i32, 9 + rng.below(20) as i32, 1 + case % 3);
        let problem = holed(width, height, k);
        let routes = scrambled(width, height, k, &mut rng);
        assert!(
            is_covered(&problem, &routes).unwrap(),
            "case {case} starts uncovered"
        );

        let deadline = Instant::now() + Duration::from_secs(10);
        let (after, stats) = turns_routes(&problem, &routes, deadline, TurnsOptions::default()).unwrap();

        saved += objective(&routes) - objective(&after);
        assert_eq!(after.len(), routes.len());
        assert!(
            is_covered(&problem, &after).unwrap(),
            "case {case}: {}",
            stats.per_move()
        );
        for new in &after {
            assert!(new.length() <= objective(&routes), "case {case}");
        }
        if k == 1 {
            assert_eq!(
                objective(&routes) - objective(&after),
                stats.saved_steps(),
                "case {case}"
            );
        }
    }

    assert!(saved > 0);
}

#[test]
fn a_spent_deadline_only_normalizes() {
    let problem = holed(20, 20, 1);
    let routes = scrambled(20, 20, 1, &mut Rng(7));

    let (after, stats) = turns_routes(&problem, &routes, Instant::now(), TurnsOptions::default()).unwrap();

    assert_eq!(stats.saved_steps(), 0);
    assert_eq!(objective(&after), objective(&routes));
}
