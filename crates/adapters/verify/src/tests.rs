//! Unit tests on a 4 x 4 square. The ones that need python skip without it.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use mow_core::{Point, PortError, Route, Solution, Verifier};
use mow_io::SolutionFile;

use crate::{python_in, PythonVerifier, SCRIPT};

const SQUARE: &str = r#"{"instance_uid":"sq",
    "region_to_cover":{"outer_boundary":{"x":[0,4,4,0],"y":[0,0,4,4]},"inner_boundaries":[]},
    "cutter":{"x":[0,1,1,0],"y":[0,0,1,1]},"cutter_center":[0,0],"number_of_cutters":1}"#;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// A fresh directory holding `sq.instance.json`.
fn instances() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);

    let dir = std::env::temp_dir().join(format!("mow-verify-{}-{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("sq.instance.json"), SQUARE).unwrap();

    dir
}

fn verifier() -> Option<PythonVerifier> {
    let root = root();
    if !python_in(&root).is_file() {
        println!("skipped: no python in {}", root.display());
        return None;
    }

    Some(PythonVerifier::from_repo(&root, instances()).unwrap())
}

fn solution(uid: &str, corners: &[(i32, i32)]) -> Solution {
    let corners = corners.iter().map(|&(x, y)| Point::new(x, y)).collect();
    Solution::new(uid, vec![Route::new(corners).unwrap()])
}

/// Every row of the square, lane by lane.
fn snake() -> Solution {
    solution(
        "sq",
        &[(0, 0), (3, 0), (3, 1), (0, 1), (0, 2), (3, 2), (3, 3), (0, 3)],
    )
}

#[test]
fn missing_script_and_directory_are_not_found() {
    let python = python_in(&root());

    let script = PythonVerifier::new(&python, "no/such/verify.py", instances());
    assert!(matches!(script, Err(PortError::NotFound(m)) if m.contains("script")));

    let dir = PythonVerifier::new(&python, root().join(SCRIPT), "no/such/dir");
    assert!(matches!(dir, Err(PortError::NotFound(m)) if m.contains("instance directory")));
}

#[test]
fn missing_python_is_an_error() {
    let result = PythonVerifier::new("no/such/python.exe", root().join(SCRIPT), instances());

    assert!(matches!(result, Err(PortError::External { message, .. }) if message.contains("python")));
}

#[test]
fn feasible_and_infeasible() {
    let Some(verifier) = verifier() else { return };

    let good = verifier.check(&snake()).unwrap();
    assert!(good.is_feasible(), "{:?}", good.errors);
    assert_eq!(good.objective, Some(snake().objective()));
    assert_eq!(good.objective, Some(18));

    let ring = solution("sq", &[(0, 0), (3, 0), (3, 3), (0, 3)]);
    let bad = verifier.verify(&ring).unwrap();
    assert!(!bad.is_feasible());
    assert!(bad.errors[0].contains("4 cells"), "{:?}", bad.errors);

    let two = Solution::new("sq", [snake().into_routes(), snake().into_routes()].concat());
    let report = verifier.check(&two).unwrap();
    assert!(report.errors[0].contains("2 tours"), "{:?}", report.errors);
    assert_eq!(report.objective, None);
}

#[test]
fn meta_is_sent_along() {
    let Some(verifier) = verifier() else { return };

    let mut json = SolutionFile::from_solution(&snake()).to_json().unwrap();
    json = json.replace(r#""meta":{}"#, r#""meta":{"job":"x|y","params":[1,2.5]}"#);
    let file = SolutionFile::parse(&json).unwrap();

    assert!(verifier.check_file(&file).unwrap().is_feasible());
}

#[test]
fn unknown_instances_are_not_found() {
    let Some(verifier) = verifier() else { return };

    for uid in ["nope", "../sq", ""] {
        let result = verifier.check(&solution(uid, &[(0, 0)]));
        assert!(matches!(result, Err(PortError::NotFound(_))), "{uid:?}");
    }
}

#[test]
fn a_dead_process_is_restarted() {
    let Some(verifier) = verifier() else { return };

    verifier.kill_worker();
    assert!(verifier.check(&snake()).unwrap().is_feasible());

    // and it keeps working on the new process
    assert!(verifier.check(&snake()).unwrap().is_feasible());
}

#[test]
fn shared_between_threads() {
    let Some(verifier) = verifier() else { return };

    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| {
                for _ in 0..5 {
                    assert!(verifier.verify(&snake()).unwrap().is_feasible());
                }
            });
        }
    });
}
