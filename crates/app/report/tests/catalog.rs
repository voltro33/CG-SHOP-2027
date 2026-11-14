//! Catalog rows of all 250 challenge instances equal `catalog.py`'s.

mod common;

use common::{instances, read_fixture};
use mow_report::catalog::{measure, Features, HEADER};

/// The columns `catalog.py` writes.
const PYTHON_COLUMNS: usize = 15;

/// Python's rows, header first, as lines.
fn python_rows() -> Vec<String> {
    read_fixture("catalog.csv").lines().map(str::to_owned).collect()
}

#[test]
fn every_instance_equals_python() {
    let rows = python_rows();
    // Python's columns come first; the bound columns are appended
    assert_eq!(rows[0], HEADER[..PYTHON_COLUMNS].join(","));

    let mut checked = 0;
    for line in &rows[1..] {
        let mut fields = line.split(',');
        let uid = fields.nth(1).unwrap();

        let path = instances().join(format!("{uid}.instance.json"));
        if !path.is_file() {
            println!("skipped: the challenge instances are not checked out");
            return;
        }

        let ours: Features = measure(&path).unwrap();
        assert_eq!(ours.fields()[..PYTHON_COLUMNS].join(","), *line);
        checked += 1;
    }

    println!("{checked} catalog rows equal");
    assert!(checked > 0);
}
