//! `splice` builds Python's tour from Python's answer. The fixtures of
//! `tools/golden/polish/export_splice.py` hold, per window, the grid arcs
//! CP-SAT took; a fake solver hands them back and `solve_window` must give
//! the same new tour (length and hash of its positions), never longer than
//! the old one and still covering the region.

mod common;

use std::collections::HashMap;
use std::fs;
use std::time::Duration;

use common::{cells_text, dense, fixtures, fnv, numbers, problem, read_routes};
use mow_core::{CpModel, CpParams, CpResponse, CpSolver, CpStatus, PortError, Route, Walk};
use mow_polish::{solve_window, Window};

/// Python's answer: the uses of the grid arcs, which are the first
/// variables of the model; every other variable at its lowest value.
struct Replay {
    status: CpStatus,
    objective: f64,
    uses: HashMap<usize, i64>,
}

impl CpSolver for Replay {
    fn solve(&self, model: &CpModel, _: &CpParams) -> Result<CpResponse, PortError> {
        let mut values: Vec<i64> = model.domains.iter().map(|d| d.lo).collect();
        for (&arc, &uses) in &self.uses {
            values[arc] = uses;
        }

        Ok(CpResponse {
            status: self.status,
            objective: self.objective,
            values,
        })
    }
}

fn status(name: &str) -> CpStatus {
    match name {
        "OPTIMAL" => CpStatus::Optimal,
        "FEASIBLE" => CpStatus::Feasible,
        other => panic!("unexpected status {other}"),
    }
}

fn replay(verdict: &str, arcs: &str) -> Replay {
    let (name, objective) = verdict.split_once(' ').unwrap();
    let uses = arcs
        .split_whitespace()
        .map(|pair| {
            let (arc, uses) = pair.split_once(':').unwrap();
            (arc.parse().unwrap(), uses.parse().unwrap())
        })
        .collect();

    Replay {
        status: status(name),
        objective: objective.parse().unwrap(),
        uses,
    }
}

#[test]
fn splice_matches_python() {
    let dir = fixtures().join("splice");
    let mut names: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "txt"))
        .map(|p| p.file_stem().unwrap().to_string_lossy().into_owned())
        .collect();
    names.sort();

    let (mut checked, mut shorter) = (0, 0);
    for name in &names {
        // the fixture name starts with the instance's short name
        let uid = instance_of(name);
        let Some(problem) = problem(&uid) else {
            println!("{uid}: instance file missing, skipped");
            continue;
        };

        let tours = dense(&read_routes(&dir.join(format!("{name}.tours"))));
        let text = fs::read_to_string(dir.join(format!("{name}.txt"))).unwrap();

        for line in text.lines() {
            let parts: Vec<&str> = line.split('|').map(str::trim).collect();
            let head: Vec<i32> = numbers(parts[0]);
            let (index, window) = (head[0] as usize, Window::new(head[1], head[2], head[3], head[4]));
            let solver = replay(parts[1], parts[2]);

            let result = solve_window(
                &problem,
                &tours,
                index,
                window,
                Duration::from_secs(1),
                1,
                &solver,
            )
            .unwrap()
            .unwrap();
            let positions = result.positions.clone().unwrap();

            let found = format!(
                "{} {} {} {}",
                result.old_steps,
                result.new_steps,
                positions.len(),
                fnv(&cells_text(&positions))
            );
            assert_eq!(found, parts[3], "{name} {}", parts[0]);
            assert!(positions.len() <= tours[index].len());

            let mut routes: Vec<Route> = Vec::new();
            for (i, tour) in tours.iter().enumerate() {
                let dense = if i == index { &positions } else { tour };
                routes.push(Walk::from_positions(dense).unwrap().closed().unwrap());
            }
            assert!(
                mow_coverage::is_covered(&problem, &routes).unwrap(),
                "{name} {}",
                parts[0]
            );

            checked += 1;
            shorter += usize::from(result.saved() > 0);
        }
    }

    println!("{checked} windows spliced as Python, {shorter} of them shorter");
}

fn instance_of(name: &str) -> String {
    let short = name.split('_').next().unwrap();
    let path = fixtures().join("windows");

    let found = fs::read_dir(path)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .find(|file| file.starts_with(&format!("{short}_")) && file.ends_with(".txt"));

    found.unwrap().trim_end_matches(".txt").to_string()
}
