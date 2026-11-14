//! How long building a window model and a window map take, on the windows
//! and instances `tools/golden/polish/time_models.py` times in Python.
//!
//!     cargo run --release -p mow-polish --example timing [uid ...]
//!
//! Without uids it takes the three largest instances. Run from the repo root.

use std::path::Path;
use std::time::Instant;

use mow_polish::{cutter_span, dense_tours, longest, prepare_window, window_map, Window, DEFAULT_SIDES};

const LARGEST: [&str; 3] = [
    "euronight0070000_8749k_3s12",
    "octa2104_8486k_3l8",
    "uniform00900002_7283k_3p8",
];

const SIDE: i32 = 20;
const WINDOWS: usize = 10;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let uids: Vec<&str> = if args.is_empty() {
        LARGEST.to_vec()
    } else {
        args.iter().map(String::as_str).collect()
    };

    for uid in uids {
        let instance = format!("data/instances/challenge_v0/instances/{uid}.instance.json");
        let problem = mow_io::load_problem(Path::new(&instance))?;
        let solution = mow_io::load_solution(Path::new(&format!("data/solutions/best/{uid}.solution.json")))?;

        let tours = dense_tours(solution.routes());
        let index = longest(&tours);
        let positions = &tours[index];

        let started = Instant::now();
        let mut constraints = 0;
        for step in 0..WINDOWS {
            let p = positions[step * positions.len() / WINDOWS];
            let window = Window::new(p.x - SIDE / 2, p.y - SIDE / 2, p.x + SIDE / 2, p.y + SIDE / 2);

            if let Some(prepared) = prepare_window(&problem, &tours, index, window) {
                constraints += prepared.model.model().constraints.len();
            }
        }
        let per_window = started.elapsed().as_secs_f64() * 1000.0 / WINDOWS as f64;
        println!("{uid:32} {per_window:8.1} ms per window, {constraints} constraints");

        for spans in DEFAULT_SIDES {
            let side = spans as i32 * cutter_span(&problem);

            let started = Instant::now();
            window_map(&problem, solution.routes(), side)?;
            println!("{:32} window_map spans={spans} {:.2?}", "", started.elapsed());
        }
    }

    Ok(())
}
