//! The gate on Python's data: every line of `data/jobs.jsonl` parses, and
//! its best per instance lines up with the stored bests and the leaderboard.
//! Read only; skips when the data is missing.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mow_core::SolutionStore;
use mow_store::{best_objectives, DirStore, JobLog, JobRecord};

fn data() -> Option<PathBuf> {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data");
    let ready = data.join("jobs.jsonl").is_file() && data.join("solutions/best").is_dir();
    if !ready {
        println!("skipped: no {}", data.display());
    }
    ready.then_some(data)
}

/// The leaderboard's columns by name, one map per instance.
fn leaderboard(path: &Path) -> BTreeMap<String, BTreeMap<String, String>> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return BTreeMap::new();
    };
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().unwrap_or_default().split(',').collect();

    lines
        .map(|line| {
            let row: BTreeMap<String, String> = header
                .iter()
                .zip(line.split(','))
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            (row["instance"].clone(), row)
        })
        .collect()
}

/// The best of rounds 0 and 1, the leaderboard's `baseline`.
fn baselines(records: &[JobRecord]) -> BTreeMap<String, (u64, String)> {
    let baseline: Vec<JobRecord> = records
        .iter()
        .filter(|r| r.round == 0 || r.round == 1)
        .cloned()
        .collect();
    best_objectives(&baseline)
}

#[test]
fn python_job_log_and_stored_bests() {
    let Some(data) = data() else { return };

    let path = data.join("jobs.jsonl");
    let lines = std::fs::read_to_string(&path).unwrap().lines().count();
    let records = JobLog::read(&path).unwrap();
    println!("{} of {lines} lines parse", records.len());
    assert_eq!(records.len(), lines);

    let best = best_objectives(&records);
    let store = DirStore::open(data.join("solutions")).unwrap();
    let (mut equal, mut above, mut below) = (0, 0, 0);
    for (uid, (objective, _)) in &best {
        let stored = store.best(uid).unwrap().map(|s| s.objective());
        match stored {
            Some(stored) if *objective == stored => equal += 1,
            Some(stored) if *objective > stored => above += 1,
            _ => below += 1,
        }
    }
    println!(
        "{} instances in the log: job-log best equals the stored best on {equal}, \
         is above it on {above}, below or without one on {below}",
        best.len()
    );
    assert_eq!(below, 0, "a stored best is worse than the log says");

    // the CP-SAT polish runs are logged with round -2; without them
    let mined: Vec<JobRecord> = records.iter().filter(|r| r.round >= 0).cloned().collect();
    let mined = best_objectives(&mined);
    let polished = mined
        .iter()
        .filter(|(uid, (objective, _))| best[*uid].0 < *objective)
        .count();
    println!(
        "without the polish rounds: {} instances, {polished} stored bests shorter than the mined best",
        mined.len()
    );

    let board = leaderboard(&data.join("leaderboard.csv"));
    let bases = baselines(&records);
    let mut checked = 0;
    for (uid, row) in board.iter().step_by(25) {
        let stored = store.best(uid).unwrap().map(|s| s.objective().to_string());
        assert_eq!(Some(&row["best"]), stored.as_ref(), "{uid}: best");

        let base = bases.get(uid).map(|(o, _)| o.to_string()).unwrap_or_default();
        assert_eq!(row["baseline"], base, "{uid}: baseline");

        let (log_best, label) = &best[uid];
        println!(
            "{uid}: best {} ({}), baseline {}, job-log best {log_best} ({label})",
            row["best"], row["best_config"], row["baseline"]
        );
        checked += 1;
    }
    println!("{checked} leaderboard rows spot checked");
}
