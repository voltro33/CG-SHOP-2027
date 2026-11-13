//! Reading the fixtures of `tools/golden/assign/export_fixtures.py`.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use mow_core::Point;

pub fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// The bench instances, one fixture file each.
pub fn bench_uids() -> Vec<String> {
    let mut uids: Vec<String> = fs::read_dir(fixtures().join("walks"))
        .expect("the walks fixtures")
        .map(|entry| entry.unwrap().path())
        .filter_map(|path| Some(path.file_stem()?.to_str()?.to_owned()))
        .collect();

    uids.sort();
    uids
}

pub fn read(folder: &str, uid: &str) -> String {
    fs::read_to_string(fixtures().join(folder).join(format!("{uid}.txt"))).expect("fixture file")
}

pub fn numbers<T: std::str::FromStr>(text: &str) -> Vec<T>
where
    T::Err: std::fmt::Debug,
{
    text.split_whitespace().map(|n| n.parse().unwrap()).collect()
}

pub fn point(line: &str) -> Point {
    let n: Vec<i32> = numbers(line);
    Point::new(n[0], n[1])
}

/// Pairs `a b c d ...` as `(a, b), (c, d), ...`.
pub fn pairs<T: std::str::FromStr + Copy>(text: &str) -> Vec<(T, T)>
where
    T::Err: std::fmt::Debug,
{
    numbers::<T>(text).chunks(2).map(|c| (c[0], c[1])).collect()
}

/// The value after `key` on a line of `key value key value ...`.
pub fn field<'a>(line: &'a str, key: &str) -> &'a str {
    let words: Vec<&str> = line.split_whitespace().collect();
    let at = words.iter().position(|&w| w == key).expect("the key");
    words[at + 1]
}
