use std::collections::BTreeMap;
use std::path::Path;

use super::*;

/// Python's grid for the bench instances: the instances, then one
/// `(round, time_limit, job_id)` per job in run order.
fn python_grid() -> (Vec<Instance>, Vec<(u32, f64, String)>) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grid.txt");
    let text = std::fs::read_to_string(path).unwrap();
    let (mut instances, mut jobs) = (Vec::new(), Vec::new());

    for line in text.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();

        if words[0] == "instance" {
            instances.push(Instance {
                uid: words[1].to_owned(),
                cells: words[2].parse().unwrap(),
                k: words[3].parse().unwrap(),
            });
            continue;
        }

        jobs.push((
            words[0].parse().unwrap(),
            words[1].parse().unwrap(),
            words[2].to_owned(),
        ));
    }

    (instances, jobs)
}

#[test]
fn the_grid_equals_pythons_job_for_job() {
    let (instances, expected) = python_grid();
    assert_eq!(instances.len(), 31);

    let jobs = build_jobs(&instances, &PYTHON_ROUNDS);
    assert_eq!(jobs.len(), expected.len());

    for (job, (round, limit, id)) in jobs.iter().zip(&expected) {
        assert_eq!(job.job_id("2"), *id);
        assert_eq!(job.round, *round as i32, "{id}");
        assert_eq!(job.time_limit, *limit, "{id}");
    }
}

#[test]
fn counts_per_round_and_k_equal_pythons() {
    let (instances, expected) = python_grid();
    let jobs = build_jobs(&instances, &PYTHON_ROUNDS);

    let mut ours: BTreeMap<i32, usize> = BTreeMap::new();
    jobs.iter()
        .for_each(|job| *ours.entry(job.round).or_default() += 1);
    let mut theirs: BTreeMap<i32, usize> = BTreeMap::new();
    expected
        .iter()
        .for_each(|(round, _, _)| *theirs.entry(*round as i32).or_default() += 1);
    assert_eq!(ours, theirs);

    // the per instance counts of jobs.configs_for
    let single = [2, 2, 8, 6, 0, 6, 4, 4, 0, 0];
    let multi = [2, 6, 8, 6, 16, 6, 4, 6, 8, 10];
    for round in 0..10 {
        assert_eq!(
            configs_for(1, round).len(),
            single[round as usize],
            "k=1 round {round}"
        );
        assert_eq!(
            configs_for(3, round).len(),
            multi[round as usize],
            "k=3 round {round}"
        );
    }
}

#[test]
fn rounds_filter_keeps_the_round_order() {
    let instances = [
        Instance {
            uid: "small".into(),
            cells: 10,
            k: 2,
        },
        Instance {
            uid: "big".into(),
            cells: 2_000_000,
            k: 2,
        },
    ];

    let jobs = build_jobs(&instances, &[1, 0, 9]);
    let rounds: Vec<i32> = jobs.iter().map(|j| j.round).collect();

    assert_eq!(rounds.first(), Some(&0));
    assert_eq!(rounds.last(), Some(&9));
    assert_eq!(jobs[0].instance, "big");
    assert_eq!(jobs[0].time_limit, 600.0);

    let long = jobs.iter().find(|j| j.round == 9 && j.instance == "big").unwrap();
    assert_eq!(long.time_limit, 1200.0);
}

#[test]
fn version_tags_tell_kick_scales_apart() {
    assert_eq!(version_tag(1.0), "r1");
    assert_eq!(version_tag(0.25), "r1-k0.25");

    let job = &build_jobs(
        &[Instance {
            uid: "u".into(),
            cells: 5,
            k: 2,
        }],
        &[5],
    )[0];
    assert_eq!(job.job_id("r1"), "u|segments-h/seed=1/kick_seconds=30|s1|vr1");
}

#[test]
fn paying_rounds_are_mining_rounds() {
    assert!(PAYING_ROUNDS.iter().all(|round| ROUND_ORDER.contains(round)));
}

#[test]
fn the_hug_round_lays_flush_lanes_on_each_base_config() {
    let labels = |k| -> Vec<String> { configs_for(k, HUG_ROUND).iter().map(|c| c.label()).collect() };

    assert_eq!(
        labels(1),
        ["segments-h/gap_factor=0/hug=0", "segments-v/gap_factor=0/hug=0"]
    );
    assert_eq!(
        labels(3),
        ["groups-h/gap_factor=0/hug=0", "groups-v/gap_factor=0/hug=0"]
    );
}
