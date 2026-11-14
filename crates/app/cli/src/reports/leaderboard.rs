//! `mow report`: the leaderboard of a data folder, written as CSV, and its
//! summary printed as JSON (`python -m solver.report`).

use std::path::{Path, PathBuf};

use mow_report::bests::read_bests;
use mow_report::{leaderboard, summary};

use super::paths::{best_dir, is_protected, read_catalog, read_log, save};
use crate::args::parse;
use crate::layout::DEFAULT_OUT;

pub fn run(raw: impl Iterator<Item = String>) -> Result<(), String> {
    let parsed = parse(raw, &["--data", "--catalog", "--out"], &[])?;
    if !parsed.positional.is_empty() {
        return Err(format!("unexpected {:?}\n{}", parsed.positional, super::USAGE));
    }

    let data = PathBuf::from(parsed.value("--data").unwrap_or(DEFAULT_OUT));
    let out = parsed
        .value("--out")
        .map_or_else(|| default_out(&data), PathBuf::from);

    let catalog = read_catalog(parsed.value("--catalog"))?;
    let bests = read_bests(&best_dir(&data))?;
    let records = read_log(&data.join("jobs.jsonl"))?;

    let rows = leaderboard::rows(&catalog, &bests, &records);
    save(&out, &leaderboard::to_csv(&rows))?;

    let challenge = catalog.iter().filter(|f| f.is_challenge()).count();
    println!("{}", summary::summary(&rows, challenge).dump());
    eprintln!("{} rows -> {}", rows.len(), out.display());

    Ok(())
}

/// `<data>/leaderboard.csv`, or `data/rust/python-leaderboard.csv` where
/// that would be Python's own file.
fn default_out(data: &Path) -> PathBuf {
    let next_to_data = data.join("leaderboard.csv");

    match is_protected(&next_to_data) {
        true => Path::new(DEFAULT_OUT).join("python-leaderboard.csv"),
        false => next_to_data,
    }
}
