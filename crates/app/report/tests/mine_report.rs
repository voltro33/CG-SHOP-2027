//! The mining report over Python's job log prints and writes, byte for
//! byte, what Python prints and writes.

mod common;

use common::{data, data_matches_fixtures, fixture, read_fixture};
use mow_report::catalog;
use mow_report::mining::{report, text, to_json, Instances};

/// `jobs.time_limit_for`, the miner's cap.
fn time_limit(cells: usize) -> f64 {
    match cells > 1_000_000 {
        true => 600.0,
        false => 120.0,
    }
}

#[test]
fn mine_report_equals_python() {
    if !data_matches_fixtures() {
        return;
    }

    let catalog = catalog::read(&fixture("catalog.csv")).unwrap();
    let instances = Instances::of(&catalog);
    let records = instances.records(mow_store::JobLog::read(&data("jobs.jsonl")).unwrap());
    let result = report(&records, &instances, time_limit);

    assert_eq!(text(&result), read_fixture("mine_report.txt"));

    // Python writes no final newline, the end of file hook adds one
    let json = read_fixture("mine_report.json");
    assert_eq!(to_json(&result).dump(), json.trim_end());
    println!(
        "{} configs, {} rounds equal",
        result.configs.len(),
        result.rounds.len()
    );
}
