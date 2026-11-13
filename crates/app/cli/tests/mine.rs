//! `mow mine` end to end on two tiny lawns, then `mow compare` on its store.

mod common;

use std::fs;

use mow_cli::compare::{self, Check, CompareOptions};
use mow_cli::meta::meta_field;
use mow_cli::mine::{self, MineOptions};
use mow_store::{DirStore, JobLog};

fn options(instances: &std::path::Path, out: &std::path::Path) -> MineOptions {
    MineOptions {
        instances: instances.to_path_buf(),
        only: None,
        rounds: vec![0, 1],
        threads: 4,
        hours: None,
        kick_scale: 0.01,
        time: Some(1.0),
        verifiers: 1,
        out: out.to_path_buf(),
    }
}

#[test]
fn mines_verifies_restarts_and_compares() {
    let instances = common::temp_dir("instances");
    common::two_instances(&instances);
    let Some(official) = common::verifier(&instances) else {
        return;
    };
    let out = common::temp_dir("out");

    // rect (k = 1): 2 + 2 jobs; ell (k = 2): 2 + 6 jobs
    let summary = mine::mine(&options(&instances, &out), &official).unwrap();
    assert_eq!((summary.total, summary.done, summary.skipped), (12, 12, 0));
    assert_eq!(summary.failed, 0, "{summary:?}");
    assert!(summary.bests >= 2);

    let records = JobLog::read(&out.join("jobs.jsonl")).unwrap();
    assert_eq!(records.len(), 12);
    assert!(records.iter().all(|r| r.job_id.ends_with("|vr1-k0.01")));

    let store = DirStore::open(&out).unwrap();
    for uid in ["rect", "ell"] {
        let best = store.best_file(uid).unwrap().unwrap();
        assert_eq!(meta_field(&best, "solver").as_deref(), Some("rust"));

        let logged = records
            .iter()
            .filter(|r| r.instance == uid)
            .filter_map(|r| r.feasible_objective())
            .min();
        assert_eq!(logged, Some(best.to_solution().unwrap().objective()), "{uid}");
    }

    // a restart finds everything done
    let again = mine::mine(&options(&instances, &out), &official).unwrap();
    assert_eq!((again.skipped, again.done), (12, 0));

    let python = common::temp_dir("python");
    let report = out.join("report.md");
    let compare_options = CompareOptions {
        out: out.clone(),
        report: report.clone(),
        instances: instances.clone(),
        python,
        verifiers: 1,
        verify: true,
    };
    let comparison = compare::compare(&compare_options, Some(&official)).unwrap();

    assert_eq!(comparison.rows.len(), 2);
    assert_eq!(comparison.failures, 0);
    assert!(comparison.rows.iter().all(|r| r.check == Check::Passed));
    assert!(comparison.cpu.rust > 0.0);

    assert!(fs::read_to_string(&report).unwrap().contains("| rect | 1 |"));
    assert_eq!(
        fs::read_to_string(out.join("compare.csv"))
            .unwrap()
            .lines()
            .count(),
        3
    );
}

#[test]
fn time_up_starts_nothing() {
    let instances = common::temp_dir("instances");
    common::two_instances(&instances);
    let Some(official) = common::verifier(&instances) else {
        return;
    };
    let out = common::temp_dir("out");

    let options = MineOptions {
        hours: Some(0.0),
        ..options(&instances, &out)
    };
    let summary = mine::mine(&options, &official).unwrap();

    assert_eq!((summary.done, summary.unstarted), (0, 12));
}
