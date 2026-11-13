//! Offers on a fresh store in a temporary directory.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use mow_core::{Offer, Point, PortError, Route, Solution, SolutionStore};
use mow_io::{save_solution, SolutionFile};
use mow_store::DirStore;

fn scratch() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);

    let dir = std::env::temp_dir().join(format!("mow-store-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

/// An out-and-back tour of objective `2 * half`.
fn line(uid: &str, half: i32) -> Solution {
    let route = Route::new(vec![Point::new(0, 0), Point::new(half, 0)]).unwrap();
    Solution::new(uid, vec![route])
}

fn with_job(solution: &Solution, job: &str) -> SolutionFile {
    let json = SolutionFile::from_solution(solution).to_json().unwrap();
    let json = json.replace(r#""meta":{}"#, &format!(r#""meta":{{"job":"{job}"}}"#));
    SolutionFile::parse(&json).unwrap()
}

fn candidates(root: &Path, uid: &str) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(root.join("candidates").join(uid))
        .map(|entries| {
            entries
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[test]
fn best_then_candidate_then_rejected() {
    let root = scratch();
    let store = DirStore::open(&root).unwrap();

    assert_eq!(store.best("a").unwrap(), None);
    assert_eq!(store.offer(&line("a", 100)).unwrap(), Offer::Best);
    assert_eq!(store.offer(&line("a", 100)).unwrap(), Offer::Candidate);
    assert_eq!(store.offer(&line("a", 104)).unwrap(), Offer::Candidate);
    assert_eq!(store.offer(&line("a", 106)).unwrap(), Offer::Rejected);
    assert_eq!(store.offer(&line("a", 90)).unwrap(), Offer::Best);

    assert_eq!(store.best("a").unwrap(), Some(line("a", 90)));
    assert_eq!(store.best_objective("a").unwrap(), Some(180));
    assert_eq!(
        candidates(&root, "a"),
        vec![
            "objective-180.solution.json",
            "objective-200.solution.json",
            "objective-208.solution.json",
        ]
    );
}

#[test]
fn candidates_are_named_after_the_job_and_keep_meta() {
    let root = scratch();
    let store = DirStore::open(&root).unwrap();

    let file = with_job(&line("b", 10), "b|groups-h/gap_factor=0.5|s0|v2");
    assert_eq!(store.offer_file(&file).unwrap(), Offer::Best);

    assert_eq!(
        candidates(&root, "b"),
        vec!["b_groups-h_gap_factor=0.5_s0_v2.solution.json"]
    );
    let stored = store.best_file("b").unwrap().unwrap();
    assert_eq!(stored.meta.get(), r#"{"job":"b|groups-h/gap_factor=0.5|s0|v2"}"#);
}

#[test]
fn a_better_best_written_by_someone_else_wins() {
    let root = scratch();
    let store = DirStore::open(&root).unwrap();
    assert_eq!(store.offer(&line("c", 100)).unwrap(), Offer::Best);

    // another process stores a shorter one behind our back
    save_solution(&store.best_path("c").unwrap(), &line("c", 50)).unwrap();

    assert_eq!(store.offer(&line("c", 80)).unwrap(), Offer::Candidate);
    assert_eq!(store.best("c").unwrap(), Some(line("c", 50)));
    assert_eq!(store.offer(&line("c", 60)).unwrap(), Offer::Rejected);
}

#[test]
fn an_existing_best_is_respected_on_open() {
    let root = scratch();
    fs::create_dir_all(root.join("best")).unwrap();
    save_solution(&root.join("best/d.solution.json"), &line("d", 10)).unwrap();

    let store = DirStore::open(&root).unwrap();
    assert_eq!(store.offer(&line("d", 20)).unwrap(), Offer::Rejected);
    assert_eq!(store.offer(&line("d", 9)).unwrap(), Offer::Best);
}

#[test]
fn bad_uids_are_errors() {
    let store = DirStore::open(scratch()).unwrap();

    for uid in ["", "..", "../x", "a/b", "c:d"] {
        assert!(matches!(store.best(uid), Err(PortError::NotFound(_))), "{uid:?}");
        assert!(matches!(store.offer(&line(uid, 1)), Err(PortError::NotFound(_))));
    }
}

#[test]
fn concurrent_offers_keep_the_shortest() {
    let root = scratch();
    let store = DirStore::open(&root).unwrap();

    std::thread::scope(|scope| {
        for thread in 0..8 {
            let store = &store;
            scope.spawn(move || {
                for i in 0..20 {
                    store.offer(&line("e", 1000 - thread * 20 - i)).unwrap();
                }
            });
        }
    });

    // 1000 - 7 * 20 - 19 is the shortest any thread offered
    assert_eq!(store.best("e").unwrap(), Some(line("e", 841)));
    assert!(!fs::read_dir(root.join("best")).unwrap().any(|e| e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .ends_with(".tmp")));
}
