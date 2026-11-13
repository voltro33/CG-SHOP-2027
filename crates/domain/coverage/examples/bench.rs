//! Timing of the full and the incremental check on one stored best.
//!
//! ```text
//! cargo run --release -p mow-coverage --example bench [uid]
//! ```
//!
//! Compare with `make_fixtures.py --time <uid>`, the official Python check.

#[path = "../tests/common/load.rs"]
#[allow(dead_code)]
mod load;

use std::time::{Duration, Instant};

use mow_core::Point;
use mow_coverage::{uncovered_count, Scratch, VisitCounts};

const DEFAULT: &str = "euronight0070000_8749k_3s12";
const REPEATS: usize = 5;
const QUERIES: usize = 200_000;
const STRETCH: usize = 40;

fn best_of<T>(mut run: impl FnMut() -> T) -> (Duration, T) {
    let mut best = Duration::MAX;
    let mut last = None;

    for _ in 0..REPEATS {
        let start = Instant::now();
        last = Some(run());
        best = best.min(start.elapsed());
    }

    (best, last.expect("at least one run"))
}

fn main() {
    let uid = std::env::args().nth(1).unwrap_or_else(|| DEFAULT.to_string());
    let (Some(problem), Some(tours)) = (load::problem(&uid), load::tours(&load::best_path(&uid))) else {
        eprintln!("no instance or best for {uid}");
        return;
    };
    let routes = load::routes(&tours);
    println!(
        "{uid}: {} cells, cutter area {}",
        problem.num_cells(),
        problem.cutter().area()
    );

    let (full, missing) = best_of(|| uncovered_count(&problem, &routes).expect("fits"));
    println!(
        "full check: {:.1} ms (best of {REPEATS}), uncovered {missing}",
        ms(full)
    );

    let start = Instant::now();
    let counts = VisitCounts::from_routes(&problem, &routes).expect("fits");
    println!("visit counts built in {:.1} ms", ms(start.elapsed()));

    let positions: Vec<Point> = routes.iter().flat_map(|r| r.positions()).collect();
    let mut scratch = Scratch::for_counts(&counts);
    let mut seed = 0x9e37_79b9_7f4a_7c15_u64;

    let start = Instant::now();
    let mut kept = 0;
    for _ in 0..QUERIES {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;

        let from = seed as usize % positions.len();
        let len = 1 + (seed >> 32) as usize % STRETCH;
        let stretch = positions[from..(from + len).min(positions.len())].iter().copied();
        kept += usize::from(counts.still_covered_without(&mut scratch, stretch));
    }
    let elapsed = start.elapsed();

    println!(
        "{QUERIES} queries of 1..={STRETCH} positions: {:.0} queries/s ({kept} keep coverage)",
        QUERIES as f64 / elapsed.as_secs_f64()
    );
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}
