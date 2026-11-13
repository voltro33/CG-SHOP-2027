//! A real instance and its best stored solution, read through mow-io.
//! Skipped when the data folder is not there.

#![cfg(feature = "cli")]

mod support;

use std::path::PathBuf;

use mow_viz::Scene;
use support::{assert_well_formed, count};

const UID: &str = "smo10575_26k_2s3";

fn data() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data")
}

#[test]
fn draws_a_stored_best() {
    let instance = data().join(format!("instances/challenge_v0/instances/{UID}.instance.json"));
    let solution = data().join(format!("solutions/best/{UID}.solution.json"));
    if !instance.exists() || !solution.exists() {
        eprintln!("skipped: no data for {UID}");
        return;
    }

    let problem = mow_io::load_problem(&instance).unwrap();
    let solution = mow_io::load_solution(&solution).unwrap();
    let svg = Scene::new(&problem).routes(solution.routes()).to_svg();

    assert_well_formed(&svg);
    assert_eq!(count(&svg, "class=\"route\""), problem.k());
    assert!(svg.contains(&format!("objective {}", solution.objective())));
    assert!(svg.len() < 100_000, "{} bytes", svg.len());
}
