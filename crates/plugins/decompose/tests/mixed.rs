//! Mixed lane segments equal Python's `mixed_segment_ends` exactly on the
//! bench instances: axis, lane and both ends of every segment, in order.
//!
//! The fixtures come from `tools/golden/decompose/export_mixed.py`.

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::time::Instant;

use common::{fixtures, load_problem, numbers, uids_in};
use mow_core::{Axis, Decomposer, Segment};
use mow_decompose::MixedLanes;

/// `(axis, lane, from, to)`, the axis as `h` or `v`.
type Row = (char, i32, i32, i32);
type Expected = BTreeMap<String, Vec<Row>>;

/// One block per case: `case <label> <count>`, then `<h|v> lane from to` lines.
fn load_expected(uid: &str) -> Expected {
    let text = fs::read_to_string(fixtures().join("mixed").join(format!("{uid}.txt"))).unwrap();
    let mut expected = Expected::new();
    let mut current = String::new();

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("case ") {
            let (label, count) = rest.rsplit_once(' ').unwrap();
            current = label.to_string();
            expected.insert(current.clone(), Vec::with_capacity(count.parse().unwrap()));
        } else {
            let (axis, rest) = line.split_once(' ').unwrap();
            let n = numbers(rest);
            let row = (axis.chars().next().unwrap(), n[0], n[1], n[2]);
            expected.get_mut(&current).unwrap().push(row);
        }
    }

    expected
}

/// The knob sets the Python script ran.
fn decomposers() -> Vec<MixedLanes> {
    let base = MixedLanes::new();

    vec![
        base,
        MixedLanes {
            tile_lanes: 2,
            segment_cost: 4.0,
            ..base
        },
        MixedLanes {
            tile_lanes: 16,
            segment_cost: 4.0,
            ..base
        },
        MixedLanes {
            gap_factor: 2.0,
            ..base
        },
        MixedLanes { shift: 0.5, ..base },
    ]
}

fn as_rows(segments: &[Segment]) -> Vec<Row> {
    let axis = |s: &Segment| match s.axis() {
        Axis::Horizontal => 'h',
        Axis::Vertical => 'v',
    };

    segments
        .iter()
        .map(|s| (axis(s), s.lane(), s.from(), s.to()))
        .collect()
}

#[test]
fn mixed_segments_match_python_on_every_bench_instance() {
    let uids = uids_in("mixed");
    assert_eq!(uids.len(), 31, "run export_mixed.py to make the fixtures");

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
            let got = as_rows(&decomposer.decompose(&problem).unwrap());

            total += 1;
            if &got == want {
                matched += 1;
            } else {
                mismatches.push(format!("{uid} {label}: {} vs {} segments", got.len(), want.len()));
            }
        }
    }

    eprintln!("mow-decompose mixed fixtures: {matched} of {total} cases match exactly");
    assert!(mismatches.is_empty(), "mismatches:\n{}", mismatches.join("\n"));
}

/// `cargo test --release -p mow-decompose --test mixed -- --ignored --nocapture`
#[test]
#[ignore]
fn time_mixed_on_the_largest_bench_instance() {
    let problem = load_problem("isoa22945_987k_3x10");
    let rounds = 20;

    let started = Instant::now();
    let mut count = 0;
    for _ in 0..rounds {
        count += MixedLanes::new().decompose(&problem).unwrap().len();
    }
    let each = started.elapsed() / rounds;

    eprintln!(
        "rust mixed lanes (defaults) on {}: {each:?} ({} segments)",
        problem.uid(),
        count / rounds as usize
    );
}
