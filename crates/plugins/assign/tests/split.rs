//! `SplitWalk` cuts Python's repaired default walks exactly where
//! `split_walk` does, and stays fast on walks of millions of positions.

mod support;

use std::time::Instant;

use mow_assign::{split_bounds, SplitWalk};
use mow_core::{Point, Splitter, Walk};
use support::{bench_uids, field, pairs, point, read};

struct Case {
    walk: Walk,
    positions: u64,
    splits: Vec<(usize, Vec<(u64, u64)>)>,
    python_seconds: f64,
}

fn load(uid: &str) -> Case {
    let text = read("walks", uid);
    let mut lines = text.lines();

    let positions = field(lines.next().unwrap(), "positions").parse().unwrap();
    let corner_count: usize = field(lines.next().unwrap(), "corners").parse().unwrap();
    let corners: Vec<Point> = lines.by_ref().take(corner_count).map(point).collect();

    let mut splits = Vec::new();
    let mut python_seconds = 0.0;
    for line in lines {
        if let Some(rest) = line.strip_prefix("split ") {
            let (k, bounds) = rest.split_once(' ').unwrap();
            splits.push((k.parse().unwrap(), pairs(bounds)));
        } else if line.starts_with("python_seconds") {
            python_seconds = field(line, "python_seconds").parse().unwrap();
        }
    }

    Case {
        walk: Walk::new(corners).unwrap(),
        positions,
        splits,
        python_seconds,
    }
}

#[test]
fn split_bounds_equal_python_on_every_bench_walk() {
    let mut cases = 0;
    let mut rust_seconds = 0.0;
    let mut python_seconds = 0.0;

    for uid in bench_uids() {
        let case = load(&uid);
        assert_eq!(case.walk.length() + 1, case.positions, "{uid}");

        let started = Instant::now();
        for (k, expected) in &case.splits {
            assert_eq!(&split_bounds(&case.walk, *k), expected, "{uid} k = {k}");
            cases += 1;
        }
        rust_seconds += started.elapsed().as_secs_f64();
        python_seconds += case.python_seconds;
    }

    assert_eq!(cases, 62);
    println!("split k = 2, 3 on 31 walks: rust {rust_seconds:.4} s, python {python_seconds:.4} s");
}

#[test]
fn pieces_rejoin_into_the_walk() {
    for uid in bench_uids() {
        let walk = load(&uid).walk;
        let all: Vec<Point> = walk.positions().collect();

        for k in [2, 3] {
            let pieces = SplitWalk.split(&walk, k).unwrap();
            let joined: Vec<Point> = pieces
                .iter()
                .flat_map(|p| p.positions().collect::<Vec<_>>())
                .collect();

            assert!(pieces.len() <= k);
            assert_eq!(joined, all, "{uid} k = {k}");
        }
    }
}

/// A boustrophedon over `lanes` lanes of `width` steps.
fn zigzag(lanes: i32, width: i32) -> Walk {
    let mut corners = Vec::with_capacity(2 * lanes as usize);
    for y in 0..lanes {
        let (a, b) = if y % 2 == 0 { (0, width) } else { (width, 0) };
        corners.push(Point::new(a, 2 * y));
        corners.push(Point::new(b, 2 * y));
    }

    Walk::new(corners).unwrap()
}

#[test]
fn millions_of_positions_split_quickly() {
    let walk = zigzag(2_000, 2_000);
    assert!(walk.length() > 4_000_000);

    let started = Instant::now();
    let pieces = SplitWalk.split(&walk, 3).unwrap();
    let seconds = started.elapsed().as_secs_f64();

    let total: u64 = pieces.iter().map(|p| p.length() + 1).sum();
    assert_eq!(total, walk.length() + 1);
    println!(
        "split a {}-position walk into 3: {seconds:.4} s",
        walk.length() + 1
    );
    assert!(seconds < 2.0);
}
