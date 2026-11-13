//! Tiny instances in fresh temporary folders, and the repository's python.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use mow_cli::official::VerifierPool;
use mow_verify::python_in;

pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// A new empty folder under the system temp folder.
pub fn temp_dir(name: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);

    let dir = std::env::temp_dir().join(format!("mow-cli-{name}-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    dir
}

/// An instance file: a polygon region, a square cutter of `side`, `k` cutters.
pub fn write_instance(dir: &Path, uid: &str, ring: &[(i32, i32)], side: i32, k: usize) {
    let xs: Vec<String> = ring.iter().map(|p| p.0.to_string()).collect();
    let ys: Vec<String> = ring.iter().map(|p| p.1.to_string()).collect();

    let json = format!(
        r#"{{"instance_uid":"{uid}",
        "region_to_cover":{{"outer_boundary":{{"x":[{}],"y":[{}]}},"inner_boundaries":[]}},
        "cutter":{{"x":[0,{side},{side},0],"y":[0,0,{side},{side}]}},"cutter_center":[0,0],
        "number_of_cutters":{k}}}"#,
        xs.join(","),
        ys.join(",")
    );

    fs::write(dir.join(format!("{uid}.instance.json")), json).unwrap();
}

/// Two small lawns: a rectangle for one cutter, an L for two.
pub fn two_instances(dir: &Path) {
    write_instance(dir, "rect", &[(0, 0), (14, 0), (14, 10), (0, 10)], 2, 1);
    write_instance(
        dir,
        "ell",
        &[(0, 0), (16, 0), (16, 6), (6, 6), (6, 14), (0, 14)],
        2,
        2,
    );
}

/// The official verifier, or `None` (and the test skips) without python.
pub fn verifier(instances: &Path) -> Option<VerifierPool> {
    let root = root();
    if !python_in(&root).is_file() {
        println!("skipped: no python in {}", root.display());
        return None;
    }

    Some(VerifierPool::start(&root, instances, 1).unwrap())
}
