//! Which configs win instances, what their wins are worth, and where.

use mow_store::JobRecord;

use super::ordered::Ordered;
use super::{config_of, points, size_band, Instances, TOP_PER_STRATUM};
use crate::family::family;

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigStats {
    pub config: String,
    pub jobs: usize,
    pub cpu_hours: f64,
    pub wins: usize,
    /// Wins nobody else ties.
    pub sole_wins: usize,
    /// The points lost on its sole wins if it had never run.
    pub points_at_stake: f64,
}

/// The configs that win most instances of one family and size band.
#[derive(Debug, Clone, PartialEq)]
pub struct Stratum {
    pub family: &'static str,
    pub band: &'static str,
    pub ranked: Vec<(String, usize)>,
}

/// The shortest length per instance and config, both in first seen order.
fn best_per_config(records: &[JobRecord]) -> Ordered<Ordered<u64>> {
    let mut best: Ordered<Ordered<u64>> = Ordered::default();

    for record in records {
        let Some(objective) = record.objective else {
            continue;
        };

        let mine = best.entry(&record.instance, Ordered::default);
        let value = mine.entry(&config_of(record), || objective);
        *value = (*value).min(objective);
    }

    best
}

#[derive(Debug, Clone, Default)]
struct Tally {
    jobs: usize,
    seconds: f64,
    wins: usize,
    sole_wins: usize,
    at_stake: f64,
}

/// Per config, most points at stake first, then most wins.
pub fn config_stats(records: &[JobRecord], instances: &Instances) -> Vec<ConfigStats> {
    let mut tally: Ordered<Tally> = Ordered::default();
    for record in records {
        let entry = tally.entry(&config_of(record), Tally::default);
        entry.jobs += 1;
        entry.seconds += record.seconds;
    }

    for (uid, by_config) in best_per_config(records).iter() {
        let mut ordered: Vec<u64> = by_config.iter().map(|(_, &v)| v).collect();
        ordered.sort_unstable();
        let shortest = ordered[0];

        let winners: Vec<&str> = by_config
            .iter()
            .filter(|(_, &value)| value == shortest)
            .map(|(config, _)| config)
            .collect();
        for config in &winners {
            tally.entry(config, Tally::default).wins += 1;
        }

        // only a sole winner costs anything when it is taken away
        if winners.len() > 1 || ordered.len() < 2 {
            continue;
        }

        let bound = instances.bound(uid);
        let sole = tally.entry(winners[0], Tally::default);
        sole.sole_wins += 1;
        sole.at_stake += points(bound, shortest) - points(bound, ordered[1]);
    }

    let mut stats: Vec<ConfigStats> = tally
        .iter()
        .map(|(config, t)| ConfigStats {
            config: config.to_owned(),
            jobs: t.jobs,
            cpu_hours: t.seconds / 3600.0,
            wins: t.wins,
            sole_wins: t.sole_wins,
            points_at_stake: t.at_stake,
        })
        .collect();
    stats.sort_by(|a, b| {
        b.points_at_stake
            .total_cmp(&a.points_at_stake)
            .then(b.wins.cmp(&a.wins))
    });

    stats
}

/// Per (family, size band) in sorted order, the configs that win most
/// instances there, at most [`TOP_PER_STRATUM`].
pub fn strata_winners(records: &[JobRecord], instances: &Instances) -> Vec<Stratum> {
    let mut counts: Vec<((&'static str, &'static str), Ordered<usize>)> = Vec::new();

    for (uid, by_config) in best_per_config(records).iter() {
        let shortest = by_config.iter().map(|(_, &v)| v).min().unwrap_or(0);
        let key = (family(uid), size_band(instances.cells(uid)));

        let position = match counts.iter().position(|(k, _)| *k == key) {
            Some(position) => position,
            None => {
                counts.push((key, Ordered::default()));
                counts.len() - 1
            }
        };

        for (config, _) in by_config.iter().filter(|(_, &value)| value == shortest) {
            *counts[position].1.entry(config, || 0) += 1;
        }
    }

    counts.sort_by(|a, b| a.0.cmp(&b.0));
    counts
        .into_iter()
        .map(|((family, band), configs)| {
            let mut ranked: Vec<(String, usize)> = configs.iter().map(|(c, &n)| (c.to_owned(), n)).collect();
            ranked.sort_by(|a, b| b.1.cmp(&a.1));
            ranked.truncate(TOP_PER_STRATUM);

            Stratum { family, band, ranked }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::tests::log;
    use super::*;

    #[test]
    fn points_at_stake_only_for_sole_winners() {
        let (records, instances) = log();
        let stats = config_stats(&records, &instances);
        let of = |name: &str| stats.iter().find(|s| s.config == name).unwrap().clone();

        let vertical = of("segments-v|s0");
        let gained = (50.0f64 / 80.0).powi(2) - (50.0f64 / 100.0).powi(2);
        assert_eq!((vertical.wins, vertical.sole_wins), (2, 1));
        assert!((vertical.points_at_stake - gained).abs() < 1e-12);
        assert_eq!(stats[0].config, "segments-v|s0");

        // a tie on b costs nothing to drop
        assert_eq!(of("segments-h|s0").wins, 1);
        assert_eq!(of("segments-h|s0").points_at_stake, 0.0);

        // a config that only failed never wins
        assert_eq!(of("lanes-h|s0").wins, 0);
    }

    #[test]
    fn strata_rank_configs_per_family_and_band() {
        let (records, instances) = log();
        let strata = strata_winners(&records, &instances);

        // a and b have no family suffix and fall in different bands
        assert_eq!(strata.len(), 2);
        assert_eq!(strata[0].band, "50k-400k");
        assert_eq!(
            strata[0].ranked,
            [("segments-h|s0".to_owned(), 1), ("segments-v|s0".to_owned(), 1)]
        );
        assert_eq!(strata[1].ranked, [("segments-v|s0".to_owned(), 1)]);
    }
}
