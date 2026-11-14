//! The reporting commands, thin wrappers over the `mow-report` crate:
//! `mow report | mine-report | diagnose | catalog | bench | window-map`.
//!
//! Each reads either data folder: Python's `data/` (bests in
//! `solutions/best/`) or Rust's `data/rust/` (bests in `best/`), picked with
//! `--data`. Files go under `data/rust/` or to stdout; Python's own files
//! are never written.

mod bench;
pub(crate) mod bench_run;
mod catalog;
mod diagnose;
mod leaderboard;
mod mine_report;
pub(crate) mod paths;
mod window_map;

/// The commands this module answers.
pub const COMMANDS: [&str; 6] = [
    "report",
    "mine-report",
    "diagnose",
    "catalog",
    "bench",
    "window-map",
];

/// One usage line per command.
pub const USAGE: &str = concat!(
    "usage: mow report [--data data/rust] [--catalog FILE] [--out FILE]\n",
    "usage: mow mine-report [--data data/rust] [--catalog FILE] [--json FILE]\n",
    "usage: mow diagnose [--data data/rust] [--instances DIR] [--bench data/bench.txt | --all] ",
    "[--only TEXT] [--json FILE]\n",
    "usage: mow window-map [--data data/rust] [--instances DIR] [--bench data/bench.txt | --all] ",
    "[--sides 2,4,8] [--only TEXT] [--json FILE]
",
    "usage: mow catalog [--instances DIR,DIR] [--out data/rust/catalog.csv] [--threads N]\n",
    "usage: mow bench pick [--catalog FILE] [--leaderboard FILE] [--out data/rust/bench.txt]\n",
    "usage: mow bench score [--log data/rust/jobs.jsonl] [--before data/solutions/best] ",
    "[--bench data/bench.txt] [--catalog FILE] [--only TEXT]
",
    "usage: mow bench run <config | best> [--seconds S] [--only TEXT] [--bench data/bench.txt] ",
    "[--threads N] [--kick-scale F] [--verifiers N] [--instances DIR] [--catalog FILE] [--out data/rust]",
);

/// Runs `command`, one of [`COMMANDS`], on the rest of the command line.
pub fn run(command: &str, raw: impl Iterator<Item = String>) -> Result<(), String> {
    match command {
        "report" => leaderboard::run(raw),
        "mine-report" => mine_report::run(raw),
        "diagnose" => diagnose::run(raw),
        "catalog" => catalog::run(raw),
        "bench" => bench::run(raw),
        "window-map" => window_map::run(raw),
        _ => Err(USAGE.to_owned()),
    }
}
