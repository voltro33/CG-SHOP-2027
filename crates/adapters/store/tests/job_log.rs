//! The job log on hand made records.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use mow_store::{best_objectives, JobLog, JobRecord};

fn scratch() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);

    let dir = std::env::temp_dir().join(format!("mow-store-log-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir.join("logs/jobs.jsonl")
}

fn record(instance: &str, label: &str, objective: Option<u64>) -> JobRecord {
    JobRecord {
        job_id: format!("{instance}|{label}|s0|v2"),
        instance: instance.to_owned(),
        round: 2,
        label: label.to_owned(),
        objective,
        seconds: 1.25,
        error: objective.is_none().then(|| "boom".to_owned()),
    }
}

#[test]
fn appended_records_read_back() {
    let path = scratch();
    let records = vec![record("a", "x", Some(10)), record("a", "y", None)];

    let log = JobLog::open(&path).unwrap();
    for r in &records {
        log.append(r).unwrap();
    }
    drop(log);

    assert_eq!(JobLog::read(&path).unwrap(), records);
}

#[test]
fn reads_python_lines_and_skips_a_broken_tail() {
    let path = scratch();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let python = concat!(
        r#"{"job_id": "a|x|s0", "instance": "a", "round": 0, "label": "x", "objective": 12, "seconds": 5.58, "error": null}"#,
        "\n",
        r#"{"job_id": "a|y|s0", "instance": "a", "round": -2, "label": "y", "objective": null, "seconds": 0.0, "error": "worker crashed"}"#,
        "\n",
        r#"{"job_id": "a|z|s0", "instance": "a", "rou"#,
    );
    fs::write(&path, python).unwrap();

    let read = JobLog::read(&path).unwrap();
    assert_eq!(read.len(), 2);
    assert_eq!(read[0].seconds, 5.58);
    assert_eq!(read[1].round, -2);
    assert_eq!(read[1].error.as_deref(), Some("worker crashed"));

    // appending after the crash starts on a fresh line
    JobLog::open(&path)
        .unwrap()
        .append(&record("b", "w", Some(3)))
        .unwrap();
    let read = JobLog::read(&path).unwrap();
    assert_eq!(read.len(), 3);
    assert_eq!(read[2].instance, "b");
}

#[test]
fn a_missing_log_is_empty() {
    assert!(JobLog::read(&scratch()).unwrap().is_empty());
}

#[test]
fn threads_append_whole_lines() {
    let path = scratch();
    let log = JobLog::open(&path).unwrap();

    std::thread::scope(|scope| {
        for t in 0..4 {
            let log = &log;
            scope.spawn(move || {
                for i in 0..25 {
                    log.append(&record(&format!("t{t}"), &format!("l{i}"), Some(i)))
                        .unwrap();
                }
            });
        }
    });

    assert_eq!(JobLog::read(&path).unwrap().len(), 100);
}

#[test]
fn best_per_instance_ignores_failures_and_keeps_the_first_tie() {
    let mut failed = record("a", "f", Some(1));
    failed.error = Some("infeasible".to_owned());

    let records = vec![
        record("a", "x", Some(10)),
        record("a", "y", Some(10)),
        failed,
        record("a", "z", None),
        record("b", "x", Some(7)),
        record("b", "y", Some(5)),
        record("c", "x", None),
    ];
    let best = best_objectives(&records);

    assert_eq!(best.len(), 2);
    assert_eq!(best["a"], (10, "x".to_owned()));
    assert_eq!(best["b"], (5, "y".to_owned()));
}
