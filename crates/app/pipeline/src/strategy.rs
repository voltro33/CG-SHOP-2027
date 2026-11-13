//! The three ways `pipeline.solve` turns a problem into k routes.

use crate::config::SolveConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    /// One walk for everybody, split into k pieces (`_tours_single_walk`).
    SingleWalk,
    /// k routes straight from the segments (`_tours_grouped`).
    Grouped,
    /// One single-cutter problem per band, rebalanced over `rounds` rounds
    /// (`_tours_partitioned`).
    Partitioned { rounds: usize },
}

impl Strategy {
    /// The strategy Python picks for a config: group methods first (they
    /// ignore any partition), then a partition, else the single walk.
    /// Unknown methods are refused.
    pub fn of(config: &SolveConfig) -> Result<Self, String> {
        if !config.is_known_method() {
            return Err(format!("unknown method {:?}", config.method));
        }

        if config.is_grouped() {
            return Ok(Self::Grouped);
        }

        Ok(match config.partition {
            Some(_) => Self::Partitioned {
                rounds: config.rebalance_rounds.max(1),
            },
            None => Self::SingleWalk,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Partition;

    #[test]
    fn picks_what_python_picks() {
        let single = SolveConfig::new("segments-h");
        let banded = single.clone().with_partition(Partition::BandsY);
        let grouped = SolveConfig::new("groups-v").with_partition(Partition::BandsX);

        assert_eq!(Strategy::of(&single), Ok(Strategy::SingleWalk));
        assert_eq!(Strategy::of(&banded), Ok(Strategy::Partitioned { rounds: 4 }));
        assert_eq!(Strategy::of(&grouped), Ok(Strategy::Grouped));
        assert!(Strategy::of(&SolveConfig::new("cover-tsp")).is_err());
    }

    #[test]
    fn always_at_least_one_round() {
        let config = SolveConfig {
            rebalance_rounds: 0,
            ..SolveConfig::new("lanes-h").with_partition(Partition::BandsX)
        };

        assert_eq!(Strategy::of(&config), Ok(Strategy::Partitioned { rounds: 1 }));
    }
}
