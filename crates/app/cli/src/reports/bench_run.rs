//! `mow bench run <config | best>`: one config on every bench instance, as
//! `bench.py run` does.
//!
//! - `best` runs each instance with the config that made its stored best
//!   (the config a polished best started from, for a polished one).
//! - The jobs go through the miner (official verifier, the store in
//!   `--out`), biggest instance first, in round -1. Each run gets a fresh
//!   log `<out>/bench/<stamp>.jsonl`, so no job is skipped as done.
//! - The run is scored against the bests from before it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use mow_pipeline::SolveConfig;
use mow_report::bench::{report, score_run};
use mow_report::bests::{read_bests, Best};
use mow_store::best_objectives;

use super::paths::{read_catalog, read_log, save};
use crate::args::parse;
use crate::grid::{time_limit_for, Job};
use crate::layout::{read_uid_list, repo_root, DEFAULT_INSTANCES, DEFAULT_OUT, PYTHON_DATA};
use crate::mine::{read_instances, run_jobs, MineOptions};
use crate::official::VerifierPool;

/// The round bench jobs are logged in, `BENCH_ROUND` of `bench.py`.
pub const BENCH_ROUND: i32 = -1;

pub const USAGE: &str = "usage: mow bench run <config | best> [--seconds S] [--only TEXT] \
                         [--bench data/bench.txt] [--threads N] [--kick-scale F] [--verifiers N] \
                         [--instances DIR] [--catalog FILE] [--out data/rust]";

pub fn run(raw: impl Iterator<Item = String>) -> Result<(), String> {
    let valued = [
        "--seconds",
        "--only",
        "--bench",
        "--threads",
        "--kick-scale",
        "--verifiers",
        "--instances",
        "--catalog",
        "--out",
    ];
    let parsed = parse(raw, &valued, &[])?;
    let [config]: [String; 1] = parsed
        .positional
        .clone()
        .try_into()
        .map_err(|_| format!("need one config label or `best`\n{USAGE}"))?;

    let bench = parsed
        .value("--bench")
        .map_or_else(|| Path::new(PYTHON_DATA).join("bench.txt"), PathBuf::from);
    let only = parsed.value("--only").unwrap_or("");
    let uids: Vec<String> = read_uid_list(&bench)?
        .into_iter()
        .filter(|uid| uid.contains(only))
        .collect();

    let threads: Option<usize> = parsed.number("--threads")?;
    let options = MineOptions {
        instances: PathBuf::from(parsed.value("--instances").unwrap_or(DEFAULT_INSTANCES)),
        only: Some(uids.clone()),
        rounds: Vec::new(),
        threads: threads
            .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()))
            .max(1),
        hours: None,
        kick_scale: parsed.number("--kick-scale")?.unwrap_or(1.0),
        time: None,
        verifiers: parsed.number("--verifiers")?.unwrap_or(2),
        out: PathBuf::from(parsed.value("--out").unwrap_or(DEFAULT_OUT)),
    };

    // the bests before the run: the score's baseline, and `best`'s configs
    let before = read_bests(&options.out.join("best"))?;
    let jobs = bench_jobs(&options, &uids, &config, &before, parsed.number("--seconds")?)?;

    let folder = options.out.join("bench");
    let stamp = stamp();
    let log = folder.join(format!("{stamp}.jsonl"));

    let pool = VerifierPool::start(&repo_root(), &options.instances, options.verifiers)
        .map_err(|e| e.to_string())?;
    run_jobs(&options, jobs, &log, &pool, None)?;

    let bounds: HashMap<String, u64> = read_catalog(parsed.value("--catalog"))?
        .into_iter()
        .map(|f| (f.instance, f.lower_bound))
        .collect();
    let before: HashMap<String, u64> = before.into_iter().map(|(uid, b)| (uid, b.objective)).collect();
    let ours: HashMap<String, u64> = best_objectives(&read_log(&log)?)
        .into_iter()
        .map(|(uid, (objective, _))| (uid, objective))
        .collect();

    let text = report(&score_run(&uids, &bounds, &before, &ours), &config);
    print!("{text}");
    save(&folder.join(format!("{stamp}.summary.txt")), &text)?;

    Ok(())
}

/// One job per bench instance, biggest first.
fn bench_jobs(
    options: &MineOptions,
    uids: &[String],
    config: &str,
    before: &std::collections::BTreeMap<String, Best>,
    seconds: Option<f64>,
) -> Result<Vec<Job>, String> {
    let instances = read_instances(&options.instances, Some(uids))?;
    let mut jobs = Vec::with_capacity(instances.len());

    for instance in instances {
        let label = match config {
            "best" => best_label(before.get(&instance.uid))
                .ok_or_else(|| format!("{}: no stored best to take a config from", instance.uid))?,
            label => label.to_owned(),
        };
        let config = SolveConfig::parse(&label).map_err(|e| format!("{}: {e}", instance.uid))?;

        // by default the same wall clock cap the miner used
        let time_limit = seconds.unwrap_or_else(|| time_limit_for(instance.cells));

        jobs.push(Job {
            instance: instance.uid,
            cells: instance.cells,
            round: BENCH_ROUND,
            config,
            time_limit,
        });
    }

    jobs.sort_by(|a, b| b.cells.cmp(&a.cells));

    Ok(jobs)
}

/// The config label in a best's `meta.job` (`uid|label|s<seed>|v<version>`).
/// A polished best names the job it started from in `meta.polished_from`.
fn best_label(best: Option<&Best>) -> Option<String> {
    let best = best?;
    let label = best.label.split('|').nth(1)?;

    if !label.starts_with("polish") {
        return Some(label.to_owned());
    }

    let file = mow_io::load_solution_file(&best.path).ok()?;
    let from = crate::meta::meta_field(&file, "polished_from")?;

    from.split('|').nth(1).map(str::to_owned)
}

/// Seconds since the epoch, so runs sort by time.
pub(crate) fn stamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());

    format!("run-{seconds}")
}
