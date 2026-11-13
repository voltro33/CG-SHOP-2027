//! Repair and corner cutting equal Python's on the 31 bench instances.
//!
//! The fixtures come from `tools/golden/improve/export_fixtures.py`: the
//! walks and tours the default pipeline builds, before and after each step
//! (`pipeline/`), and from `export_damaged.py`: the same walks and tours with
//! corners dropped, so repair has many cells to fix (`damaged/`).
//! The instance files are not checked in; without them the test says so and
//! passes.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::slice;
use std::time::{Duration, Instant};

use mow_core::{Budget, Improver, Point, Problem, Repairer, Route, Walk};
use mow_coverage::{is_covered, uncovered_walks};
use mow_improve::{CornerCut, Detours};

/// The blocks that hold several corner lists.
const GROUPS: [&str; 5] = ["tours", "closed", "groups", "grepaired", "gpruned"];

type Fixture = BTreeMap<String, Vec<Vec<Point>>>;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn problem(uid: &str) -> Option<Problem> {
    let path = root().join(format!(
        "data/instances/challenge_v0/instances/{uid}.instance.json"
    ));
    mow_io::load_problem(&path).ok()
}

fn corners(line: &str) -> Vec<Point> {
    let numbers: Vec<i32> = line.split_whitespace().map(|n| n.parse().unwrap()).collect();
    numbers.chunks(2).map(|p| Point::new(p[0], p[1])).collect()
}

/// `name count` then a corner line, or for a group `name count` then
/// `count` such pairs.
fn load(set: &str, uid: &str) -> Fixture {
    let text = fs::read_to_string(fixtures().join(set).join(format!("{uid}.txt"))).unwrap();
    let mut lines = text.lines();
    let mut fixture = Fixture::new();

    while let Some(header) = lines.next() {
        let (name, count) = header.split_once(' ').unwrap();
        let count: usize = count.parse().unwrap();

        let lists = if GROUPS.contains(&name) {
            (0..count)
                .map(|_| {
                    lines.next().unwrap();
                    corners(lines.next().unwrap())
                })
                .collect()
        } else {
            vec![corners(lines.next().unwrap())]
        };
        fixture.insert(name.to_string(), lists);
    }

    fixture
}

fn uids(set: &str) -> Vec<String> {
    let mut uids: Vec<String> = fs::read_dir(fixtures().join(set))
        .unwrap()
        .map(|e| {
            e.unwrap()
                .path()
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    uids.sort();

    uids
}

fn unlimited() -> Budget {
    Budget::new(Duration::from_secs(1_000_000), 0)
}

fn routes(lists: &[Vec<Point>]) -> Vec<Route> {
    lists.iter().map(|c| Route::new(c.clone()).unwrap()).collect()
}

fn route_corners(routes: &[Route]) -> Vec<Vec<Point>> {
    routes.iter().map(|r| r.corners().to_vec()).collect()
}

fn walk_covers(problem: &Problem, walk: &Walk) -> bool {
    uncovered_walks(problem, slice::from_ref(walk))
        .unwrap()
        .is_empty()
}

/// Seconds spent per step, summed over the instances.
#[derive(Default)]
struct Times(BTreeMap<&'static str, Duration>);

impl Times {
    fn run<T>(&mut self, step: &'static str, f: impl FnOnce() -> T) -> T {
        let started = Instant::now();
        let result = f();
        *self.0.entry(step).or_default() += started.elapsed();
        result
    }
}

/// Repairs the walk and the group tours; returns the steps that differ from
/// Python or leave cells unswept.
fn check_repair(problem: &Problem, fixture: &Fixture, times: &mut Times) -> Vec<&'static str> {
    let one = |name: &str| fixture[name][0].clone();
    let mut wrong = Vec::new();

    let walk = Walk::new(one("walk")).unwrap();
    let repaired = times.run("repair_walk", || Detours.repair_walk(problem, walk).unwrap());
    if repaired.corners() != one("repaired") || !walk_covers(problem, &repaired) {
        wrong.push("repair_walk");
    }

    let grepaired = times.run("repair_tours", || {
        Detours.repair(problem, routes(&fixture["groups"])).unwrap()
    });
    if route_corners(&grepaired) != fixture["grepaired"] || !is_covered(problem, &grepaired).unwrap() {
        wrong.push("repair_tours");
    }

    wrong
}

/// Repairs as [`check_repair`], then prunes the repaired walk, the closed
/// pieces of the pruned walk and the repaired group tours.
fn check_pipeline(problem: &Problem, fixture: &Fixture, times: &mut Times) -> Vec<&'static str> {
    let one = |name: &str| fixture[name][0].clone();
    let mut wrong = check_repair(problem, fixture, times);

    let input = Walk::new(one("repaired")).unwrap();
    let pruned = times.run("prune_open", || {
        CornerCut.improve_walk(problem, input, unlimited()).unwrap()
    });
    if pruned.corners() != one("pruned") || !walk_covers(problem, &pruned) {
        wrong.push("prune_open");
    }

    for (step, input, expected) in [
        ("prune_closed", "tours", "closed"),
        ("prune_groups", "grepaired", "gpruned"),
    ] {
        let pruned = times.run(step, || {
            CornerCut
                .improve(problem, routes(&fixture[input]), unlimited())
                .unwrap()
        });
        if route_corners(&pruned) != fixture[expected] || !is_covered(problem, &pruned).unwrap() {
            wrong.push(step);
        }
    }

    wrong
}

type Check = fn(&Problem, &Fixture, &mut Times) -> Vec<&'static str>;

/// Runs `check` on every instance of the fixture set `set`.
fn run_set(set: &str, steps: usize, check: Check) {
    let uids = uids(set);
    assert_eq!(uids.len(), 31, "run the export scripts to make the fixtures");

    let mut times = Times::default();
    let (mut matched, mut total) = (0, 0);
    let mut mismatches = Vec::new();

    for uid in &uids {
        let Some(problem) = problem(uid) else {
            eprintln!("skipped: no instance file for {uid}");
            return;
        };

        let wrong = check(&problem, &load(set, uid), &mut times);
        total += steps;
        matched += steps - wrong.len();
        if !wrong.is_empty() {
            mismatches.push(format!("{uid}: {}", wrong.join(", ")));
        }
    }

    eprintln!("mow-improve {set} fixtures: {matched} of {total} steps match exactly");
    for (step, spent) in &times.0 {
        eprintln!("  {step}: {spent:?} over all instances");
    }
    assert!(
        mismatches.is_empty(),
        "mismatches:
{}",
        mismatches.join(
            "
"
        )
    );
}

#[test]
fn repair_and_prune_match_python_on_the_bench_instances() {
    run_set("pipeline", 5, check_pipeline);
}

#[test]
fn repair_matches_python_on_damaged_walks_and_tours() {
    run_set("damaged", 2, check_repair);
}

/// `cargo test --release -p mow-improve --test fixtures -- --ignored --nocapture`
#[test]
#[ignore]
fn time_the_largest_bench_instance() {
    let uid = "isoa22945_987k_3x10";
    let Some(problem) = problem(uid) else {
        eprintln!("skipped: no instance file for {uid}");
        return;
    };

    for (set, check) in [("pipeline", check_pipeline as Check), ("damaged", check_repair)] {
        let mut times = Times::default();
        let wrong = check(&problem, &load(set, uid), &mut times);

        for (step, spent) in &times.0 {
            eprintln!("rust {set} {step} on {uid}: {spent:?}");
        }
        assert!(wrong.is_empty(), "{set}: {wrong:?}");
    }
}
