//! The plain lane snake drives the lanes exactly as `lanes.py` does: the
//! corners before the links equal Python's on three bench instances.
//!
//! The fixtures come from `tools/golden/cli/export_fixtures.py`; the
//! instances are read from `data/`, so the test skips without them.

mod common;

use std::fs;

use mow_core::{Axis, Closure, Decomposer, Orderer, Point};
use mow_decompose::Lanes;
use mow_order::Snake;

fn expected(uid: &str) -> Vec<(Axis, Vec<Point>)> {
    let path = common::root().join(format!("crates/app/cli/tests/fixtures/lanes/{uid}.txt"));
    let text = fs::read_to_string(path).unwrap();
    let mut blocks: Vec<(Axis, Vec<Point>)> = Vec::new();

    for line in text.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        if words[0] == "axis" {
            let axis = if words[1] == "h" {
                Axis::Horizontal
            } else {
                Axis::Vertical
            };
            blocks.push((axis, Vec::new()));
            continue;
        }

        let point = Point::new(words[0].parse().unwrap(), words[1].parse().unwrap());
        blocks.last_mut().unwrap().1.push(point);
    }

    blocks
}

#[test]
fn snake_corners_equal_pythons() {
    let dir = common::root().join("data/instances/challenge_v0/instances");
    let budget = mow_core::Budget::new(std::time::Duration::ZERO, 0);

    for uid in ["fjs3649_17k_1x7", "iso5009_26k_1x6", "dka1376_23k_1p7"] {
        let path = dir.join(format!("{uid}.instance.json"));
        if !path.is_file() {
            println!("skipped: no {}", path.display());
            return;
        }
        let problem = mow_io::load_problem(&path).unwrap();

        for (axis, corners) in expected(uid) {
            let segments = Lanes::new(axis).decompose(&problem).unwrap();
            let order = Snake.order(&segments, Closure::Closed, budget).unwrap();

            assert_eq!(order.waypoints(&segments), corners, "{uid} {axis:?}");
        }
    }
}
