//! `mow mine-report`: rounds, configs, strata and time caps of a job log
//! (`python -m solver.mine_report`).

use std::path::{Path, PathBuf};

use mow_report::mining::{report, text, to_json, Instances};

use super::paths::{read_catalog, read_log, save};
use crate::args::parse;
use crate::grid::time_limit_for;
use crate::layout::DEFAULT_OUT;

pub fn run(raw: impl Iterator<Item = String>) -> Result<(), String> {
    let parsed = parse(raw, &["--data", "--catalog", "--json"], &[])?;
    if !parsed.positional.is_empty() {
        return Err(format!("unexpected {:?}\n{}", parsed.positional, super::USAGE));
    }

    let data = PathBuf::from(parsed.value("--data").unwrap_or(DEFAULT_OUT));
    let instances = Instances::of(&read_catalog(parsed.value("--catalog"))?);

    // only challenge instances count, the log may also hold example runs
    let records = instances.records(read_log(&data.join("jobs.jsonl"))?);
    let result = report(&records, &instances, time_limit_for);

    print!("{}", text(&result));

    if let Some(path) = parsed.value("--json") {
        save(Path::new(path), &to_json(&result).dump())?;
    }

    Ok(())
}
