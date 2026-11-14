//! `mow turns`: the turn-shape corner search on the stored bests, run the
//! way `mow polish` runs.
//!
//! - Without `--all`: the bench instances, logged to a fresh
//!   `<out>/bench/turns-<stamp>.jsonl` and scored against the bests from
//!   before, with the points gained per CPU hour.
//! - With `--all`: every stored best, through the resumable
//!   `<out>/jobs.jsonl` within `--hours`, as mining.
//!
//! Each job runs on one miner thread; every shorter result passes the
//! official verifier before the store takes it.

use std::collections::HashMap;
use std::path::PathBuf;

use mow_report::bests::read_bests;
use mow_turns::{DEFAULT_SECONDS, MOVES};

use crate::args::{parse, Parsed};
use crate::layout::{repo_root, DEFAULT_INSTANCES, DEFAULT_OUT};
use crate::mine::turns::{config, turns_jobs, ALL_MOVES};
use crate::mine::{read_instances, run_jobs, MineOptions};
use crate::official::VerifierPool;
use crate::polish_cmd::{score, wanted};
use crate::reports::bench_run::stamp;
use crate::reports::paths::{read_log, save};

pub const USAGE: &str = "usage: mow turns [--all [--hours H]] [--seconds S] [--window N] [--moves slide,shortcut,2-opt,or-opt] [--no-balance] \
                         [--only TEXT] [--bench FILE] [--threads N] [--verifiers N] \
                         [--instances DIR] [--catalog FILE] [--out data/rust]";

/// The budget of `--all`, hours.
const DEFAULT_HOURS: f64 = 6.0;

/// The most corners one shortcut skips, by default.
const DEFAULT_WINDOW: usize = 6;

pub fn run(raw: impl Iterator<Item = String>) -> Result<(), String> {
    let valued = [
        "--hours",
        "--seconds",
        "--window",
        "--moves",
        "--only",
        "--bench",
        "--threads",
        "--verifiers",
        "--instances",
        "--catalog",
        "--out",
    ];
    let parsed = parse(raw, &valued, &["--all", "--no-balance"])?;
    if !parsed.positional.is_empty() {
        return Err(format!("unexpected {:?}\n{USAGE}", parsed.positional));
    }

    let every = parsed.switch("--all");
    let options = options(&parsed, every)?;
    let window: usize = parsed.number("--window")?.unwrap_or(DEFAULT_WINDOW);
    let config = config(
        window.max(2),
        moves(parsed.value("--moves"))?,
        !parsed.switch("--no-balance"),
    );
    let seconds: f64 = parsed.number("--seconds")?.unwrap_or(DEFAULT_SECONDS);

    // the bests before the run: what is searched, and the score's baseline
    let before = read_bests(&options.out.join("best"))?;
    let uids = wanted(&parsed, every, &before.keys().cloned().collect::<Vec<_>>())?;
    if uids.is_empty() {
        return Err(format!(
            "no stored best in {}",
            options.out.join("best").display()
        ));
    }

    let instances = read_instances(&options.instances, Some(&uids))?;
    let jobs = turns_jobs(&instances, &config, seconds);

    let log = match every {
        true => options.out.join("jobs.jsonl"),
        false => options.out.join("bench").join(format!("turns-{}.jsonl", stamp())),
    };

    let verifier = VerifierPool::start(&repo_root(), &options.instances, options.verifiers)
        .map_err(|e| e.to_string())?;
    let summary = run_jobs(&options, jobs, &log, &verifier, None)?;
    println!(
        "{} turns jobs: {} run, {} failed, {} new bests, {:.1} min",
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

/// The move mask of `--moves`, every move without it.
fn moves(text: Option<&str>) -> Result<u32, String> {
    let Some(text) = text else { return Ok(ALL_MOVES) };

    text.split(',')
        .map(|name| {
            MOVES
                .iter()
                .position(|&m| m == name.trim())
                .map(|m| 1 << m)
                .ok_or_else(|| format!("unknown move {name:?}, expected one of {MOVES:?}"))
        })
        .sum()
}

/// The miner settings of a turns run: all but one core by default.
fn options(parsed: &Parsed, every: bool) -> Result<MineOptions, String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_by_name() {
        assert_eq!(moves(None), Ok(ALL_MOVES));
        assert_eq!(moves(Some("slide, 2-opt")), Ok(0b0101));
        assert!(moves(Some("kick")).is_err());
    }
}
