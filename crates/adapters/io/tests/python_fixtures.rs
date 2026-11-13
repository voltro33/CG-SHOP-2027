//! Every challenge instance against what Python's `Problem.from_instance`
//! makes of it (`tools/golden/io/export_problems.py`). All 250 take a few
//! seconds even in a debug build.

use std::path::{Path, PathBuf};

use mow_core::{BitGrid, Problem};
use serde::Deserialize;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0100_0000_01b3;

#[derive(Debug, Deserialize)]
struct Expected {
    uid: String,
    k: usize,
    num_cells: usize,
    region_frame: [i32; 4],
    region_runs: usize,
    region_hash: String,
    cutter_frame: [i32; 4],
    cutter_area: usize,
    cutter_runs: Vec<[i32; 3]>,
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn expected() -> Vec<Expected> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/problems.json");
    let json = std::fs::read_to_string(path).expect("fixture file");
    serde_json::from_str(&json).expect("fixture JSON")
}

fn instance_path(uid: &str) -> PathBuf {
    repo().join(format!(
        "data/instances/challenge_v0/instances/{uid}.instance.json"
    ))
}

fn frame(grid: &BitGrid) -> [i32; 4] {
    let f = grid.frame();
    [f.x_min, f.y_min, f.x_max, f.y_max]
}

/// The same FNV-1a over 64 bit words as the Python script.
fn run_hash(grid: &BitGrid) -> (usize, String) {
    let mut h = FNV_OFFSET;
    let mut count = 0;

    for run in grid.runs() {
        for value in [run.y, run.x_min, run.x_max] {
            h = (h ^ i64::from(value) as u64).wrapping_mul(FNV_PRIME);
        }
        count += 1;
    }

    (count, format!("{h:016x}"))
}

/// Empty if the problem matches, else what differs.
fn differences(problem: &Problem, e: &Expected) -> Vec<String> {
    let cutter_runs: Vec<[i32; 3]> = problem
        .cutter()
        .cells()
        .runs()
        .map(|r| [r.y, r.x_min, r.x_max])
        .collect();

    let checks = [
        ("uid", problem.uid() == e.uid),
        ("k", problem.k() == e.k),
        ("num_cells", problem.num_cells() == e.num_cells),
        ("region_frame", frame(problem.region()) == e.region_frame),
        (
            "region_runs+hash",
            run_hash(problem.region()) == (e.region_runs, e.region_hash.clone()),
        ),
        ("cutter_frame", frame(problem.cutter().cells()) == e.cutter_frame),
        ("cutter_area", problem.cutter().area() == e.cutter_area),
        ("cutter_runs", cutter_runs == e.cutter_runs),
    ];

    checks
        .iter()
        .filter(|(_, ok)| !ok)
        .map(|(name, _)| format!("{}: {name}", e.uid))
        .collect()
}

#[test]
fn every_instance_matches_python() {
    let all = expected();
    assert_eq!(all.len(), 250, "one fixture entry per challenge instance");

    if !instance_path(&all[0].uid).is_file() {
        println!("skipped: the challenge instances are not checked out");
        return;
    }

    let mut failures = Vec::new();
    for e in &all {
        match mow_io::load_problem(&instance_path(&e.uid)) {
            Ok(problem) => failures.extend(differences(&problem, e)),
            Err(err) => failures.push(format!("{}: {err}", e.uid)),
        }
    }

    println!("{} of {} instances match", all.len() - failures.len(), all.len());
    assert!(failures.is_empty(), "{failures:#?}");
}
