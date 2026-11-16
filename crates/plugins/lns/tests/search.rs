//! Whole searches on small problems: every result covers, never gets
//! longer, and the counts never drift from the tours.

use std::time::{Duration, Instant};

use mow_core::{BitGrid, Bounds, Cutter, Point, Problem, Route};
use mow_coverage::is_covered;
use mow_lns::{default_wheels, resume, search, LnsOptions};

fn square_cutter(side: i32) -> Cutter {
    let half = side / 2;
    let cells: Vec<Point> = (-half..side - half)
        .flat_map(|y| (-half..side - half).map(move |x| Point::new(x, y)))
        .collect();

    Cutter::new(BitGrid::from_cells(&cells).unwrap().unwrap()).unwrap()
}

/// A `width x height` rectangle with a square hole, cut by a 3 x 3 square.
fn holed(uid: &str, width: i32, height: i32, k: usize) -> Problem {
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
    for y in 8..13 {
        for x in 10..16 {
            region.remove(Point::new(x, y));
        }
    }

    Problem::new(uid, region, square_cutter(3), k).unwrap()
}

/// Vertical lanes three apart over the whole box, a snake, as one route:
/// covers, but crosses the hole and wastes the far turns.
fn snake(width: i32, height: i32) -> Route {
    let mut corners = Vec::new();
    let mut x = 1;
    let mut up = true;
    while x < width + 1 {
        let x_lane = x.min(width - 2);
        let (a, b) = if up { (1, height - 2) } else { (height - 2, 1) };
        corners.push(Point::new(x_lane, a));
        corners.push(Point::new(x_lane, b));
        x += 3;
        up = !up;
    }

    Route::through(&corners).unwrap()
}

fn options(seed: u64) -> LnsOptions {
    LnsOptions {
        seed,
        ..LnsOptions::default()
    }
}

#[test]
fn a_search_keeps_coverage_and_never_lengthens() {
    let problem = holed("holed_1k_1s3", 30, 21, 1);
    let start = vec![snake(30, 21)];
    assert!(is_covered(&problem, &start).unwrap());

    for seed in 0..4 {
        let deadline = Instant::now() + Duration::from_millis(400);
        let outcome = search(&problem, &start, deadline, &options(seed), default_wheels()).unwrap();

        assert!(is_covered(&problem, &outcome.best).unwrap(), "seed {seed}");
        assert!(is_covered(&problem, &outcome.current).unwrap(), "seed {seed}");
        assert!(outcome.stats.best_objective <= start[0].length());
        assert_eq!(outcome.best[0].length(), outcome.stats.best_objective);
        assert!(outcome.stats.moves > 0);
    }
}

#[test]
fn a_search_shortens_a_wasteful_tour() {
    let problem = holed("holed_1k_1s3", 30, 21, 1);
    let start = vec![snake(30, 21)];

    let deadline = Instant::now() + Duration::from_millis(1500);
    let outcome = search(&problem, &start, deadline, &options(7), default_wheels()).unwrap();

    assert!(outcome.stats.best_objective < start[0].length());
}

#[test]
fn two_tours_balance_through_transfers() {
    let problem = holed("holed_1k_2s3", 30, 21, 2);
    let idle = Route::new(vec![Point::new(28, 19)]).unwrap();
    let start = vec![snake(30, 21), idle];
    assert!(is_covered(&problem, &start).unwrap());

    let deadline = Instant::now() + Duration::from_millis(1500);
    let outcome = search(&problem, &start, deadline, &options(1), default_wheels()).unwrap();

    assert!(is_covered(&problem, &outcome.best).unwrap());
    assert!(outcome.stats.best_objective < start[0].length());

    let transfer = outcome
        .destroy
        .operators
        .iter()
        .find(|o| o.name == "transfer-3")
        .unwrap();
    assert!(transfer.tally.tries > 0);
}

#[test]
fn resume_saves_state_and_continues() {
    let dir = std::env::temp_dir().join(format!("mow-lns-resume-{}", std::process::id()));
    let problem = holed("holed_1k_1s3", 30, 21, 1);
    let start = vec![snake(30, 21)];
    let stored = start[0].length();

    let deadline = Instant::now() + Duration::from_millis(300);
    let first = resume(&problem, &start, stored, &dir, deadline, &options(2)).unwrap();
    assert!(dir.join("holed_1k_1s3.json").is_file());

    let deadline = Instant::now() + Duration::from_millis(300);
    let second = resume(
        &problem,
        &start,
        first.stats.best_objective,
        &dir,
        deadline,
        &options(3),
    )
    .unwrap();
    assert!(is_covered(&problem, &second.best).unwrap());

    let saved = mow_lns::Saved::load(&dir, "holed_1k_1s3").unwrap().unwrap();
    assert_eq!(saved.runs, 2);

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn resume_starts_from_the_best_once_the_walk_drifted_above_it() {
    let dir = std::env::temp_dir().join(format!("mow-lns-drift-{}", std::process::id()));
    let problem = holed("holed_1k_1s3", 30, 21, 1);
    let snake = vec![snake(30, 21)];

    let deadline = Instant::now() + Duration::from_millis(600);
    let first = resume(&problem, &snake, snake[0].length(), &dir, deadline, &options(2)).unwrap();
    let best = first.stats.best_objective;
    assert!(snake[0].length() as f64 > best as f64 * 1.01);

    // the saved walk set back to the snake, far above the best it saw
    let mut saved = mow_lns::Saved::load(&dir, "holed_1k_1s3").unwrap().unwrap();
    saved.stored_best = best;
    saved.current_objective = snake[0].length();
    saved.current = mow_lns::Saved::corners_of(&snake);
    saved.save(&dir).unwrap();

    let deadline = Instant::now() + Duration::from_millis(100);
    let second = resume(&problem, &first.best, best, &dir, deadline, &options(3)).unwrap();
    assert_eq!(second.stats.start_objective, best);

    std::fs::remove_dir_all(dir).unwrap();
}
