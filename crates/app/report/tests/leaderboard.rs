//! The leaderboard, its summary and the bench pick equal Python's, from the
//! stored bests and the job log in `data/`.

mod common;

use common::{data, data_matches_fixtures, fixture, read_fixture};
use mow_report::bests::read_bests;
use mow_report::csv::Table;
use mow_report::{bench, catalog, leaderboard, summary};
use mow_store::JobRecord;

fn python_log() -> Vec<JobRecord> {
    mow_store::JobLog::read(&data("jobs.jsonl")).unwrap()
}

#[test]
fn leaderboard_and_summary_equal_python() {
    if !data_matches_fixtures() {
        return;
    }

    let catalog = catalog::read(&fixture("catalog.csv")).unwrap();
    let bests = read_bests(&data("solutions/best")).unwrap();
    let rows = leaderboard::rows(&catalog, &bests, &python_log());

    assert_eq!(leaderboard::to_csv(&rows), read_fixture("leaderboard.csv"));

    let challenge = catalog.iter().filter(|f| f.is_challenge()).count();
    assert_eq!(
        summary::summary(&rows, challenge).dump() + "\n",
        read_fixture("summary.json")
    );
    println!("{} leaderboard rows equal", rows.len());
}

#[test]
fn bench_pick_equals_python() {
    let leaderboard_csv = data("leaderboard.csv");
    if !leaderboard_csv.is_file() {
        println!("skipped: {} not found", leaderboard_csv.display());
        return;
    }

    let catalog = catalog::read(&fixture("catalog.csv")).unwrap();
    let picked = bench::pick_bench(&catalog, &Table::read(&leaderboard_csv).unwrap());

    let expected: Vec<String> = read_fixture("bench_pick.txt")
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(picked, expected);
}
