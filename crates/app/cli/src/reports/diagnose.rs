//! `mow diagnose`: where the tour length of each stored best goes
//! (`python -m solver.diagnose`). The bench instances by default, every
//! stored best with `--all`. A second table follows for the instances with
//! more than one cutter: what splitting the lawn between them costs.

use std::path::{Path, PathBuf};

use mow_io::{load_problem, load_solution};
use mow_report::bests::read_bests;
use mow_report::diagnose::{diagnose, header, row, split_header, split_row, Breakdown};
use mow_report::pyfmt::Json;

use super::paths::{best_dir, save};
use crate::args::parse;
use crate::layout::{instance_path, read_uid_list, DEFAULT_INSTANCES, DEFAULT_OUT, PYTHON_DATA};

pub fn run(raw: impl Iterator<Item = String>) -> Result<(), String> {
    let valued = ["--data", "--instances", "--bench", "--only", "--json"];
    let parsed = parse(raw, &valued, &["--all"])?;
    if !parsed.positional.is_empty() {
        return Err(format!("unexpected {:?}\n{}", parsed.positional, super::USAGE));
    }
    if parsed.switch("--all") && parsed.value("--bench").is_some() {
        return Err("--all and --bench exclude each other".to_owned());
    }

    let data = PathBuf::from(parsed.value("--data").unwrap_or(DEFAULT_OUT));
    let instances = PathBuf::from(parsed.value("--instances").unwrap_or(DEFAULT_INSTANCES));
    let best = best_dir(&data);

    let uids = match parsed.switch("--all") {
        true => read_bests(&best)?.into_keys().collect(),
        false => {
            let default = Path::new(PYTHON_DATA).join("bench.txt");
            read_uid_list(&parsed.value("--bench").map_or(default, PathBuf::from))?
        }
    };
    let only = parsed.value("--only").unwrap_or("");
    let uids: Vec<String> = uids.into_iter().filter(|uid| uid.contains(only)).collect();

    let (results, failures) = walk(&uids, &instances, &best);
    print_split(&results);

    if let Some(path) = parsed.value("--json") {
        let list = Json::List(results.iter().map(Breakdown::to_full_json).collect());
        save(Path::new(path), &list.dump())?;
    }

    match failures {
        0 => Ok(()),
        n => Err(format!("{n} of {} solutions could not be diagnosed", uids.len())),
    }
}

/// Prints the table as it goes; a solution that fails is reported and skipped.
fn walk(uids: &[String], instances: &Path, best: &Path) -> (Vec<Breakdown>, usize) {
    println!("{}", header());

    let mut results = Vec::new();
    let mut failures = 0;
    for uid in uids {
        match one(uid, instances, best) {
            Ok(result) => {
                println!("{}", row(&result));
                results.push(result);
            }
            Err(error) => {
                eprintln!("{uid}: {error}");
                failures += 1;
            }
        }
    }

    (results, failures)
}

/// The second table, what splitting the lawn costs, for the instances with
/// more than one cutter.
fn print_split(results: &[Breakdown]) {
    let split: Vec<&Breakdown> = results.iter().filter(|result| result.k > 1).collect();
    if split.is_empty() {
        return;
    }

    println!();
    println!("{}", split_header());
    for result in split {
        println!("{}", split_row(result));
    }
}

fn one(uid: &str, instances: &Path, best: &Path) -> Result<Breakdown, String> {
    let problem = load_problem(&instance_path(instances, uid)).map_err(|e| e.to_string())?;

    let path = best.join(format!("{uid}.solution.json"));
    let solution = load_solution(&path).map_err(|e| format!("{}: {e}", path.display()))?;

    diagnose(&problem, &solution)
}
