//! `mow solve | mine | polish | turns | lns | compare` and the reports, see
//! `mow help`. With no arguments, `mow` opens the menus of `mow-tui`.

use std::process::ExitCode;

use mow_cli::compare;
use mow_cli::layout::{repo_root, DEFAULT_INSTANCES};
use mow_cli::lns_cmd;
use mow_cli::mine::{self, MineSummary};
use mow_cli::official::VerifierPool;
use mow_cli::polish_cmd;
use mow_cli::reports;
use mow_cli::solve_cmd;
use mow_cli::turns_cmd;

fn usage() -> String {
    [
        "mow: the Rust lawn mowing solver",
        "",
        "usage: mow                      the menus: score, instances, tours",
        solve_cmd::USAGE,
        mine::USAGE,
        polish_cmd::USAGE,
        turns_cmd::USAGE,
        lns_cmd::USAGE,
        compare::USAGE,
        reports::USAGE,
    ]
    .join("\n")
}

fn main() -> ExitCode {
    let mut raw = std::env::args().skip(1);
    let command = raw.next().unwrap_or_default();

    let result = match command.as_str() {
        "" => mow_tui::run(mow_tui::paths_under(&repo_root(), DEFAULT_INSTANCES)),
        mow_tui::SUPERVISE => match raw.next() {
            Some(dir) => mow_tui::supervise(std::path::Path::new(&dir)),
            None => Err("usage: mow __supervise <run folder>".to_owned()),
        },
        "solve" => solve_cmd::parse_args(raw).and_then(|args| solve_cmd::run(&args)),
        "mine" => mine::parse_args(raw).and_then(|options| run_mine(&options)),
        "polish" => polish_cmd::run(raw),
        "turns" => turns_cmd::run(raw),
        "lns" => lns_cmd::run(raw),
        "compare" => compare::parse_args(raw).and_then(|options| compare::run(&options)),
        other if reports::COMMANDS.contains(&other) => reports::run(other, raw),
        "help" | "-h" | "--help" => {
            println!("{}", usage());
            Ok(())
        }
        _ => Err(usage()),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run_mine(options: &mine::MineOptions) -> Result<(), String> {
    let pool = VerifierPool::start(&repo_root(), &options.instances, options.verifiers)
        .map_err(|e| e.to_string())?;
    let summary = mine::mine(options, &pool)?;

    print_mine(&summary);
    Ok(())
}

fn print_mine(s: &MineSummary) {
    println!(
        "{} jobs: {} skipped as done, {} run, {} failed, {} new bests, {} unstarted, {:.1} min",
        s.total,
        s.skipped,
        s.done,
        s.failed,
        s.bests,
        s.unstarted,
        s.seconds / 60.0
    );
}
