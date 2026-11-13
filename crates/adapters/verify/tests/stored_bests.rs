//! The gate: Python accepts every stored best with the objective Rust
//! computes, and rejects a best with one edge moved. Skips without data or
//! python. The full run takes minutes, so it is ignored by default:
//!
//!     cargo test -p mow-verify --release -- --ignored --nocapture

use std::path::{Path, PathBuf};
use std::time::Instant;

use mow_core::{Point, Route, Solution};
use mow_io::load_solution;
use mow_verify::{python_in, PythonVerifier};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// The verifier and every stored best, sorted; `None` (skipped) without them.
fn setup() -> Option<(PythonVerifier, Vec<PathBuf>)> {
    let root = root();
    let bests = root.join("data/solutions/best");
    let instances = root.join("data/instances/challenge_v0/instances");

    if !python_in(&root).is_file() || !bests.is_dir() || !instances.is_dir() {
        println!("skipped: no python, stored bests or instances");
        return None;
    }

    let mut paths: Vec<PathBuf> = std::fs::read_dir(&bests)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.to_string_lossy().ends_with(".solution.json"))
        .collect();
    paths.sort();

    let verifier = PythonVerifier::from_repo(&root, instances).unwrap();
    Some((verifier, paths))
}

/// Shifts the longest edge of the first tour sideways by `by`, so its
/// neighbours stay axis parallel but the lane it mowed moves away.
fn moved_edge(solution: &Solution, by: i32) -> Solution {
    let mut routes = solution.routes().to_vec();
    let corners = routes[0].corners().to_vec();
    let n = corners.len();

    let length = |i: usize| {
        let (a, b) = (corners[i], corners[(i + 1) % n]);
        (a.x - b.x).abs() + (a.y - b.y).abs()
    };
    let i = (0..n).max_by_key(|&i| length(i)).unwrap();
    let j = (i + 1) % n;

    let horizontal = corners[i].y == corners[j].y;
    let shift = |p: Point| match horizontal {
        true => Point::new(p.x, p.y + by),
        false => Point::new(p.x + by, p.y),
    };

    // corners may sit in the middle of a straight line; move the whole line
    let on_line = |p: Point| match horizontal {
        true => p.y == corners[i].y,
        false => p.x == corners[i].x,
    };
    let mut moved = corners.clone();
    for step in 0..n {
        let k = (i + step) % n;
        if !on_line(corners[k]) {
            break;
        }
        moved[k] = shift(corners[k]);
    }
    for step in 1..n {
        let k = (i + n - step) % n;
        if !on_line(corners[k]) {
            break;
        }
        moved[k] = shift(corners[k]);
    }
    routes[0] = Route::new(moved).unwrap();

    Solution::new(solution.uid(), routes)
}

#[test]
fn a_few_bests_pass_and_a_moved_edge_fails() {
    let Some((verifier, paths)) = setup() else { return };

    // the smallest files load fastest
    let mut paths = paths;
    paths.sort_by_key(|path| std::fs::metadata(path).unwrap().len());

    for path in paths.iter().take(3) {
        let best = load_solution(path).unwrap();

        let report = verifier.check(&best).unwrap();
        assert!(report.is_feasible(), "{}: {:?}", path.display(), report.errors);
        assert_eq!(report.objective, Some(best.objective()), "{}", path.display());

        let broken = verifier.check(&moved_edge(&best, 40)).unwrap();
        assert!(!broken.is_feasible(), "{}", path.display());
        println!("{}: {}", best.uid(), broken.errors[0]);
    }
}

#[test]
#[ignore = "verifies all 250 stored bests, minutes of Python"]
fn every_stored_best_passes() {
    let Some((verifier, paths)) = setup() else { return };
    let bests: Vec<Solution> = paths.iter().map(|p| load_solution(p).unwrap()).collect();
    let n = bests.len();

    // each best is a new instance, so every call here loads one
    let started = Instant::now();
    let mut after_first = None;
    for best in &bests {
        let report = verifier.check(best).unwrap();
        assert!(report.is_feasible(), "{}: {:?}", best.uid(), report.errors);
        assert_eq!(report.objective, Some(best.objective()), "{}", best.uid());

        after_first.get_or_insert_with(Instant::now);
    }
    let all = started.elapsed().as_secs_f64();
    let rest = after_first.map_or(0.0, |t| t.elapsed().as_secs_f64());
    println!(
        "{n} bests feasible, objectives equal: {all:.1}s, {:.2}/s with the first call, {:.2}/s without",
        n as f64 / all,
        (n - 1) as f64 / rest,
    );

    // a shift may land on ground another lane mows; one way must not
    let started = Instant::now();
    let mut checks = 0;
    let mut rejected = 0;
    for best in &bests {
        let mut broken = |by| {
            checks += 1;
            !verifier.check(&moved_edge(best, by)).unwrap().is_feasible()
        };
        match broken(40) || broken(-40) {
            true => rejected += 1,
            false => println!("{}: the moved edge is still covered", best.uid()),
        }
    }
    println!(
        "{rejected}/{n} moved edges rejected; {checks} checks of cached instances at {:.2}/s",
        checks as f64 / started.elapsed().as_secs_f64(),
    );
    assert_eq!(rejected, n);
}

#[test]
#[ignore = "throughput once instances are cached"]
fn cached_throughput() {
    let Some((verifier, paths)) = setup() else { return };
    let best = load_solution(&paths[0]).unwrap();

    let first = Instant::now();
    verifier.check(&best).unwrap();
    let first = first.elapsed().as_secs_f64();

    let rounds = 50;
    let started = Instant::now();
    for _ in 0..rounds {
        assert!(verifier.check(&best).unwrap().is_feasible());
    }
    let seconds = started.elapsed().as_secs_f64();

    println!(
        "{}: first call {first:.2}s, then {:.1} solutions/s cached",
        best.uid(),
        rounds as f64 / seconds
    );
}
