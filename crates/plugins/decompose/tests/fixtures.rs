//! Segments equal Python's exactly on the bench instances.
//!
//! The fixtures come from `tools/golden/decompose/export_fixtures.py`.

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::time::Instant;

use common::{fixtures, load_problem, numbers, uids_in};
use mow_core::{Axis, Decomposer, Segment};
use mow_decompose::{LaneSegments, Lanes};

type Expected = BTreeMap<String, Vec<(i32, i32, i32)>>;

/// One block per case: `case <label> <count>`, then `lane from to` lines.
fn load_expected(uid: &str) -> Expected {
    let text = fs::read_to_string(fixtures().join("segments").join(format!("{uid}.txt"))).unwrap();
    let mut expected = Expected::new();
    let mut current = String::new();

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("case ") {
            let (label, count) = rest.rsplit_once(' ').unwrap();
            current = label.to_string();
            expected.insert(current.clone(), Vec::with_capacity(count.parse().unwrap()));
        } else {
            let n = numbers(line);
            expected.get_mut(&current).unwrap().push((n[0], n[1], n[2]));
        }
    }

    expected
}

fn bench_uids() -> Vec<String> {
    uids_in("segments")
}

/// The decomposers the Python script ran, both axes.
fn decomposers() -> Vec<Box<dyn Decomposer>> {
    let mut all: Vec<Box<dyn Decomposer>> = Vec::new();

    for axis in [Axis::Horizontal, Axis::Vertical] {
        let base = LaneSegments::new(axis);

        all.push(Box::new(Lanes::new(axis)));
        all.push(Box::new(base));
        all.push(Box::new(LaneSegments {
            gap_factor: 0.0,
            ..base
        }));
        all.push(Box::new(LaneSegments {
            gap_factor: 4.0,
            ..base
        }));
        all.push(Box::new(LaneSegments { offset: 1, ..base }));
        all.push(Box::new(LaneSegments { shift: 0.5, ..base }));
    }

    all
}

fn as_numbers(segments: &[Segment]) -> Vec<(i32, i32, i32)> {
    segments.iter().map(|s| (s.lane(), s.from(), s.to())).collect()
}

#[test]
fn segments_match_python_on_every_bench_instance() {
    let uids = bench_uids();
    assert_eq!(uids.len(), 31, "run the export script to make the fixtures");

    let (mut matched, mut total) = (0, 0);
    let mut mismatches = Vec::new();

    for uid in &uids {
        let problem = load_problem(uid);
        let expected = load_expected(uid);

        for decomposer in decomposers() {
            let label = decomposer.label();
            let want = expected
                .get(&label)
                .unwrap_or_else(|| panic!("{uid}: no case {label}"));
            let got = as_numbers(&decomposer.decompose(&problem).unwrap());

            total += 1;
            if &got == want {
                matched += 1;
            } else {
                mismatches.push(format!("{uid} {label}: {} vs {} segments", got.len(), want.len()));
            }
        }
    }

    eprintln!("mow-decompose fixtures: {matched} of {total} cases match exactly");
    assert!(mismatches.is_empty(), "mismatches:\n{}", mismatches.join("\n"));
}

/// `cargo test --release -p mow-decompose -- --ignored --nocapture`
#[test]
#[ignore]
fn time_the_largest_bench_instance() {
    let problem = load_problem("isoa22945_987k_3x10");
    let rounds = 20;

    let started = Instant::now();
    let mut count = 0;
    for _ in 0..rounds {
        count += LaneSegments::new(Axis::Horizontal)
            .decompose(&problem)
            .unwrap()
            .len();
        count += LaneSegments::new(Axis::Vertical)
            .decompose(&problem)
            .unwrap()
            .len();
    }
    let each = started.elapsed() / rounds;

    eprintln!(
        "rust lane segments both axes on {}: {each:?} ({count} segments)",
        problem.uid()
    );
}
