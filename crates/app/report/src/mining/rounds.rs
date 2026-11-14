//! Per round: jobs, CPU hours, improvements and the points they bought.
//! Per size band: how many jobs ran into their time cap.

use std::collections::{BTreeMap, HashMap};

use mow_store::JobRecord;

use super::{points, size_band, Instances, CAP_SHARE, SIZE_BANDS};
use crate::family::median;

#[derive(Debug, Clone, PartialEq)]
pub struct RoundStats {
    pub round: i32,
    pub jobs: usize,
    pub cpu_hours: f64,
    pub improvements: usize,
    pub points: f64,
}

impl RoundStats {
    pub fn points_per_cpu_hour(&self) -> f64 {
        match self.cpu_hours != 0.0 {
            true => self.points / self.cpu_hours,
            false => 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Totals {
    jobs: usize,
    seconds: f64,
    improvements: usize,
    points: f64,
}

/// Walks the log in the order jobs finished. A job improves when it is
/// shorter than every earlier result for its instance; the first result of
/// an instance sets the start and buys nothing.
pub fn round_stats(records: &[JobRecord], instances: &Instances) -> Vec<RoundStats> {
    let mut running_best: HashMap<&str, u64> = HashMap::new();
    let mut per_round: BTreeMap<i32, Totals> = BTreeMap::new();

    for record in records {
        let totals = per_round.entry(record.round).or_default();
        totals.jobs += 1;
        totals.seconds += record.seconds;

        let Some(objective) = record.objective else {
            continue;
        };

        let uid = record.instance.as_str();
        let before = running_best.get(uid).copied();

        if let Some(before) = before.filter(|&b| objective < b) {
            let bound = instances.bound(uid);
            totals.improvements += 1;
            totals.points += points(bound, objective) - points(bound, before);
        }

        if before.map_or(true, |b| objective < b) {
            running_best.insert(uid, objective);
        }
    }

    per_round
        .into_iter()
        .map(|(round, totals)| RoundStats {
            round,
            jobs: totals.jobs,
            cpu_hours: totals.seconds / 3600.0,
            improvements: totals.improvements,
            points: totals.points,
        })
        .collect()
}

/// One size band's time caps.
#[derive(Debug, Clone, PartialEq)]
pub struct CapShare {
    pub band: &'static str,
    pub jobs: usize,
    pub capped_share: f64,
    /// The median run time as a share of the cap.
    pub median_used_share: f64,
}

/// Per size band with jobs, in band order.
pub fn time_cap_shares(
    records: &[JobRecord],
    instances: &Instances,
    time_limit: impl Fn(usize) -> f64,
) -> Vec<CapShare> {
    let mut capped: HashMap<&str, Vec<bool>> = HashMap::new();
    let mut used: HashMap<&str, Vec<f64>> = HashMap::new();

    for record in records {
        let size = instances.cells(&record.instance);
        let limit = time_limit(size);
        let band = size_band(size);

        capped
            .entry(band)
            .or_default()
            .push(record.seconds >= CAP_SHARE * limit);
        used.entry(band).or_default().push(record.seconds / limit);
    }

    SIZE_BANDS
        .iter()
        .filter_map(|&(_, _, band)| {
            let flags = capped.get(band)?;
            let hits = flags.iter().filter(|&&c| c).count();

            Some(CapShare {
                band,
                jobs: flags.len(),
                capped_share: hits as f64 / flags.len() as f64,
                median_used_share: median(used.get(band)?)?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::tests::log;
    use super::*;

    #[test]
    fn only_strict_improvements_count_and_the_first_result_is_free() {
        let (records, instances) = log();
        let rounds = round_stats(&records, &instances);

        assert_eq!(rounds[0].improvements, 0);
        assert_eq!(rounds[0].points, 0.0);

        // a goes from 100 to 80 against a bound of 50
        let gained = (50.0f64 / 80.0).powi(2) - (50.0f64 / 100.0).powi(2);
        assert_eq!(rounds[1].improvements, 1);
        assert!((rounds[1].points - gained).abs() < 1e-12);
        assert_eq!(rounds[1].jobs, 3);
        assert!((rounds[1].cpu_hours - 3.0).abs() < 1e-12);
    }

    #[test]
    fn caps_count_jobs_near_their_limit() {
        let (records, instances) = log();
        let caps = time_cap_shares(&records, &instances, |_| 3700.0);

        assert_eq!(caps.len(), 2);
        assert_eq!((caps[0].band, caps[0].jobs), ("<50k", 3));
        assert_eq!(caps[0].capped_share, 1.0);
        assert!((caps[1].median_used_share - 3600.0 / 3700.0).abs() < 1e-12);
    }
}
