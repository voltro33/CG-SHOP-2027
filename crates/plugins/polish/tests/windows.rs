//! Pieces, required cells and the CP-SAT model equal Python's for every
//! window of `tools/golden/polish/export_windows.py`: random sizes and
//! places, across the border of the tour, the whole tour, nothing, on nine
//! stored bests with one, two and three cutters. The model is compared in
//! the canonical form of `canonical.py`, through its hash.
//!
//! The hint of every model must also be a feasible solution of it, which
//! Python never checks.

mod common;

use std::fs;

use common::{
    canonical, cells_text, dense, first_broken, fixtures, fnv, hinted_values, problem, read_routes,
};
use mow_polish::{cut_pieces, prepare_window, required_cells, Piece, Window};

#[derive(Default)]
struct Counts {
    windows: usize,
    pieces: usize,
    required: usize,
    models: usize,
    hints: usize,
}

#[test]
fn windows_match_python() {
    let dir = fixtures().join("windows");
    let mut uids: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "txt"))
        .map(|p| p.file_stem().unwrap().to_string_lossy().into_owned())
        .collect();
    uids.sort();

    let mut counts = Counts::default();
    let mut failures = Vec::new();

    for uid in &uids {
        let Some(problem) = problem(uid) else {
            println!("{uid}: instance file missing, skipped");
            continue;
        };

        let tours = dense(&read_routes(&dir.join(format!("{uid}.tours"))));
        let text = fs::read_to_string(dir.join(format!("{uid}.txt"))).unwrap();

        for line in text.lines() {
            if let Err(why) = check(&problem, &tours, line, &mut counts) {
                failures.push(format!("{uid} {line:.60}: {why}"));
            }
        }
    }

    println!(
        "{} windows: {} with pieces, {} required cell sets, {} models, {} feasible hints",
        counts.windows, counts.pieces, counts.required, counts.models, counts.hints
    );
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

fn check(
    problem: &mow_core::Problem,
    tours: &[Vec<mow_core::Point>],
    line: &str,
    counts: &mut Counts,
) -> Result<(), String> {
    let parts: Vec<&str> = line.split('|').map(str::trim).collect();
    let head: Vec<i32> = common::numbers(parts[0]);
    let (index, window) = (head[0] as usize, Window::new(head[1], head[2], head[3], head[4]));
    counts.windows += 1;

    let pieces = cut_pieces(&tours[index], window);
    let text = match &pieces {
        None => "none".to_string(),
        Some(pieces) => pieces
            .iter()
            .map(|p| format!("{}:{}", p.first, p.last))
            .collect::<Vec<_>>()
            .join(" "),
    };
    if text != parts[1] {
        return Err(format!("pieces {text:.80}"));
    }

    let Some(pieces) = pieces.filter(|p| !p.is_empty()) else {
        return Ok(());
    };
    counts.pieces += 1;

    check_required(problem, tours, index, &pieces, window, parts[2])?;
    counts.required += 1;

    if parts[3] == "-" {
        return Ok(());
    }

    let prepared = prepare_window(problem, tours, index, window).ok_or("no model")?;
    let model = prepared.model.model();

    let shape = format!("{} {}", model.domains.len(), model.constraints.len());
    let found = format!("{shape} {}", fnv(&canonical(model)));
    if found != parts[3] {
        return Err(format!("model {found}, python {}", parts[3]));
    }
    counts.models += 1;

    if let Some(broken) = first_broken(model, &hinted_values(model)) {
        return Err(format!("the hint breaks constraint {broken}"));
    }
    counts.hints += 1;

    Ok(())
}

fn check_required(
    problem: &mow_core::Problem,
    tours: &[Vec<mow_core::Point>],
    index: usize,
    pieces: &[Piece],
    window: Window,
    expected: &str,
) -> Result<(), String> {
    let old: usize = pieces.iter().map(|p| p.steps(tours[index].len())).sum();
    let required = required_cells(problem, tours, index, pieces, window);

    let found = format!("{old} {} {}", required.len(), fnv(&cells_text(&required)));
    if found == expected {
        Ok(())
    } else {
        Err(format!("required {found}, python {expected}"))
    }
}
