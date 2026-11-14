//! `mow lns`: the ruin and recreate search of `mow-lns` on the stored bests.
//!
//! - Without `--all`: the bench instances, `--seconds` each, logged to a
//!   fresh `<out>/bench/lns-<stamp>.jsonl`, scored against the bests from
//!   before with the points gained per CPU hour, plus what every operator did.
//! - With `--all`: every stored best up to `--max-cells`, through the
//!   resumable `<out>/jobs.jsonl` within `--hours`; `--pass N` runs again.
//!
//! Each job runs on one miner thread and continues from its instance's state
//! in `<out>/state`; every shorter result passes the official verifier first.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use mow_lns::LnsOptions;
use mow_report::bests::read_bests;

use crate::args::parse;
use crate::layout::repo_root;
use crate::mine::lns::{config, lns_jobs, OPERATOR_LOG, STATE_DIR};
use crate::mine::{read_instances, run_jobs};
use crate::official::VerifierPool;
use crate::polish_cmd::{options, score, wanted};
use crate::reports::bench_run::stamp;
use crate::reports::paths::{read_log, save};

pub const USAGE: &str =
    "usage: mow lns [--all [--hours H] [--max-cells N] [--pass N]] [--seconds S] [--temperature F] [--seed N] \
                      [--only TEXT] [--bench FILE] [--threads N] [--verifiers N] [--instances DIR] \
                      [--catalog FILE] [--out data/rust]";

/// Seconds per instance, bench or `--all`.
const DEFAULT_SECONDS: f64 = 60.0;

/// Threads unless `--threads` says otherwise: the machine is shared.
const DEFAULT_THREADS: usize = 3;

pub fn run(raw: impl Iterator<Item = String>) -> Result<(), String> {
    let valued = [
        "--hours",
        "--seconds",
        "--temperature",
        "--pass",
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
    let parsed = parse(raw, &valued, &["--all"])?;
    if !parsed.positional.is_empty() {
        return Err(format!("unexpected {:?}\n{USAGE}", parsed.positional));
    }

    let every = parsed.switch("--all");
    let mut options = options(&parsed, every)?;
    if parsed.value("--threads").is_none() {
        options.threads = DEFAULT_THREADS;
    }

    let temperature = parsed
        .number("--temperature")?
        .unwrap_or(LnsOptions::default().temperature);
    let config = config(
        temperature,
        parsed.number("--pass")?.unwrap_or(0),
        parsed.number("--seed")?.unwrap_or(0),
    );

    let before = read_bests(&options.out.join("best"))?;
    let uids = wanted(&parsed, every, &before.keys().cloned().collect::<Vec<_>>())?;
    if uids.is_empty() {
        return Err(format!(
            "no stored best to search from in {}",
            options.out.join("best").display()
        ));
    }

    let max_cells = parsed.number("--max-cells")?.unwrap_or(usize::MAX);
    let instances: Vec<_> = read_instances(&options.instances, Some(&uids))?
        .into_iter()
        .filter(|instance| instance.cells <= max_cells)
        .collect();
    let seconds = parsed.number("--seconds")?.unwrap_or(DEFAULT_SECONDS);
    let jobs = lns_jobs(&instances, &config, seconds);

    let log = match every {
        true => options.out.join("jobs.jsonl"),
        false => options.out.join("bench").join(format!("lns-{}.jsonl", stamp())),
    };
    let operator_log = options.out.join(STATE_DIR).join(OPERATOR_LOG);
    let logged_before = std::fs::metadata(&operator_log).map_or(0, |m| m.len());

    let verifier = VerifierPool::start(&repo_root(), &options.instances, options.verifiers)
        .map_err(|e| e.to_string())?;
    let summary = run_jobs(&options, jobs, &log, &verifier, None)?;
    println!(
        "{} lns jobs: {} run, {} failed, {} new bests, {:.1} min",
        summary.total,
        summary.done,
        summary.failed,
        summary.bests,
        summary.seconds / 60.0
    );

    let operators = operator_table(&operator_log, logged_before)?;
    print!("{operators}");
    if every {
        return Ok(());
    }

    let before: HashMap<String, u64> = before.into_iter().map(|(uid, b)| (uid, b.objective)).collect();
    let text = score(&parsed, &uids, &before, &read_log(&log)?, &config.label())? + &operators;
    print!("{text}");
    save(&log.with_extension("summary.txt"), &text)
}

/// Tries, kept, better and new bests per operator, summed over the lines
/// of the operator log past byte `from` (this run's).
fn operator_table(path: &Path, from: u64) -> Result<String, String> {
    let text = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let start = usize::try_from(from).unwrap_or(0).min(text.len());
    let tail = String::from_utf8_lossy(&text[start..]);

    let mut sums: BTreeMap<String, [u64; 4]> = BTreeMap::new();
    for line in tail.lines().filter(|l| !l.trim().is_empty()) {
        let value: serde_json::Value =
            serde_json::from_str(line).map_err(|e| format!("operator log: {e}"))?;
        for op in value["operators"].as_array().into_iter().flatten() {
            let name = op["name"].as_str().unwrap_or("?").to_owned();
            let sum = sums.entry(name).or_default();
            for (slot, key) in sum.iter_mut().zip(["tries", "kept", "better", "best"]) {
                *slot += op[key].as_u64().unwrap_or(0);
            }
        }
    }

    let mut out = String::from("operator          tries      kept   better   best\n");
    for (name, [tries, kept, better, best]) in sums {
        out += &format!("{name:<14} {tries:>8} {kept:>9} {better:>8} {best:>6}\n");
    }

    Ok(out)
}
