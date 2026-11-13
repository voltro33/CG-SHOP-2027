//! Fixture comparison with the Python solver. The fixtures are written by
//! `tools/golden/order/export_fixtures.py`.
//!
//! The timed comparison is ignored by default, run it in release:
//!
//! ```text
//! cargo test -p mow-order --release --test fixtures -- --ignored --nocapture
//! ```

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use mow_core::{Budget, Closure, Orderer, Segment};
use mow_order::{nearest_neighbour, IteratedLocalSearch};

struct Fixture {
    name: String,
    closure: Closure,
    python_nn: u64,
    python_ils: u64,
    python_seconds: f64,
    python_checks: u64,
    python_moves: u64,
    kick_seconds: f64,
    segments: Vec<Segment>,
}

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn fixtures() -> Vec<Fixture> {
    let mut paths: Vec<PathBuf> = fs::read_dir(fixture_dir())
        .expect("fixtures exist, see tools/golden/order")
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "txt"))
        .collect();
    paths.sort();

    paths.iter().map(|path| load(path)).collect()
}

fn load(path: &Path) -> Fixture {
    let text = fs::read_to_string(path).unwrap();
    let mut lines = text.lines();
    let mut header = || -> Vec<String> {
        lines
            .next()
            .unwrap()
            .split_whitespace()
            .map(str::to_owned)
            .collect()
    };

    let closure = match header()[1].as_str() {
        "closed" => Closure::Closed,
        _ => Closure::Open,
    };
    let python_nn = header()[1].parse().unwrap();

    // python_ils <len> seed <s> kick_seconds <k> seconds <t> checks <c> moves <m>
    let ils = header();
    let count: usize = header()[1].parse().unwrap();

    let segments: Vec<Segment> = lines
        .map(|line| {
            let v: Vec<i32> = line.split_whitespace().map(|x| x.parse().unwrap()).collect();
            Segment::horizontal(v[0], v[1], v[2]).unwrap()
        })
        .collect();
    assert_eq!(segments.len(), count);

    Fixture {
        name: path.file_stem().unwrap().to_string_lossy().into_owned(),
        closure,
        python_nn,
        python_ils: ils[1].parse().unwrap(),
        kick_seconds: ils[5].parse().unwrap(),
        python_seconds: ils[7].parse().unwrap(),
        python_checks: ils[9].parse().unwrap(),
        python_moves: ils[11].parse().unwrap(),
        segments,
    }
}

#[test]
fn nearest_neighbour_matches_python_exactly() {
    for fixture in fixtures() {
        let order = nearest_neighbour(&fixture.segments);

        assert_eq!(
            order.link_length(&fixture.segments, fixture.closure),
            fixture.python_nn,
            "{}",
            fixture.name
        );
    }
}

#[test]
#[ignore = "timed, run in release with --ignored"]
fn iterated_local_search_is_no_worse_than_python() {
    println!("fixture                              segs  python      rust   rust/py  checks/s py -> rust     moves/s py -> rust   secs");

    let mut worse = Vec::new();
    for fixture in fixtures() {
        let search = IteratedLocalSearch {
            kick_seconds: fixture.kick_seconds,
            ..IteratedLocalSearch::default()
        };
        let budget = Budget::new(Duration::from_secs(600), 0);

        let outcome = search.search(&fixture.segments, fixture.closure, budget);
        let length = outcome.order.link_length(&fixture.segments, fixture.closure);
        let seconds = outcome.stats.elapsed.as_secs_f64();

        println!(
            "{:<34} {:>6} {:>7} {:>9} {:>8.3} {:>9.0} -> {:<10.0} {:>8.0} -> {:<10.0} {:>5.1}",
            fixture.name,
            fixture.segments.len(),
            fixture.python_ils,
            length,
            length as f64 / fixture.python_ils as f64,
            fixture.python_checks as f64 / fixture.python_seconds,
            outcome.stats.checks as f64 / seconds,
            fixture.python_moves as f64 / fixture.python_seconds,
            outcome.stats.moves as f64 / seconds,
            seconds,
        );

        if length > fixture.python_ils {
            worse.push(fixture.name.clone());
        }
        assert_eq!(search.label(), "ils/kick=3/anneal=0");
    }

    assert!(worse.is_empty(), "worse than Python on {worse:?}");
}

/// Lanes with gaps, like a big lawn with many holes: `count` segments.
fn holey_lanes(count: i32) -> Vec<Segment> {
    let per_lane = 40;
    (0..count)
        .map(|i| {
            let (lane, k) = (i / per_lane, i % per_lane);
            let a = k * 50 + (i * 7919) % 17;
            Segment::horizontal(3 * lane, a, a + 20 + (i * 104_729) % 13).unwrap()
        })
        .collect()
}

#[test]
#[ignore = "timed, run in release with --ignored"]
fn a_hundred_thousand_segments_set_up_in_seconds() {
    let segments = holey_lanes(100_000);
    let search = IteratedLocalSearch {
        kick_seconds: 1.0,
        ..IteratedLocalSearch::default()
    };

    let outcome = search.search(
        &segments,
        Closure::Closed,
        Budget::new(Duration::from_secs(60), 0),
    );
    let length = outcome.order.link_length(&segments, Closure::Closed);
    println!(
        "100k segments: nn {} -> {} in {:.1} s, {} checks, {} kicks",
        outcome.stats.start_length,
        length,
        outcome.stats.elapsed.as_secs_f64(),
        outcome.stats.checks,
        outcome.stats.kicks
    );

    assert!(length < outcome.stats.start_length);
    assert!(outcome.stats.elapsed < Duration::from_secs(10));
}
