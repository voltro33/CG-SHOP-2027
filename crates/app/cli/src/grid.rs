//! The job grid for mining (port of `jobs.py`): which configs run on which
//! instance, in what order. Rounds run in [`ROUND_ORDER`]; every instance
//! finishes a round before the next round starts, and within a round the
//! biggest instances go first. See `jobs.py` for why each round exists.

use mow_pipeline::{format_g, Partition, SolveConfig};

/// The Rust job version: job ids never collide with Python's `v2`.
pub const CODE_VERSION: &str = "r1";

pub const ROUND_ORDER: [u32; 10] = [0, 1, 2, 8, 9, 7, 5, 6, 3, 4];

const GAP_FACTORS: [f64; 4] = [0.0, 1.0, 4.0, 8.0];
const SHIFTS: [f64; 3] = [0.25, 0.5, 0.75];
const SEEDS: [f64; 3] = [1.0, 2.0, 3.0];
const ANNEAL: f64 = 0.003;

const GROUP_GAP_FACTORS: [f64; 2] = [0.0, 1.0];
const GROUP_SEEDS: [f64; 2] = [0.0, 1.0];
const VARIANT_SEEDS: [f64; 2] = [2.0, 3.0];
const VARIANT_SHIFTS: [f64; 2] = [0.25, 0.5];
const VARIANT_GAP: f64 = 0.5;

/// Round 9 doubles the time cap above this many cells.
const LONG_CAP_CELLS: usize = 400_000;
const LONG_CAP_FACTOR: f64 = 2.0;

/// The mixed lane variants of round 7.
const MIXED_PARAMS: [&[(&str, f64)]; 4] = [
    &[("gap_factor", 0.0)],
    &[("gap_factor", 0.0), ("tile_lanes", 2.0), ("segment_cost", 4.0)],
    &[("tile_lanes", 16.0), ("segment_cost", 4.0)],
    &[("gap_factor", 2.0)],
];

const ORIENTATIONS: [&str; 2] = ["segments-h", "segments-v"];
const GROUP_METHODS: [&str; 2] = ["groups-h", "groups-v"];

/// What the grid needs to know of an instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instance {
    pub uid: String,
    pub cells: usize,
    pub k: usize,
}

/// One config on one instance.
#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    pub instance: String,
    pub cells: usize,
    pub round: u32,
    pub config: SolveConfig,
    /// The job's time for improving, seconds.
    pub time_limit: f64,
}

impl Job {
    pub fn label(&self) -> String {
        self.config.label()
    }

    /// `uid|label|s<seed>|v<version>`, as `jobs.Job.job_id`.
    pub fn job_id(&self, version: &str) -> String {
        format!(
            "{}|{}|s{}|v{version}",
            self.instance,
            self.label(),
            self.config.seed()
        )
    }
}

/// The version tag of a run: [`CODE_VERSION`], plus the kick scale when it
/// is not Python's, so runs with other kick times never skip each other.
pub fn version_tag(kick_scale: f64) -> String {
    match kick_scale == 1.0 {
        true => CODE_VERSION.to_owned(),
        false => format!("{CODE_VERSION}-k{}", format_g(kick_scale)),
    }
}

/// A wall clock cap per job, seconds (`jobs.time_limit_for`).
pub fn time_limit_for(cells: usize) -> f64 {
    match cells > 1_000_000 {
        true => 600.0,
        false => 120.0,
    }
}

fn config(method: &str, params: &[(&str, f64)], partition: Option<Partition>) -> SolveConfig {
    let config = params.iter().fold(SolveConfig::new(method), |c, &(key, value)| {
        c.with_param(key, value)
    });

    match partition {
        Some(partition) => config.with_partition(partition),
        None => config,
    }
}

/// The configs of one round for one instance (`jobs.configs_for`).
pub fn configs_for(k: usize, round: u32) -> Vec<SolveConfig> {
    let partitions: &[Partition] = match k > 1 {
        true => &[Partition::BandsX, Partition::BandsY],
        false => &[],
    };
    let mut configs = Vec::new();
    let mut add = |method: &str, params: &[(&str, f64)], partition: Option<Partition>| {
        configs.push(config(method, params, partition));
    };

    match round {
        0 => ORIENTATIONS.iter().for_each(|m| add(m, &[], None)),
        1 => {
            add("lanes-h", &[], None);
            add("lanes-v", &[], None);
            for method in ORIENTATIONS {
                partitions.iter().for_each(|&p| add(method, &[], Some(p)));
            }
        }
        2 => {
            for method in ORIENTATIONS {
                GAP_FACTORS
                    .iter()
                    .for_each(|&g| add(method, &[("gap_factor", g)], None));
            }
        }
        3 => {
            for method in ORIENTATIONS {
                SHIFTS.iter().for_each(|&s| add(method, &[("shift", s)], None));
            }
        }
        4 => cross(partitions, &mut add),
        5 => {
            for method in ORIENTATIONS {
                for seed in SEEDS {
                    add(method, &[("seed", seed), ("kick_seconds", 30.0)], None);
                }
            }
        }
        6 => {
            for method in ORIENTATIONS {
                for seed in [1.0, 2.0] {
                    add(
                        method,
                        &[("seed", seed), ("kick_seconds", 30.0), ("anneal", ANNEAL)],
                        None,
                    );
                }
            }
        }
        7 => {
            MIXED_PARAMS
                .iter()
                .for_each(|params| add("segments-mixed", params, None));
            for &partition in partitions {
                add("segments-mixed", MIXED_PARAMS[0], Some(partition));
            }
        }
        8 if k > 1 => groups(&mut add),
        9 if k > 1 => group_variants(&mut add),
        _ => {}
    }

    configs
}

type Add<'a> = dyn FnMut(&str, &[(&str, f64)], Option<Partition>) + 'a;

/// Round 4: partitions crossed with gap factors and shifts.
fn cross(partitions: &[Partition], add: &mut Add) {
    for method in ORIENTATIONS {
        for &partition in partitions {
            for gap in [0.0, 4.0] {
                for shift in [0.0, 0.5] {
                    add(method, &[("gap_factor", gap), ("shift", shift)], Some(partition));
                }
            }
        }
    }
}

/// Round 8: k closed tours straight from the lane segments.
fn groups(add: &mut Add) {
    for method in GROUP_METHODS {
        for gap in GROUP_GAP_FACTORS {
            for seed in GROUP_SEEDS {
                add(method, &[("gap_factor", gap), ("seed", seed)], None);
            }
        }
    }
}

/// Round 9: what round 8 left out.
fn group_variants(add: &mut Add) {
    for method in GROUP_METHODS {
        for seed in VARIANT_SEEDS {
            add(method, &[("gap_factor", 0.0), ("seed", seed)], None);
        }
        for shift in VARIANT_SHIFTS {
            add(method, &[("gap_factor", 0.0), ("shift", shift)], None);
        }
        add(method, &[("gap_factor", VARIANT_GAP)], None);
    }
}

/// Every job of `rounds` (in [`ROUND_ORDER`], others ignored) for every
/// instance, in run order: round by round, biggest instance first.
pub fn build_jobs(instances: &[Instance], rounds: &[u32]) -> Vec<Job> {
    let mut sorted: Vec<&Instance> = instances.iter().collect();
    sorted.sort_by(|a, b| b.cells.cmp(&a.cells).then_with(|| a.uid.cmp(&b.uid)));

    let mut jobs = Vec::new();
    for round in ROUND_ORDER.into_iter().filter(|r| rounds.contains(r)) {
        for instance in &sorted {
            let mut limit = time_limit_for(instance.cells);
            if round == 9 && instance.cells > LONG_CAP_CELLS {
                limit *= LONG_CAP_FACTOR;
            }

            for config in configs_for(instance.k, round) {
                jobs.push(Job {
                    instance: instance.uid.clone(),
                    cells: instance.cells,
                    round,
                    config,
                    time_limit: limit,
                });
            }
        }
    }

    jobs
}

#[cfg(test)]
#[path = "grid_tests.rs"]
mod tests;
