//! Loading the bench instances written by `tools/golden/decompose/export_fixtures.py`.

use std::fs;
use std::path::PathBuf;

use mow_core::{BitGrid, Bounds, Cutter, Point, Problem};

pub fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

pub fn numbers(line: &str) -> Vec<i32> {
    line.split_whitespace().map(|n| n.parse().unwrap()).collect()
}

/// The file stems in `fixtures/<folder>`, sorted.
pub fn uids_in(folder: &str) -> Vec<String> {
    let mut uids: Vec<String> = fs::read_dir(fixtures().join(folder))
        .unwrap()
        .map(|e| {
            e.unwrap()
                .path()
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    uids.sort();

    uids
}

/// `k`, the cutter cells, then the region as row runs.
pub fn load_problem(uid: &str) -> Problem {
    let text = fs::read_to_string(fixtures().join("instances").join(format!("{uid}.txt"))).unwrap();
    let mut lines = text.lines();
    let mut header = |name: &str| {
        let line = lines.next().unwrap();
        line.strip_prefix(name).unwrap().trim().parse::<usize>().unwrap()
    };

    let k = header("k");
    let cutter_cells = header("cutter");
    let offsets: Vec<Point> = lines
        .by_ref()
        .take(cutter_cells)
        .map(|l| {
            let n = numbers(l);
            Point::new(n[0], n[1])
        })
        .collect();

    let run_count = lines
        .next()
        .unwrap()
        .strip_prefix("runs ")
        .unwrap()
        .parse()
        .unwrap();
    let runs: Vec<Vec<i32>> = lines.take(run_count).map(numbers).collect();

    let cutter = Cutter::new(BitGrid::from_cells(&offsets).unwrap().unwrap()).unwrap();
    Problem::new(uid, region(&runs), cutter, k).unwrap()
}

fn region(runs: &[Vec<i32>]) -> BitGrid {
    let bounds = Bounds {
        x_min: runs.iter().map(|r| r[1]).min().unwrap(),
        y_min: runs.first().unwrap()[0],
        x_max: runs.iter().map(|r| r[2]).max().unwrap(),
        y_max: runs.last().unwrap()[0],
    };

    let mut grid = BitGrid::empty(bounds).unwrap();
    for run in runs {
        grid.insert_run(run[0], run[1], run[2]);
    }

    grid
}
