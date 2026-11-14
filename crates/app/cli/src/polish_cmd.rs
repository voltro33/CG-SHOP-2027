//! `mow polish`: CP-SAT window polish of the stored bests (port of
//! `python -m solver.polish`).
//!
//! - Without `--all`: the bench instances, `--seconds` each, logged to a
//!   fresh `<out>/bench/<stamp>.jsonl` and scored against the bests from
//!   before, with the points gained per CPU hour.
//! - With `--all`: every stored best up to `--max-cells`, `polish_seconds` of
//!   its size each, through the resumable `<out>/jobs.jsonl` within `--hours`,
//!   as mining.
//!
//! Each job runs on one miner thread with one CP-SAT process; every shorter
//! result passes the official verifier before the store takes it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use mow_cpsat::CpSatPool;
use mow_polish::{DEFAULT_SIDE, DEFAULT_WINDOW_SECONDS};
use mow_report::bench::{report, score_run};
use mow_report::bests::read_bests;
use mow_store::{best_objectives, JobRecord};

use crate::args::{parse, Parsed};
use crate::layout::{read_uid_list, repo_root, DEFAULT_INSTANCES, DEFAULT_OUT, PYTHON_DATA};
use crate::mine::polish::{config, polish_jobs};
use crate::mine::{read_instances, run_jobs, MineOptions};
use crate::official::VerifierPool;
use crate::reports::bench_run::stamp;
use crate::reports::paths::{read_catalog, read_log, save};

pub const USAGE: &str =
    "usage: mow polish [--all [--hours H] [--max-cells N]] [--seconds S] [--side N] [--window-seconds F] \
                         [--seed N] [--aim] [--only TEXT] [--bench FILE] [--threads N] [--verifiers N] \
                         [--instances DIR] [--catalog FILE] [--out data/rust]";

/// Seconds per instance without `--all`, as `polish.py`.
const DEFAULT_SECONDS: f64 = 300.0;

/// The budget of `--all`, hours.
const DEFAULT_HOURS: f64 = 6.0;

/// The largest instance `--all` polishes by default. On 2026-11-13 polish
/// earned 0.34 of its 0.39 points below this size, in 27 of its 59.9 hours.
const DEFAULT_MAX_CELLS: usize = 400_000;

pub fn run(raw: impl Iterator<Item = String>) -> Result<(), String> {
    let valued = [
        "--hours",
        "--seconds",
        "--side",
        "--window-seconds",
        "--seed",
        "--max-cells",
        "--only",
        "--bench",
        "--threads",
        "--verifiers",
        "--instances",
        "--catalog",
        "--out",
    ];
    let parsed = parse(raw, &valued, &["--all", "--aim"])?;
    if !parsed.positional.is_empty() {
        return Err(format!("unexpected {:?}\n{USAGE}", parsed.positional));
    }

    let every = parsed.switch("--all");
    let options = options(&parsed, every)?;
    let side: i32 = parsed.number("--side")?.unwrap_or(DEFAULT_SIDE);
    if side < 1 {
        return Err(format!("--side must be at least 1, not {side}"));
    }
    let window_seconds = parsed
        .number("--window-seconds")?
        .unwrap_or(DEFAULT_WINDOW_SECONDS);
    let config = config(
        side,
        window_seconds,
        parsed.number("--seed")?.unwrap_or(0),
        parsed.switch("--aim"),
    );

    // the bests before the run: what is polished, and the score's baseline
    let before = read_bests(&options.out.join("best"))?;
    let uids = wanted(&parsed, every, &before.keys().cloned().collect::<Vec<_>>())?;
    if uids.is_empty() {
        return Err(format!(
            "no stored best to polish in {}",
            options.out.join("best").display()
        ));
    }

    let max_cells = match every {
        true => parsed.number("--max-cells")?.unwrap_or(DEFAULT_MAX_CELLS),
        false => usize::MAX,
    };
    let instances: Vec<_> = read_instances(&options.instances, Some(&uids))?
        .into_iter()
        .filter(|instance| instance.cells <= max_cells)
        .collect();
    let seconds = match every {
        true => None,
        false => Some(parsed.number("--seconds")?.unwrap_or(DEFAULT_SECONDS)),
    };
    let jobs = polish_jobs(&instances, &config, seconds);

    let stamp = stamp();
    let log = match every {
        true => options.out.join("jobs.jsonl"),
        false => options.out.join("bench").join(format!("polish-{stamp}.jsonl")),
    };

    let verifier = VerifierPool::start(&repo_root(), &options.instances, options.verifiers)
        .map_err(|e| e.to_string())?;
    let cpsat = CpSatPool::from_repo(&repo_root(), options.threads).map_err(|e| e.to_string())?;
    let summary = run_jobs(&options, jobs, &log, &verifier, Some(&cpsat))?;
    println!(
        "{} polish jobs: {} run, {} failed, {} new bests, {:.1} min",
        summary.total,
        summary.done,
        summary.failed,
        summary.bests,
        summary.seconds / 60.0
    );

    if every {
        return Ok(());
    }

    let before: HashMap<String, u64> = before.into_iter().map(|(uid, b)| (uid, b.objective)).collect();
    let text = score(&parsed, &uids, &before, &read_log(&log)?, &config.label())?;
    print!("{text}");
    save(&log.with_extension("summary.txt"), &text)
}

/// The miner settings of a polish run. All but one core by default, as
/// `polish.py` takes them.
pub(crate) fn options(parsed: &Parsed, every: bool) -> Result<MineOptions, String> {
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    let threads: usize = parsed.number("--threads")?.unwrap_or(cores.saturating_sub(1));
    let hours = match every {
        true => Some(parsed.number("--hours")?.unwrap_or(DEFAULT_HOURS)),
        false => None,
    };

    Ok(MineOptions {
        instances: PathBuf::from(parsed.value("--instances").unwrap_or(DEFAULT_INSTANCES)),
        only: None,
        rounds: Vec::new(),
        threads: threads.max(1),
        hours,
        kick_scale: 1.0,
        time: None,
        verifiers: parsed.number("--verifiers")?.unwrap_or(2),
        out: PathBuf::from(parsed.value("--out").unwrap_or(DEFAULT_OUT)),
    })
}

/// The instances to polish: every stored best with `--all`, else the bench
/// instances that have one, filtered by `--only`.
pub(crate) fn wanted(parsed: &Parsed, every: bool, stored: &[String]) -> Result<Vec<String>, String> {
    let only = parsed.value("--only").unwrap_or("");
    let pool = match every {
        true => stored.to_vec(),
        false => {
            let bench = parsed
                .value("--bench")
                .map_or_else(|| Path::new(PYTHON_DATA).join("bench.txt"), PathBuf::from);
            read_uid_list(&bench)?
                .into_iter()
                .filter(|uid| stored.contains(uid))
                .collect()
        }
    };

    Ok(pool.into_iter().filter(|uid| uid.contains(only)).collect())
}

/// The bench score of the run, then the points it gained per CPU hour:
/// `polish.py` prints them exactly, since a polish gain is too small for the
/// rounded score.
pub(crate) fn score(
    parsed: &Parsed,
    uids: &[String],
    before: &HashMap<String, u64>,
    records: &[JobRecord],
    label: &str,
) -> Result<String, String> {
    let bounds: HashMap<String, u64> = read_catalog(parsed.value("--catalog"))?
        .into_iter()
        .map(|f| (f.instance, f.lower_bound))
        .collect();
    let ours: HashMap<String, u64> = best_objectives(records)
        .into_iter()
        .map(|(uid, (objective, _))| (uid, objective))
        .collect();

    let mut points = 0.0;
    for (uid, &objective) in &ours {
        let (Some(&bound), Some(&was)) = (bounds.get(uid), before.get(uid)) else {
            continue;
        };
        let bound = bound as f64;
        points += (bound / objective as f64).powi(2) - (bound / was as f64).powi(2);
    }

    let cpu_hours: f64 = records.iter().map(|r| r.seconds).sum::<f64>() / 3600.0;
    let per_hour = match cpu_hours > 0.0 {
        true => format!("{:.3}", points / cpu_hours),
        false => "n/a".to_owned(),
    };

    Ok(format!(
        "{}cpu hours {cpu_hours:.2}, points {points:.3}, points per cpu hour {per_hour}\n",
        report(&score_run(uids, &bounds, before, &ours), label)
    ))
}
