//! What the job log says about the mining runs (`mine_report.py`), so the
//! next run spends its hours where bests come from.
//!
//! Points are pessimistic score, `(lower bound / length) ** 2` per instance.
//! Four questions:
//!
//! - [`round_stats`]: CPU hours per round, how often a job beat everything
//!   logged before it for its instance, and the points that bought
//! - [`config_stats`]: how many instances each config wins, and the points
//!   at stake, the score lost if it had never run and the next best stood
//! - [`strata_winners`]: which configs win per cutter family and size band
//! - [`time_cap_shares`]: how many jobs ran into their wall clock cap
//!
//! A config is a label plus its seed, since extra seeds are their own jobs.
//! Every map keeps Python's dict order, first seen first, so ties and float
//! sums come out the same.

mod configs;
mod ordered;
mod print;
mod rounds;

use std::collections::HashMap;

use mow_store::JobRecord;

use crate::catalog::Features;

pub use configs::{config_stats, strata_winners, ConfigStats, Stratum};
pub use print::{text, to_json};
pub use rounds::{round_stats, time_cap_shares, CapShare, RoundStats};

/// A job counts as capped when it ran this close to its time limit.
pub const CAP_SHARE: f64 = 0.95;

/// Size bands by region cells: `[low, high)` and a name.
pub const SIZE_BANDS: [(usize, usize, &str); 4] = [
    (0, 50_000, "<50k"),
    (50_000, 400_000, "50k-400k"),
    (400_000, 2_000_000, "400k-2M"),
    (2_000_000, usize::MAX, ">2M"),
];

/// How many configs each stratum lists.
pub const TOP_PER_STRATUM: usize = 3;

pub fn size_band(cells: usize) -> &'static str {
    SIZE_BANDS
        .iter()
        .find(|(low, high, _)| (*low..*high).contains(&cells))
        .map_or(SIZE_BANDS[3].2, |(_, _, name)| name)
}

/// `label|sN` from a job id `instance|label|sN|vM` or an older `instance|label|sN`.
pub fn config_of(record: &JobRecord) -> String {
    let mut parts = record.job_id.split('|').skip(1);
    let label = parts.next().unwrap_or("");
    let seed = parts.next().unwrap_or("");

    format!("{label}|{seed}")
}

/// What the report needs of the catalog: the challenge instances' bounds and sizes.
#[derive(Debug, Clone, Default)]
pub struct Instances {
    pub bounds: HashMap<String, u64>,
    pub cells: HashMap<String, usize>,
}

impl Instances {
    pub fn of(catalog: &[Features]) -> Self {
        let challenge = || catalog.iter().filter(|f| f.is_challenge());

        Self {
            bounds: challenge().map(|f| (f.instance.clone(), f.lower_bound)).collect(),
            cells: challenge().map(|f| (f.instance.clone(), f.cells)).collect(),
        }
    }

    /// Only challenge instances count; the log may also hold example runs.
    pub fn records(&self, records: Vec<JobRecord>) -> Vec<JobRecord> {
        records
            .into_iter()
            .filter(|r| self.bounds.contains_key(&r.instance))
            .collect()
    }

    fn bound(&self, uid: &str) -> u64 {
        self.bounds.get(uid).copied().unwrap_or(0)
    }

    fn cells(&self, uid: &str) -> usize {
        self.cells.get(uid).copied().unwrap_or(0)
    }
}

/// Everything the report found.
#[derive(Debug, Clone, PartialEq)]
pub struct MineReport {
    pub rounds: Vec<RoundStats>,
    pub configs: Vec<ConfigStats>,
    pub strata: Vec<Stratum>,
    pub caps: Vec<CapShare>,
}

/// The four tables over `records`, which must be challenge instances only.
/// `time_limit` is the miner's cap for an instance of so many cells.
pub fn report(records: &[JobRecord], instances: &Instances, time_limit: impl Fn(usize) -> f64) -> MineReport {
    MineReport {
        rounds: round_stats(records, instances),
        configs: config_stats(records, instances),
        strata: strata_winners(records, instances),
        caps: time_cap_shares(records, instances, time_limit),
    }
}

/// `(bound / objective) ** 2`, zero where Python has no score.
fn points(bound: u64, objective: u64) -> f64 {
    crate::family::pessimistic_score(bound, objective).unwrap_or(0.0)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn record(
        instance: &str,
        label: &str,
        round: i32,
        objective: Option<u64>,
        seed: u32,
    ) -> JobRecord {
        JobRecord {
            job_id: format!("{instance}|{label}|s{seed}|v2"),
            instance: instance.to_owned(),
            round,
            label: label.to_owned(),
            objective,
            seconds: 3600.0,
            error: None,
        }
    }

    /// Instance a: round 0 sets 100, round 1 improves to 80; instance b:
    /// both rounds tie at 100.
    pub(crate) fn log() -> (Vec<JobRecord>, Instances) {
        let records = vec![
            record("a", "segments-h", 0, Some(100), 0),
            record("b", "segments-h", 0, Some(100), 0),
            record("a", "segments-v", 1, Some(80), 0),
            record("b", "segments-v", 1, Some(100), 0),
            record("a", "lanes-h", 1, None, 0),
        ];
        let instances = Instances {
            bounds: HashMap::from([("a".to_owned(), 50), ("b".to_owned(), 50)]),
            cells: HashMap::from([("a".to_owned(), 10), ("b".to_owned(), 60_000)]),
        };

        (records, instances)
    }

    #[test]
    fn config_is_label_and_seed() {
        let r = record("a", "segments-h/gap_factor=0", 2, Some(1), 3);
        assert_eq!(config_of(&r), "segments-h/gap_factor=0|s3");

        let older = JobRecord {
            job_id: "a|segments-v|s0".to_owned(),
            ..r
        };
        assert_eq!(config_of(&older), "segments-v|s0");
    }

    #[test]
    fn size_bands_cover_every_size() {
        assert_eq!(size_band(10), "<50k");
        assert_eq!(size_band(50_000), "50k-400k");
        assert_eq!(size_band(usize::MAX), ">2M");
    }
}
