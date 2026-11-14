//! Breakdowns of Python's stored bests equal `diagnose.py`'s: integers
//! exactly, floats to `1e-9` (numpy sums some parts pairwise), and the
//! printed table exactly.

mod common;

use common::{assert_json_close, data, instances, read_fixture};
use mow_io::{load_problem, load_solution};
use mow_report::diagnose::{diagnose, header, row};

#[test]
fn breakdowns_equal_python() {
    let expected: Vec<serde_json::Value> = serde_json::from_str(&read_fixture("diagnose.json")).unwrap();
    let printed = read_fixture("diagnose.txt");
    let mut lines = printed.lines();
    assert_eq!(lines.next().unwrap(), header());

    for (python, line) in expected.iter().zip(lines) {
        let uid = python["instance"].as_str().unwrap();
        let best = data(&format!("solutions/best/{uid}.solution.json"));
        let instance = instances().join(format!("{uid}.instance.json"));
        if !best.is_file() || !instance.is_file() {
            println!("skipped: {uid} has no instance or stored best here");
            return;
        }

        let problem = load_problem(&instance).unwrap();
        let result = diagnose(&problem, &load_solution(&best).unwrap()).unwrap();

        let ours: serde_json::Value = serde_json::from_str(&result.to_json().dump()).unwrap();
        assert_json_close(&ours, python, uid);
        assert_eq!(row(&result), line, "{uid}");
    }

    println!("{} breakdowns equal", expected.len());
}
