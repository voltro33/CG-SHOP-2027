//! One search per instance, no store, no verifier: how far it gets and what
//! each operator did. For tuning.
//!
//!     cargo run --release -p mow-lns --example probe -- <instances dir> <best dir> <seconds> uid [uid ...]

use std::path::Path;
use std::time::{Duration, Instant};

use mow_lns::{default_wheels, search, LnsOptions, Wheel};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [instances, bests, seconds, uids @ ..] = args.as_slice() else {
        return Err("usage: probe <instances dir> <best dir> <seconds> uid [uid ...]".into());
    };
    let seconds: f64 = seconds.parse()?;
    let temperature: f64 = std::env::var("LNS_T")
        .ok()
        .and_then(|t| t.parse().ok())
        .unwrap_or(0.0003);

    for uid in uids {
        let problem = mow_io::load_problem(&Path::new(instances).join(format!("{uid}.instance.json")))?;
        let best = mow_io::load_solution(&Path::new(bests).join(format!("{uid}.solution.json")))?;

        let deadline = Instant::now() + Duration::from_secs_f64(seconds);
        let options = LnsOptions {
            seed: 1,
            temperature,
            ..LnsOptions::default()
        };
        let outcome = search(&problem, best.routes(), deadline, &options, default_wheels())?;
        let covered = mow_coverage::is_covered(&problem, &outcome.best)?;

        let s = outcome.stats;
        println!(
            "{uid}: {} -> {} ({:+.3}%), {} moves, {} skipped, {} kept, {} bests, covered {covered}",
            s.start_objective,
            s.best_objective,
            100.0 * (s.best_objective as f64 / s.start_objective as f64 - 1.0),
            s.moves,
            s.skipped,
            s.kept,
            s.bests
        );
        print_wheel(&outcome.destroy);
        print_wheel(&outcome.recreate);

        // to check with the official verifier
        if let Ok(out) = std::env::var("LNS_OUT") {
            let solution = mow_core::Solution::new(uid.as_str(), outcome.best);
            mow_io::save_solution(&Path::new(&out).join(format!("{uid}.solution.json")), &solution)?;
        }
    }

    Ok(())
}

fn print_wheel(wheel: &Wheel) {
    for op in &wheel.operators {
        let t = op.tally;
        println!(
            "    {:<14} weight {:>6.2}  tries {:>7}  kept {:>6}  better {:>5}  best {:>4}",
            op.name, op.weight, t.tries, t.accepted, t.better, t.best
        );
    }
}
