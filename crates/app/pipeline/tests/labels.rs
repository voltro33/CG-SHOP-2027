//! Every distinct label of the Python job log (`data/jobs.jsonl`, not in
//! git) reads and writes back byte for byte.

use std::collections::BTreeSet;
use std::path::PathBuf;

use mow_pipeline::{SolveConfig, Strategy};

fn job_log() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data/jobs.jsonl")
}

fn labels(text: &str) -> BTreeSet<String> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let job: serde_json::Value = serde_json::from_str(line).expect("a JSON line");
            job["label"].as_str().expect("a label").to_string()
        })
        .collect()
}

#[test]
fn every_job_label_round_trips() {
    let Ok(text) = std::fs::read_to_string(job_log()) else {
        println!("skipped: {} is missing", job_log().display());
        return;
    };

    let labels = labels(&text);
    assert!(!labels.is_empty());

    let mut pipeline_labels = 0;
    for label in &labels {
        let config = SolveConfig::parse(label).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(&config.label(), label);

        if Strategy::of(&config).is_ok() {
            pipeline_labels += 1;
        }
    }

    println!(
        "{} labels round trip, {pipeline_labels} run by the pipeline",
        labels.len()
    );
    assert!(pipeline_labels > 0);
}
