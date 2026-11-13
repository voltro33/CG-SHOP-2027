//! `mow compare` on a hand made store: ratios against both Python columns.

mod common;

use std::fs;

use mow_cli::compare::{self, Check, CompareOptions};
use mow_core::{Point, Route, Solution};
use mow_io::{save_solution, SolutionFile};
use mow_store::{DirStore, JobLog, JobRecord};

/// Every row of the 4 x 4 square, lane by lane: length 18.
fn snake() -> Solution {
    let corners = [(0, 0), (3, 0), (3, 1), (0, 1), (0, 2), (3, 2), (3, 3), (0, 3)];
    let route = Route::new(corners.iter().map(|&(x, y)| Point::new(x, y)).collect()).unwrap();

    Solution::new("sq", vec![route])
}

fn record(round: i32, objective: u64, seconds: f64) -> JobRecord {
    JobRecord {
        job_id: format!("sq|x|s0|v2|{round}"),
        instance: "sq".into(),
        round,
        label: "x".into(),
        objective: Some(objective),
        seconds,
        error: None,
    }
}

#[test]
fn compares_with_the_stored_and_the_mined_best() {
    let instances = common::temp_dir("cmp-instances");
    common::write_instance(&instances, "sq", &[(0, 0), (4, 0), (4, 4), (0, 4)], 1, 1);
    common::write_instance(&instances, "other", &[(0, 0), (2, 0), (2, 2), (0, 2)], 1, 1);

    let python = common::temp_dir("cmp-python");
    fs::create_dir_all(python.join("solutions/best")).unwrap();
    save_solution(&python.join("solutions/best/sq.solution.json"), &snake()).unwrap();
    fs::write(python.join("bench.txt"), "# bench\nsq\n").unwrap();
    let log = JobLog::open(python.join("jobs.jsonl")).unwrap();
    log.append(&record(0, 20, 5.0)).unwrap();
    log.append(&record(-2, 18, 7.0)).unwrap();

    let out = common::temp_dir("cmp-out");
    DirStore::open(&out)
        .unwrap()
        .offer_file(&SolutionFile::from_solution(&snake()))
        .unwrap();

    let options = CompareOptions {
        out: out.clone(),
        report: out.join("report.md"),
        instances,
        python,
        verifiers: 1,
        verify: false,
    };
    let comparison = compare::compare(&options, None).unwrap();

    let sq = comparison.rows.iter().find(|r| r.uid == "sq").unwrap();
    assert_eq!(
        (sq.python_best, sq.python_mined, sq.rust),
        (Some(18), Some(20), Some(18))
    );
    assert_eq!(sq.ratio_best(), Some(1.0));
    assert_eq!(sq.ratio_mined(), Some(0.9));
    assert_eq!(sq.check, Check::NotRun);
    assert!(sq.bench);

    let all = &comparison.groups[0];
    assert_eq!((all.instances, all.solved), (2, 1));
    assert_eq!((all.vs_best.ties, all.vs_mined.wins), (1, 1));
    assert_eq!(comparison.cpu.python_all, 12.0);
    assert_eq!(comparison.cpu.python_mined, 5.0);

    let bench = comparison.groups.last().unwrap();
    assert_eq!((bench.name.as_str(), bench.instances), ("bench", 1));
}
