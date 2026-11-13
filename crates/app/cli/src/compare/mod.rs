//! `mow compare`: every Rust best against Python's, per instance, with a
//! summary; every Rust best checked again by the official verifier.

mod rows;
mod summary;
mod write;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use mow_store::{write_atomic, DirStore};

use crate::args::parse;
use crate::layout::{read_uid_list, DEFAULT_INSTANCES, DEFAULT_OUT, PYTHON_DATA};
use crate::official::{confirm, Official};

pub use rows::{Check, Row};
pub use summary::{groups, Group, Tally};
pub use write::Cpu;

pub const USAGE: &str = "usage: mow compare [--out data/rust] [--report docs/rust-results.md] \
                         [--instances DIR] [--python data] [--verifiers N] [--no-verify]";

#[derive(Debug, Clone)]
pub struct CompareOptions {
    pub out: PathBuf,
    pub report: PathBuf,
    pub instances: PathBuf,
    pub python: PathBuf,
    pub verifiers: usize,
    pub verify: bool,
}

pub fn parse_args(raw: impl Iterator<Item = String>) -> Result<CompareOptions, String> {
    let valued = ["--out", "--report", "--instances", "--python", "--verifiers"];
    let parsed = parse(raw, &valued, &["--no-verify"])?;
    if !parsed.positional.is_empty() {
        return Err(format!("unexpected {:?}\n{USAGE}", parsed.positional));
    }

    Ok(CompareOptions {
        out: PathBuf::from(parsed.value("--out").unwrap_or(DEFAULT_OUT)),
        report: PathBuf::from(parsed.value("--report").unwrap_or("docs/rust-results.md")),
        instances: PathBuf::from(parsed.value("--instances").unwrap_or(DEFAULT_INSTANCES)),
        python: PathBuf::from(parsed.value("--python").unwrap_or(PYTHON_DATA)),
        verifiers: parsed.number("--verifiers")?.unwrap_or(4),
        verify: !parsed.switch("--no-verify"),
    })
}

/// What a comparison found.
#[derive(Debug, Clone)]
pub struct Comparison {
    pub rows: Vec<Row>,
    pub groups: Vec<Group>,
    pub cpu: Cpu,
    pub failures: usize,
}

/// Builds the rows, verifies the Rust bests when `official` is given, and
/// writes the report and `<out>/compare.csv`.
pub fn compare(options: &CompareOptions, official: Option<&dyn Official>) -> Result<Comparison, String> {
    let bench_path = options.python.join("bench.txt");
    let bench: HashSet<String> = match bench_path.is_file() {
        true => read_uid_list(&bench_path)?.into_iter().collect(),
        false => HashSet::new(),
    };

    let store = DirStore::open(&options.out).map_err(|e| e.to_string())?;
    let sources = rows::Sources {
        instances: &options.instances,
        python: &options.python,
        rust: &store,
        bench: &bench,
    };
    let mut rows = rows::rows(&sources)?;

    if let Some(official) = official {
        verify_all(&mut rows, official, options.verifiers.max(1) * 2);
    }
    let failures = rows
        .iter()
        .filter(|r| matches!(r.check, Check::Failed(_)))
        .count();

    let (rust, _) = rows::cpu_seconds(&options.out.join("jobs.jsonl"))?;
    let (python_all, python_mined) = rows::cpu_seconds(&options.python.join("jobs.jsonl"))?;
    let cpu = Cpu {
        rust,
        python_all,
        python_mined,
    };

    save(&options.report, &write::markdown(&rows, cpu, failures))?;
    save(&options.out.join("compare.csv"), &write::csv(&rows))?;

    Ok(Comparison {
        groups: groups(&rows),
        rows,
        cpu,
        failures,
    })
}

fn save(path: &Path, text: &str) -> Result<(), String> {
    write_atomic(path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// Checks every Rust best on `threads` threads; failures are printed loudly.
fn verify_all(rows: &mut [Row], official: &dyn Official, threads: usize) {
    let next = AtomicUsize::new(0);
    let checks: Vec<std::sync::Mutex<Check>> = rows
        .iter()
        .map(|_| std::sync::Mutex::new(Check::NotRun))
        .collect();
    let shared: &[Row] = rows;

    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(row) = shared.get(index) else { break };
                let (Some(file), Some(objective)) = (&row.rust_file, row.rust) else {
                    continue;
                };

                let check = match confirm(official, file, objective) {
                    Ok(()) => Check::Passed,
                    Err(error) => {
                        eprintln!("!!! VERIFY FAILED {}: {error}", row.uid);
                        Check::Failed(error)
                    }
                };
                *checks[index].lock().unwrap_or_else(|p| p.into_inner()) = check;
            });
        }
    });

    for (row, check) in rows.iter_mut().zip(checks) {
        row.check = check.into_inner().unwrap_or_else(|p| p.into_inner());
        row.rust_file = None;
    }
}
