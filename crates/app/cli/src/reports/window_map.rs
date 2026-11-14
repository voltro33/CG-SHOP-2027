//! `mow window-map`: where the waste of each stored best sits, window by
//! window, and how much a perfect re-solve could save at most
//! (`python -m solver.window_map`). The bench instances by default, every
//! stored best with `--all`.

use std::path::{Path, PathBuf};

use mow_io::{load_problem, load_solution};
use mow_polish::{report_header, report_line, window_report, WindowSummary, DEFAULT_SIDES};
use mow_report::bests::read_bests;
use mow_report::pyfmt::Json;

use super::paths::{best_dir, save};
use crate::args::{list, parse};
use crate::layout::{instance_path, read_uid_list, DEFAULT_INSTANCES, DEFAULT_OUT, PYTHON_DATA};

pub fn run(raw: impl Iterator<Item = String>) -> Result<(), String> {
    let valued = ["--data", "--instances", "--bench", "--sides", "--only", "--json"];
    let parsed = parse(raw, &valued, &["--all"])?;
    if !parsed.positional.is_empty() {
        return Err(format!("unexpected {:?}\n{}", parsed.positional, super::USAGE));
    }

    let data = PathBuf::from(parsed.value("--data").unwrap_or(DEFAULT_OUT));
    let instances = PathBuf::from(parsed.value("--instances").unwrap_or(DEFAULT_INSTANCES));
    let best = best_dir(&data);
    let sides: Vec<u32> = match parsed.value("--sides") {
        Some(text) => list("--sides", text)?,
        None => DEFAULT_SIDES.to_vec(),
    };

    let uids = match parsed.switch("--all") {
        true => read_bests(&best)?.into_keys().collect(),
        false => {
            let default = Path::new(PYTHON_DATA).join("bench.txt");
            read_uid_list(&parsed.value("--bench").map_or(default, PathBuf::from))?
        }
    };
    let only = parsed.value("--only").unwrap_or("");

    println!("{}", report_header());
    let mut results = Vec::new();
    for uid in uids.iter().filter(|uid| uid.contains(only)) {
        for summary in one(uid, &instances, &best, &sides)? {
            println!("{}", report_line(&summary));
            results.push(summary);
        }
    }

    if let Some(path) = parsed.value("--json") {
        let list = Json::List(results.iter().map(to_json).collect());
        save(Path::new(path), &list.dump())?;
    }

    Ok(())
}

fn one(uid: &str, instances: &Path, best: &Path, sides: &[u32]) -> Result<Vec<WindowSummary>, String> {
    let problem = load_problem(&instance_path(instances, uid)).map_err(|e| e.to_string())?;

    let path = best.join(format!("{uid}.solution.json"));
    let solution = load_solution(&path).map_err(|e| format!("{}: {e}", path.display()))?;

    window_report(&problem, solution.routes(), sides).map_err(|e| format!("{uid}: {e}"))
}

/// The fields of `asdict(WindowSummary)`, in order.
fn to_json(summary: &WindowSummary) -> Json {
    Json::object(vec![
        ("instance", Json::str(summary.instance.clone())),
        ("family", Json::str(summary.family)),
        ("side_spans", Json::int(summary.side_spans)),
        ("side", Json::int(summary.side)),
        ("windows", Json::int(summary.windows as i64)),
        ("room_share", Json::float(Some(summary.room_share))),
        ("worst_10_share", Json::float(Some(summary.worst_10_share))),
        ("worst_25_share", Json::float(Some(summary.worst_25_share))),
        ("worst_10_pieces", Json::float(Some(summary.worst_10_pieces))),
        ("worst_10_room", Json::float(Some(summary.worst_10_room))),
    ])
}
