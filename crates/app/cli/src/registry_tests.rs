use std::collections::BTreeSet;
use std::path::Path;

use mow_core::{Closure, NoObserver};
use mow_pipeline::SolveConfig;
use mow_store::JobLog;

use super::*;

/// Every label of Python's job grid for the bench instances, from the fixture.
fn grid_labels() -> BTreeSet<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grid.txt");
    let text = std::fs::read_to_string(path).unwrap();

    text.lines()
        .filter(|line| !line.starts_with("instance "))
        .map(|line| line.split('|').nth(1).unwrap().to_owned())
        .collect()
}

fn kit(label: &str) -> Result<Kit, String> {
    Kit::new(&SolveConfig::parse(label).unwrap(), 1.0)
}

#[test]
fn every_grid_label_maps() {
    let labels = grid_labels();
    assert_eq!(labels.len(), 72);

    for label in &labels {
        let kit = kit(label).unwrap_or_else(|e| panic!("{label}: {e}"));
        let plugins = kit.plugins(&NoObserver);

        match kit.strategy() {
            Strategy::Grouped => assert!(plugins.assigner.is_some(), "{label}"),
            Strategy::Partitioned { .. } => assert!(plugins.partitioning.is_some(), "{label}"),
            Strategy::SingleWalk => assert!(plugins.partitioning.is_none(), "{label}"),
        }
    }
}

/// Python's own log, when this machine has it: every label but the CP-SAT
/// polish and the retired `cover-tsp` maps.
#[test]
fn every_logged_label_maps() {
    let log = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data/jobs.jsonl");
    if !log.is_file() {
        println!("skipped: no {}", log.display());
        return;
    }

    let records = JobLog::read(&log).unwrap();
    let labels: BTreeSet<&str> = records.iter().map(|r| r.label.as_str()).collect();
    assert!(labels.len() > 50);

    for label in labels {
        if label.starts_with("polish/") || label == "cover-tsp" {
            continue;
        }
        kit(label).unwrap_or_else(|e| panic!("{label}: {e}"));
    }
}

#[test]
fn picks_the_strategy_and_decomposer_of_the_table() {
    let cases = [
        ("lanes-v", Strategy::SingleWalk, "lanes-v"),
        ("segments-h/bands-x", Strategy::Partitioned { rounds: 4 }, ""),
        ("groups-h/gap_factor=0/bands-y", Strategy::Grouped, ""),
        (
            "segments-mixed/gap_factor=0",
            Strategy::SingleWalk,
            "segments-mixed",
        ),
    ];

    for (label, strategy, decomposer) in cases {
        let kit = kit(label).unwrap();
        assert_eq!(kit.strategy(), strategy, "{label}");
        assert!(kit.decomposer_label().starts_with(decomposer), "{label}");
    }
}

#[test]
fn refuses_what_python_refuses() {
    for label in [
        "cover-tsp",
        "lanes-h/gap_factor=0",
        "segments-h/tile_lanes=2",
        "groups-h/anneal=0.1",
        "segments-mixed/tile_lanes=2.5",
    ] {
        assert!(kit(label).is_err(), "{label}");
    }

    let config = SolveConfig::parse("segments-h").unwrap();
    assert!(Kit::new(&config, 0.0).is_err());
    assert!(Kit::new(&config, f64::NAN).is_err());
}

#[test]
fn lanes_snake_and_segments_search() {
    let lanes = kit("lanes-h").unwrap();
    assert_eq!((lanes.orderer)(10).label(), "snake");

    let segments = kit("segments-h/seed=1/kick_seconds=30/anneal=0.003").unwrap();
    assert_eq!((segments.orderer)(10).label(), "ils/kick=30/anneal=0.003");

    let auto = kit("segments-v").unwrap();
    assert_eq!((auto.orderer)(100).label(), "ils/kick=4/anneal=0");

    let groups = kit("groups-v/gap_factor=0/seed=1").unwrap();
    assert_eq!((groups.orderer)(100).label(), "ils/kick=4/anneal=0");
}

#[test]
fn kick_scale_shrinks_every_kick() {
    let config = SolveConfig::parse("segments-h").unwrap();
    let scaled = Kit::new(&config, 0.25).unwrap();
    assert_eq!((scaled.orderer)(100).label(), "ils/kick=1/anneal=0");

    let groups = ScaledGroups {
        kick_seconds: None,
        scale: 0.5,
    };
    assert_eq!(groups.groups(100).kick_seconds, Some(2.0));
}

#[test]
fn snake_needs_no_time() {
    let segments: Vec<Segment> = (0..4).map(|y| Segment::horizontal(y, 0, 3).unwrap()).collect();
    let orderer = (kit("lanes-h").unwrap().orderer)(segments.len());
    let budget = Budget::new(std::time::Duration::ZERO, 0);

    let order = orderer.order(&segments, Closure::Closed, budget).unwrap();
    assert_eq!(order.len(), 4);
}
