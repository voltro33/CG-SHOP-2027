//! `window_map`, `exclusive_cells` and `summarize` equal Python's on every
//! stored best, for windows of 2, 4 and 8 cutter spans
//! (`tools/golden/polish/export_window_map.py`). The arrays are compared
//! through their hash, the summary numbers to the last bit.
//!
//! The fixture names the hash of each best's corners. A best that changed
//! since the export, or an instance file that is missing, is skipped and
//! counted. Debug builds take minutes over the largest instances, so they
//! go up to `DEBUG_CELLS`; `cargo test --release` takes all of them.

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::time::Instant;

use common::{fixtures, fnv, numbers, problem, root};
use mow_core::{Problem, Route};
use mow_polish::{cutter_span, summarize, window_map, WindowMap};

const DEBUG_CELLS: usize = 150_000;

fn corners_text(routes: &[Route]) -> String {
    let lines: Vec<String> = routes
        .iter()
        .map(|r| {
            let pairs: Vec<String> = r.corners().iter().map(|p| format!("{} {}", p.x, p.y)).collect();
            pairs.join(" ")
        })
        .collect();

    lines.join("\n") + "\n"
}

fn arrays_text(map: &WindowMap) -> String {
    let line = |values: &[u64]| values.iter().map(u64::to_string).collect::<Vec<_>>().join(" ");

    format!(
        "{}\n{}\n{}\n",
        line(&map.length),
        line(&map.need),
        line(&map.pieces)
    )
}

/// Whether the window just inside the worst 10 % has the room of the one
/// just outside.
fn tie_at_the_cut(map: &WindowMap) -> bool {
    let all = map.room();
    let mut room: Vec<f64> = (0..all.len())
        .filter(|&w| map.length[w] > 0)
        .map(|w| all[w])
        .collect();
    room.sort_by(|a, b| b.total_cmp(a));

    let count = ((0.10 * room.len() as f64).round_ties_even() as usize).max(1);
    count < room.len() && room[count - 1] == room[count]
}

/// Python's `repr` of a float reads back to the same bits.
fn same_bits(found: f64, python: &str) -> bool {
    found.to_bits() == python.parse::<f64>().unwrap().to_bits()
}

#[derive(Default)]
struct Tally {
    maps: usize,
    instances: usize,
    changed: Vec<String>,
    missing: usize,
    too_big: usize,
    pieces_ties: Vec<String>,
}

#[test]
fn window_maps_match_python() {
    let text = fs::read_to_string(fixtures().join("window_map.txt")).unwrap();

    let mut lines: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for line in text.lines() {
        lines
            .entry(line.split(' ').next().unwrap())
            .or_default()
            .push(line);
    }

    let started = Instant::now();
    let mut tally = Tally::default();
    for (uid, lines) in &lines {
        check_instance(uid, lines, &mut tally);
    }

    println!(
        "{} maps on {} instances as Python in {:.1?}; skipped: {} changed bests {:?}, {} missing, {} above {} cells in debug",
        tally.maps,
        tally.instances,
        started.elapsed(),
        tally.changed.len(),
        tally.changed,
        tally.missing,
        tally.too_big,
        DEBUG_CELLS
    );
    println!(
        "worst_10_pieces differs on {} maps (numpy's argsort breaks ties its own way): {:?}",
        tally.pieces_ties.len(),
        tally.pieces_ties
    );
    assert!(tally.maps > 0);
}

fn check_instance(uid: &str, lines: &[&str], tally: &mut Tally) {
    let Some(problem) = problem(uid) else {
        tally.missing += 1;
        return;
    };
    if cfg!(debug_assertions) && problem.num_cells() > DEBUG_CELLS {
        tally.too_big += 1;
        return;
    }

    let best = root().join(format!("data/solutions/best/{uid}.solution.json"));
    let Ok(solution) = mow_io::load_solution(&best) else {
        tally.missing += 1;
        return;
    };

    let tours_hash = lines[0].split(' ').nth(1).unwrap();
    if fnv(&corners_text(solution.routes())) != tours_hash {
        tally.changed.push(uid.to_string());
        return;
    }

    for line in lines {
        check_map(&problem, solution.routes(), line, tally);
    }
    tally.instances += 1;
}

fn check_map(problem: &Problem, routes: &[Route], line: &str, tally: &mut Tally) {
    let uid = problem.uid();
    let parts: Vec<&str> = line.split('|').map(str::trim).collect();
    let shape: Vec<&str> = parts[1].split(' ').collect();
    let head: Vec<i64> = numbers(&shape[..6].join(" "));
    let (spans, side) = (head[0] as u32, head[1] as i32);

    assert_eq!(side, spans as i32 * cutter_span(problem), "{uid} side");

    let map = window_map(problem, routes, side).unwrap();
    let found = format!(
        "{} {} {} {} {} {} {}",
        spans,
        side,
        map.gain,
        map.total_length,
        map.rows,
        map.columns,
        fnv(&arrays_text(&map))
    );
    assert_eq!(found, parts[1], "{uid} spans {spans}");

    let summary = summarize(problem, &map, spans);
    let python: Vec<&str> = parts[2].split(' ').collect();
    let exact = [
        ("room_share", summary.room_share, python[0]),
        ("worst_10_share", summary.worst_10_share, python[1]),
        ("worst_25_share", summary.worst_25_share, python[2]),
        ("worst_10_room", summary.worst_10_room, python[4]),
    ];
    for (name, value, expected) in exact {
        assert!(
            same_bits(value, expected),
            "{uid} spans {spans} {name}: {value} vs {expected}"
        );
    }

    // the worst windows are a set only up to ties at the cut, which numpy's
    // unstable argsort may break otherwise; then the median of their pieces may differ
    if !same_bits(summary.worst_10_pieces, python[3]) {
        assert!(
            tie_at_the_cut(&map),
            "{uid} spans {spans} worst_10_pieces without a tie"
        );

        let note = format!("{uid}/{spans}: {} vs {}", summary.worst_10_pieces, python[3]);
        tally.pieces_ties.push(note);
    }

    tally.maps += 1;
}
