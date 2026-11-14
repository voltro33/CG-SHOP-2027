//! `mow solve | mine | polish | compare` and the reports, see `mow help`.

use std::process::ExitCode;

use mow_cli::compare::{self, Comparison};
use mow_cli::layout::repo_root;
use mow_cli::mine::{self, MineSummary};
use mow_cli::official::{Official, VerifierPool};
use mow_cli::polish_cmd;
use mow_cli::reports;
use mow_cli::solve_cmd;

fn usage() -> String {
    [
        "mow: the Rust lawn mowing solver",
        "",
        solve_cmd::USAGE,
        mine::USAGE,
        polish_cmd::USAGE,
        compare::USAGE,
        reports::USAGE,
    ]
    .join("\n")
}

fn main() -> ExitCode {
    let mut raw = std::env::args().skip(1);
    let command = raw.next().unwrap_or_default();

    let result = match command.as_str() {
        "solve" => solve_cmd::parse_args(raw).and_then(|args| solve_cmd::run(&args)),
        "mine" => mine::parse_args(raw).and_then(|options| run_mine(&options)),
        "polish" => polish_cmd::run(raw),
        "compare" => compare::parse_args(raw).and_then(|options| run_compare(&options)),
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

fn run_compare(options: &compare::CompareOptions) -> Result<(), String> {
    let pool = match options.verify {
        true => Some(
            VerifierPool::start(&repo_root(), &options.instances, options.verifiers)
                .map_err(|e| e.to_string())?,
        ),
        false => None,
    };
    let official = pool.as_ref().map(|p| p as &dyn Official);

    let comparison = compare::compare(options, official)?;
    print_compare(&comparison);
    println!(
        "written {} and {}",
        options.report.display(),
        options.out.join("compare.csv").display()
    );

    match comparison.failures {
        0 => Ok(()),
        n => Err(format!("!!! {n} Rust bests FAILED the official verifier")),
    }
}

fn print_compare(c: &Comparison) {
    println!("group        n  solved  vs best W/T/L  geomean   vs mined W/T/L  geomean");

    for g in &c.groups {
        let geo = |t: &compare::Tally| t.geomean.map_or("n/a".to_owned(), |v| format!("{v:.4}"));
        println!(
            "{:<10} {:>3}  {:>6}  {:>4}/{:>3}/{:<4}  {:>7}   {:>4}/{:>3}/{:<4}  {:>7}",
            g.name,
            g.instances,
            g.solved,
            g.vs_best.wins,
            g.vs_best.ties,
            g.vs_best.losses,
            geo(&g.vs_best),
            g.vs_mined.wins,
            g.vs_mined.ties,
            g.vs_mined.losses,
            geo(&g.vs_mined)
        );
    }

    println!(
        "CPU hours: Rust {:.2}, Python {:.2} ({:.2} without polish); verifier failures: {}",
        c.cpu.rust / 3600.0,
        c.cpu.python_all / 3600.0,
        c.cpu.python_mined / 3600.0,
        c.failures
    );
}
