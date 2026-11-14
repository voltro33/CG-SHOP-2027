//! Paths shared by the golden tests: the fixtures and the repository data.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

pub fn read_fixture(name: &str) -> String {
    let text = std::fs::read_to_string(fixture(name)).unwrap();

    // git may check the fixtures out with CRLF
    text.replace("\r\n", "\n")
}

/// `<repo>/data/<rest>`.
pub fn data(rest: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../data")
        .join(rest)
}

pub fn instances() -> PathBuf {
    data("instances/challenge_v0/instances")
}

/// True when the data folder holds what the fixtures were exported from:
/// as many job records and stored bests as `inputs.txt` says.
pub fn data_matches_fixtures() -> bool {
    let inputs = read_fixture("inputs.txt");
    let wanted = |key: &str| -> usize {
        inputs
            .lines()
            .find_map(|line| line.strip_prefix(key)?.trim().parse().ok())
            .unwrap()
    };

    let records = mow_store::JobLog::read(&data("jobs.jsonl")).map_or(0, |r| r.len());
    let bests = std::fs::read_dir(data("solutions/best")).map_or(0, |d| d.count());

    let matches = records == wanted("records") && bests == wanted("bests");
    if !matches {
        println!("skipped: data/ holds {records} job records and {bests} bests, the fixtures other ones");
    }

    matches
}

/// Two JSON values equal up to a relative `1e-9` on floats.
pub fn assert_json_close(ours: &serde_json::Value, theirs: &serde_json::Value, at: &str) {
    use serde_json::Value;

    match (ours, theirs) {
        (Value::Number(a), Value::Number(b)) if a.is_f64() || b.is_f64() => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            assert!(
                (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0),
                "{at}: {a} != {b}"
            );
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{at}: lengths differ");
            for (index, (x, y)) in a.iter().zip(b).enumerate() {
                assert_json_close(x, y, &format!("{at}[{index}]"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            let keys = |m: &serde_json::Map<String, Value>| m.keys().cloned().collect::<Vec<_>>();
            assert_eq!(keys(a), keys(b), "{at}: keys differ");
            for (key, value) in a {
                assert_json_close(value, &b[key], &format!("{at}.{key}"));
            }
        }
        _ => assert_eq!(ours, theirs, "{at}"),
    }
}
