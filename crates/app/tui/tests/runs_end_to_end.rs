//! A real pass through the supervisor: the release `mow`, the official
//! verifier, a scratch data folder. Ignored by default; run with
//! `cargo build --release && cargo test --release -p mow-tui --test runs_end_to_end -- --ignored`.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use mow_tui::binaries;
use mow_tui::data::{Lawn, Paths};
use mow_tui::experiments::{self, Choice};
use mow_tui::recipes::{pass_spec, runs_dir, Pass, Search, Set};
use mow_tui::runs::{process, supervise, Run, Status};

const UID: &str = "jupiter0010000_14k_1d5";
const OTHER: &str = "srpg41087_15k_1s5";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("..")
        .canonicalize()
        .unwrap()
}

/// A data folder holding the catalog and one best, and the lawn of just
/// that instance.
fn scratch(root: &Path, name: &str) -> (Paths, Lawn) {
    let mut paths = mow_tui::paths_under(root, "data/instances/challenge_v0/instances");
    let real = paths.data.clone();
    paths.data = std::env::temp_dir().join(format!("mow-tui-e2e-{name}-{}", std::process::id()));
    paths.mow = root
        .join("target")
        .join("release")
        .join(format!("mow{}", std::env::consts::EXE_SUFFIX));

    let _ = std::fs::remove_dir_all(&paths.data);
    std::fs::create_dir_all(paths.best_dir()).unwrap();
    std::fs::copy(real.join("catalog.csv"), paths.data.join("catalog.csv")).unwrap();
    let file = format!("{UID}.solution.json");
    std::fs::copy(real.join("best").join(&file), paths.best_dir().join(&file)).unwrap();

    let all = Lawn::load(&paths).unwrap();
    let lawn = Lawn {
        instances: vec![all.find(UID).unwrap().clone()],
        latest: None,
    };
    (paths, lawn)
}

fn read(dir: &Path) -> Run {
    let pids = process::running_pids();
    Run::read(dir, |pid| pids.contains(&pid)).unwrap()
}

fn start(paths: &Paths, lawn: &Lawn, seconds: u32) -> PathBuf {
    let pass = Pass {
        search: Search::Lns,
        set: Set::Cutters(1),
        seconds,
        threads: 1,
        pass: 1,
    };
    let spec = pass_spec(paths, lawn, &pass).unwrap();
    supervise::start(&runs_dir(paths), &spec, &paths.mow).unwrap()
}

#[test]
#[ignore]
fn a_pass_runs_to_the_end_and_another_is_stopped() {
    let root = root();
    let real_best = root.join("data/rust/best").join(format!("{UID}.solution.json"));
    let before = std::fs::metadata(&real_best).and_then(|m| m.modified()).ok();
    let (paths, lawn) = scratch(&root, "pass");

    let dir = start(&paths, &lawn, 5);
    let started = Instant::now();
    while read(&dir).is_running() && started.elapsed() < Duration::from_secs(180) {
        std::thread::sleep(Duration::from_millis(500));
    }

    let run = read(&dir);
    println!("{:?} {:?} {:?}", run.status, run.progress, run.done);
    assert_eq!(
        run.status,
        Status::Finished,
        "{}",
        std::fs::read_to_string(dir.join("lns.log")).unwrap_or_default()
    );
    assert_eq!(run.progress.map(|p| (p.done, p.total)), Some((1, 1)));
    assert!(run.done.and_then(|d| d.score_after).is_some());

    let long = start(&paths, &lawn, 600);
    std::thread::sleep(Duration::from_secs(4));
    let running = read(&long);
    assert!(running.is_running());
    let pid = running.pid().unwrap();

    process::stop_tree(pid).unwrap();
    running.mark_stopped().unwrap();
    std::thread::sleep(Duration::from_secs(1));
    assert_eq!(read(&long).status, Status::Stopped);
    assert!(!process::alive(pid));

    let after = std::fs::metadata(&real_best).and_then(|m| m.modified()).ok();
    assert_eq!(before, after, "the real best was touched");
    std::fs::remove_dir_all(&paths.data).ok();
}

fn wait(dir: &Path, limit: Duration) -> Run {
    let started = Instant::now();
    while read(dir).is_running() && started.elapsed() < limit {
        std::thread::sleep(Duration::from_secs(1));
    }
    read(dir)
}

/// Builds the checked-out commit in a worktree (a full build in a fresh
/// target folder: minutes), then runs an experiment between that binary and
/// the release one on two small instances.
#[test]
#[ignore]
fn a_commit_builds_and_an_experiment_compares_two_binaries() {
    let root = root();
    let (paths, _) = scratch(&root, "build");
    let file = format!("{OTHER}.solution.json");
    std::fs::copy(
        root.join("data/rust/best").join(&file),
        paths.best_dir().join(&file),
    )
    .unwrap();
    let all = Lawn::load(&paths).unwrap();
    let lawn = Lawn {
        instances: [UID, OTHER]
            .iter()
            .map(|u| all.find(u).unwrap().clone())
            .collect(),
        latest: None,
    };

    let head = binaries::commits(&paths, 1).unwrap().remove(0);
    let spec = binaries::build_spec(&paths, &lawn, &head).unwrap();
    let build = supervise::start(&runs_dir(&paths), &spec, &paths.mow).unwrap();
    let built = wait(&build, Duration::from_secs(1800));
    assert_eq!(
        built.status,
        Status::Finished,
        "{}",
        std::fs::read_to_string(build.join("cargo.log")).unwrap_or_default()
    );
    assert!(
        !paths.data.join("worktrees").join(&head.short).exists(),
        "the worktree stays"
    );

    let saved = binaries::save_current(&paths).unwrap();
    let kept = binaries::list(&paths);
    let pick = |label: &str| kept.iter().find(|b| b.label == label).unwrap().clone();
    let choice = Choice {
        base: pick(&head.short),
        fast: pick(&saved),
        set: Set::Cutters(1),
        seconds: 10,
        threads: 1,
        seed: 7,
    };

    let spec = experiments::create(&paths, &lawn, &choice).unwrap();
    let run = supervise::start(&runs_dir(&paths), &spec, &paths.mow).unwrap();
    let done = wait(&run, Duration::from_secs(300));
    assert_eq!(done.status, Status::Finished);

    let (dir, setup) = experiments::list(&paths).remove(0);
    let logs = done.logs();
    let report = experiments::report(&dir, &setup, &lawn, (&logs[0], &logs[1])).unwrap();
    println!("{report:?}");
    assert_eq!(report.lines.len(), 2);
    assert!(report
        .lines
        .iter()
        .all(|l| l.moves.0.is_some() && l.moves.1.is_some()));

    std::fs::remove_dir_all(&paths.data).ok();
}
