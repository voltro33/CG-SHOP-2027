//! `group_order` cuts Python's orders exactly where Python does, and
//! `Groups` end to end comes out about as long as `groups_horizontal` on the
//! same segments with the same kick time (the order search is randomized,
//! so only the totals are compared).

mod support;

use std::time::{Duration, Instant};

use mow_assign::{group_order, Groups};
use mow_core::{Assigner, BitGrid, Budget, Cutter, Point, Problem, Segment};
use mow_order::IteratedLocalSearch;
use support::{bench_uids, field, numbers, pairs, read};

struct Case {
    k: usize,
    python_longest: u64,
    python_seconds: f64,
    kick: f64,
    segments: Vec<Segment>,
    ends: Vec<(Point, Point)>,
    cuts: Vec<(usize, Vec<(usize, usize)>)>,
}

fn load(uid: &str) -> Case {
    let text = read("groups", uid);
    let mut lines = text.lines();

    let k = field(lines.next().unwrap(), "k").parse().unwrap();
    let python = lines.next().unwrap();

    let count: usize = field(lines.next().unwrap(), "segments").parse().unwrap();
    let segments = lines
        .by_ref()
        .take(count)
        .map(|line| {
            let n: Vec<i32> = numbers(line);
            Segment::horizontal(n[0], n[1], n[2]).unwrap()
        })
        .collect();

    let count: usize = field(lines.next().unwrap(), "order").parse().unwrap();
    let ends = lines
        .by_ref()
        .take(count)
        .map(|line| {
            let n: Vec<i32> = numbers(line);
            (Point::new(n[0], n[1]), Point::new(n[2], n[3]))
        })
        .collect();

    let cuts = lines
        .map(|line| {
            let rest = line.strip_prefix("group_order ").unwrap();
            let (k, groups) = rest.split_once(' ').unwrap();
            (k.parse().unwrap(), pairs(groups))
        })
        .collect();

    Case {
        k,
        python_longest: field(python, "python_groups").parse().unwrap(),
        python_seconds: field(python, "seconds").parse().unwrap(),
        kick: field(python, "kick").parse().unwrap(),
        segments,
        ends,
        cuts,
    }
}

#[test]
fn group_order_equals_python_on_every_bench_order() {
    let mut cases = 0;

    for uid in bench_uids() {
        let case = load(&uid);

        for (k, expected) in &case.cuts {
            assert_eq!(&group_order(&case.ends, *k), expected, "{uid} k = {k}");
            cases += 1;
        }
    }

    assert_eq!(cases, 62);
}

/// `Groups` only reads `k` from the problem.
fn problem_with(k: usize) -> Problem {
    let one = BitGrid::from_cells(&[Point::new(0, 0)]).unwrap().unwrap();
    Problem::new("groups", one.clone(), Cutter::new(one).unwrap(), k).unwrap()
}

#[test]
fn groups_are_about_as_long_as_python() {
    let orderer = IteratedLocalSearch {
        kick_seconds: 60.0,
        ..IteratedLocalSearch::default()
    };

    let mut ratios = Vec::new();
    let (mut rust_seconds, mut python_seconds) = (0.0, 0.0);

    for uid in bench_uids() {
        let case = load(&uid);
        let groups = Groups {
            kick_seconds: Some(case.kick),
        };

        // Python's deadline: the time limit plus twice the kick time
        let budget = Budget::new(Duration::from_secs_f64(2.0 + 2.0 * case.kick), 0);
        let started = Instant::now();
        let routes = groups
            .assign(&problem_with(case.k), &case.segments, &orderer, budget)
            .unwrap();
        let seconds = started.elapsed().as_secs_f64();

        let longest = routes.iter().map(|r| r.length()).max().unwrap();
        let ratio = longest as f64 / case.python_longest as f64;
        println!(
            "{uid}: k {} rust {longest} python {} ratio {ratio:.3}, {seconds:.2} s vs {:.2} s",
            case.k, case.python_longest, case.python_seconds
        );

        assert!(routes.len() <= case.k);
        assert!(ratio < 1.10, "{uid}: {longest} vs {}", case.python_longest);

        ratios.push(ratio);
        rust_seconds += seconds;
        python_seconds += case.python_seconds;
    }

    let mean = ratios.iter().sum::<f64>() / ratios.len() as f64;
    // randomized search: no single instance must win, but on the whole Rust
    // must keep up with Python, even in a debug build
    assert!(mean < 1.02, "mean ratio {mean}");
    let better = ratios.iter().filter(|&&r| r < 1.0).count();
    let equal = ratios.iter().filter(|&&r| r == 1.0).count();
    println!(
        "groups: mean ratio {mean:.4}, shorter on {better}, equal on {equal} of {}; rust {rust_seconds:.1} s, python {python_seconds:.1} s",
        ratios.len()
    );
}
