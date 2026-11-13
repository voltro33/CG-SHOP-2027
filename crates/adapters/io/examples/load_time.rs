//! Loads one instance and prints how long it took:
//! `cargo run --release -p mow-io --example load_time -- <path>`.
//!
//! With `MOW_HOLD` set it waits two seconds before exiting, so an outside
//! tool can read the process's peak memory.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

fn main() -> ExitCode {
    let Some(path) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: load_time <instance.json>");
        return ExitCode::FAILURE;
    };

    let started = Instant::now();
    match mow_io::load_problem(&path) {
        Ok(problem) => {
            let seconds = started.elapsed().as_secs_f64();
            println!(
                "{} cells={} load={seconds:.3}s",
                problem.uid(),
                problem.num_cells()
            );
            hold();
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}

fn hold() {
    if std::env::var_os("MOW_HOLD").is_some() {
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}
