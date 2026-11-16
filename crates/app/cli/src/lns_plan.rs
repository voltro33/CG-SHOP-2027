//! The seconds of each `mow lns` job.
//!
//! - `--allocate even` (the default): `--seconds` each.
//! - `--allocate weighted`: the same total, shared by the points each
//!   instance is expected to gain per second (see [`crate::mine::allocate`]),
//!   from its stored best, its lower bound in the catalog and its LNS passes
//!   in `--history` (default `<out>/jobs.jsonl`; with `--pass N` only the
//!   passes before N count).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::args::Parsed;
use crate::grid::Instance;
use crate::mine::allocate::{gain_rate, passes, split, spread, weights, Allocation};
use crate::reports::paths::{read_catalog, read_log};

/// Instances listed by name when the allocation is printed.
const SHOWN: usize = 5;

/// What one instance knows for the weighted split.
struct Evidence {
    points: Option<f64>,
    rate: Option<f64>,
}

/// Seconds per instance, `instances` order, and prints how they spread.
/// `objectives` are the stored bests, `pass` the pass about to run (0 for
/// none), `mean` the `--seconds`.
pub fn seconds(
    parsed: &Parsed,
    out: &Path,
    pass: u32,
    instances: &[Instance],
    objectives: &HashMap<String, u64>,
    mean: f64,
) -> Result<Vec<f64>, String> {
    let allocation = Allocation::parse(parsed.value("--allocate").unwrap_or("even"))?;
    if allocation == Allocation::Even {
        println!("seconds per instance: {mean:.1} each (even)");
        return Ok(vec![mean; instances.len()]);
    }

    let history = parsed
        .value("--history")
        .map_or_else(|| out.join("jobs.jsonl"), PathBuf::from);
    let evidence = evidence(parsed, &history, pass, instances, objectives)?;

    let known: Vec<f64> = evidence.iter().filter_map(|e| e.points).collect();
    let typical = known.iter().sum::<f64>() / known.len().max(1) as f64;
    let points: Vec<f64> = evidence.iter().map(|e| e.points.unwrap_or(typical)).collect();
    let rates: Vec<Option<f64>> = evidence.iter().map(|e| e.rate).collect();
    let seconds = split(&weights(&points, &rates), mean);

    print!("{}", describe(instances, &seconds, &points, &rates, &history));
    Ok(seconds)
}

/// Points and gain rate per instance; the history must exist.
fn evidence(
    parsed: &Parsed,
    history: &Path,
    pass: u32,
    instances: &[Instance],
    objectives: &HashMap<String, u64>,
) -> Result<Vec<Evidence>, String> {
    if !history.is_file() {
        return Err(format!("--history: no job log at {}", history.display()));
    }

    let before = (pass > 0).then_some(pass);
    let passes = passes(&read_log(history)?, before);
    let bounds: HashMap<String, u64> = read_catalog(parsed.value("--catalog"))?
        .into_iter()
        .map(|f| (f.instance, f.lower_bound))
        .collect();

    Ok(instances
        .iter()
        .map(|instance| {
            let uid = &instance.uid;
            let points = match (bounds.get(uid), objectives.get(uid)) {
                (Some(&bound), Some(&objective)) if objective > 0 => {
                    Some((bound as f64 / objective as f64).powi(2))
                }
                _ => None,
            };
            let rate = passes.get(uid).and_then(|list| gain_rate(list));

            Evidence { points, rate }
        })
        .collect())
}

/// The spread of the split and the instances with the most seconds.
fn describe(
    instances: &[Instance],
    seconds: &[f64],
    points: &[f64],
    rates: &[Option<f64>],
    history: &Path,
) -> String {
    let (least, middle, most) = spread(seconds).unwrap_or_default();
    let total: f64 = seconds.iter().sum();
    let rated = rates.iter().flatten().count();
    let mut text = format!(
        "seconds per instance (weighted, {rated} of {} with a gain rate from {}): \
         min {least:.1}, median {middle:.1}, max {most:.1}, total {:.2} cpu h\n",
        instances.len(),
        history.display(),
        total / 3600.0
    );

    let mut order: Vec<usize> = (0..instances.len()).collect();
    order.sort_by(|&a, &b| seconds[b].total_cmp(&seconds[a]));
    text += "instance                              seconds  points  gain %/h\n";
    for &i in order.iter().take(SHOWN) {
        let rate = rates[i].map_or_else(|| "n/a".to_owned(), |r| format!("{:.3}", r * 3600.0 * 100.0));
        text += &format!(
            "{:<36} {:>8.1} {:>7.3} {rate:>9}\n",
            instances[i].uid, seconds[i], points[i]
        );
    }

    text
}
