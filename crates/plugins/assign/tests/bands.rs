//! `Bands` cuts the same cells as `band_problems`, and `rebalanced` gives
//! the same shares as `_rebalanced`. Bands need the gitignored `data/`
//! folder and print "skipped" without it.

mod support;

use std::time::Instant;

use mow_assign::{rebalanced, Bands};
use mow_core::{Axis, Partitioner, Problem};
use support::{bench_uids, fixtures, numbers, read, repo};

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0100_0000_01b3;

/// FNV-1a 64 over the text `y a b\n` of every run, as the script hashes.
fn run_hash(problem: &Problem) -> u64 {
    let mut hash = FNV_OFFSET;
    for run in problem.region().runs() {
        for byte in format!("{} {} {}\n", run.y, run.x_min, run.x_max).bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }

    hash
}

/// `x_min y_min x_max y_max cells hash`, as the fixture lists a band.
fn describe(band: &Problem) -> String {
    let b = band.region_bounds();
    format!(
        "{} {} {} {} {} {}",
        b.x_min,
        b.y_min,
        b.x_max,
        b.y_max,
        band.num_cells(),
        run_hash(band)
    )
}

fn check_instance(uid: &str, problem: &Problem) -> (usize, f64) {
    let text = read("bands", uid);
    let mut lines = text.lines().peekable();
    let mut cases = 0;
    let mut seconds = 0.0;

    while let Some(header) = lines.next() {
        let words: Vec<&str> = header.split_whitespace().collect();
        let axis = if words[1] == "y" {
            Axis::Horizontal
        } else {
            Axis::Vertical
        };
        let count: usize = words[2].parse().unwrap();
        let shares: Vec<f64> = numbers(&words[3..].join(" "));

        let started = Instant::now();
        let bands = Bands { axis }.bands(problem, &shares).unwrap();
        seconds += started.elapsed().as_secs_f64();

        let expected: Vec<&str> = lines.by_ref().take(count).collect();
        let actual: Vec<String> = bands.iter().map(describe).collect();
        assert_eq!(actual, expected, "{uid} {header}");
        assert!(bands.iter().all(|b| b.k() == 1 && b.uid() == uid));

        cases += 1;
    }

    (cases, seconds)
}

#[test]
fn bands_equal_python_on_every_bench_instance() {
    let folder = repo().join("data/instances/challenge_v0/instances");
    if !folder.exists() {
        println!("skipped: no data/instances");
        return;
    }

    let mut cases = 0;
    let mut seconds = 0.0;
    for uid in bench_uids() {
        let problem = mow_io::load_problem(&folder.join(format!("{uid}.instance.json"))).unwrap();
        let (done, took) = check_instance(&uid, &problem);
        cases += done;
        seconds += took;
    }

    assert_eq!(cases, 31 * 10);
    println!("bands, 310 cases: rust {seconds:.4} s");
}

#[test]
fn rebalanced_equals_python() {
    let text = std::fs::read_to_string(fixtures().join("rebalanced.txt")).unwrap();
    let lines: Vec<&str> = text.lines().collect();

    for case in lines.chunks(3) {
        let shares: Vec<f64> = numbers(case[0]);
        let lengths: Vec<u64> = numbers(case[1]);
        let expected: Vec<f64> = numbers(case[2]);

        let actual = rebalanced(&shares, &lengths);
        assert_eq!(actual.len(), expected.len());
        for (a, e) in actual.iter().zip(&expected) {
            assert!((a - e).abs() <= 1e-12, "{a} vs {e}");
        }
    }

    assert_eq!(lines.len(), 150);
}
