//! `mow catalog`: one row of features per instance (`python -m
//! solver.catalog`). Instance folders only: Python also reads
//! `example_instances.zip`, which can be unpacked and passed as a folder.

use std::path::{Path, PathBuf};

use mow_report::catalog::{catalog, to_csv};

use super::paths::save;
use crate::args::{list, parse};
use crate::layout::{instance_files, DEFAULT_INSTANCES, DEFAULT_OUT};

pub fn run(raw: impl Iterator<Item = String>) -> Result<(), String> {
    let parsed = parse(raw, &["--instances", "--out", "--threads"], &[])?;
    if !parsed.positional.is_empty() {
        return Err(format!("unexpected {:?}\n{}", parsed.positional, super::USAGE));
    }

    let folders: Vec<String> = match parsed.value("--instances") {
        Some(text) => list("--instances", text)?,
        None => vec![DEFAULT_INSTANCES.to_owned()],
    };
    let out = parsed
        .value("--out")
        .map_or_else(|| Path::new(DEFAULT_OUT).join("catalog.csv"), PathBuf::from);
    let threads = parsed
        .number("--threads")?
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()));

    let mut files = Vec::new();
    for folder in &folders {
        files.extend(instance_files(Path::new(folder))?);
    }

    let rows = catalog(&files, threads)?;
    save(&out, &to_csv(&rows))?;
    println!("{} instances -> {}", rows.len(), out.display());

    Ok(())
}
