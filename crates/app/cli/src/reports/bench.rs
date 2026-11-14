//! `mow bench pick | score`: the parts of `python -m solver.bench` that no
//! other command covers.
//!
//! - `pick` writes a benchmark list once, as `bench.py pick` does.
//! - `score` scores a job log over the bench instances against a folder of
//!   bests from before, for instance Rust's `mow mine --bench` log against
//!   Python's bests. Each instance's best feasible result in the log counts.
//!
//! - `run` is in [`super::bench_run`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use mow_report::bench::{pick_bench, report, score_run};
use mow_report::bests::read_bests;
use mow_report::csv::Table;
use mow_store::best_objectives;

use super::paths::{prefer_rust, read_catalog, read_log, save};
use crate::args::{parse, Parsed};
use crate::layout::{read_uid_list, DEFAULT_OUT, PYTHON_DATA};

const HEADER: &str = "# fixed benchmark set, written once by mow bench pick\n";

pub fn run(mut raw: impl Iterator<Item = String>) -> Result<(), String> {
    let action = raw.next().unwrap_or_default();
    if action == "run" {
        return super::bench_run::run(raw);
    }

    let valued = [
        "--catalog",
        "--leaderboard",
        "--out",
        "--log",
        "--before",
        "--bench",
        "--only",
    ];
    let parsed = parse(raw, &valued, &[])?;
    if !parsed.positional.is_empty() {
        return Err(format!("unexpected {:?}\n{}", parsed.positional, super::USAGE));
    }

    match action.as_str() {
        "pick" => pick(&parsed),
        "score" => score(&parsed),
        _ => Err(super::USAGE.to_owned()),
    }
}

fn pick(parsed: &Parsed) -> Result<(), String> {
    let out = parsed
        .value("--out")
        .map_or_else(|| Path::new(DEFAULT_OUT).join("bench.txt"), PathBuf::from);
    if out.exists() {
        return Err(format!(
            "{} already exists, delete it first to pick again",
            out.display()
        ));
    }

    let catalog = read_catalog(parsed.value("--catalog"))?;
    let leaderboard = parsed
        .value("--leaderboard")
        .map_or_else(|| prefer_rust("leaderboard.csv"), PathBuf::from);
    let uids = pick_bench(&catalog, &Table::read(&leaderboard)?);

    save(&out, &format!("{HEADER}{}\n", uids.join("\n")))?;
    println!("wrote {} instances to {}", uids.len(), out.display());

    Ok(())
}

fn score(parsed: &Parsed) -> Result<(), String> {
    let log = parsed
        .value("--log")
        .map_or_else(|| Path::new(DEFAULT_OUT).join("jobs.jsonl"), PathBuf::from);
    let before = parsed
        .value("--before")
        .map_or_else(|| Path::new(PYTHON_DATA).join("solutions/best"), PathBuf::from);
    let bench = parsed
        .value("--bench")
        .map_or_else(|| Path::new(PYTHON_DATA).join("bench.txt"), PathBuf::from);

    let only = parsed.value("--only").unwrap_or("");
    let uids: Vec<String> = read_uid_list(&bench)?
        .into_iter()
        .filter(|uid| uid.contains(only))
        .collect();

    let bounds: HashMap<String, u64> = read_catalog(parsed.value("--catalog"))?
        .into_iter()
        .map(|f| (f.instance, f.lower_bound))
        .collect();
    let before: HashMap<String, u64> = read_bests(&before)?
        .into_iter()
        .map(|(uid, best)| (uid, best.objective))
        .collect();
    let ours: HashMap<String, u64> = best_objectives(&read_log(&log)?)
        .into_iter()
        .map(|(uid, (objective, _))| (uid, objective))
        .collect();

    let result = score_run(&uids, &bounds, &before, &ours);
    print!("{}", report(&result, &log.display().to_string()));

    Ok(())
}
