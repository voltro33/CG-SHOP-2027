//! The steps of one run, borrowed as trait objects, and the time the
//! construction gets of its own.

use std::time::Duration;

use mow_core::{Assigner, Decomposer, Improver, Observer, Orderer, Partitioner, Repairer, Splitter};

use crate::config::SolveConfig;

/// Builds the orderer for a segment list of the given length. A factory
/// rather than one orderer, because the default kick time of the Python
/// order search grows with the segment count ([`auto_kick_seconds`]).
pub type OrdererFactory<'a> = dyn Fn(usize) -> Box<dyn Orderer> + 'a;

/// New band shares from the current ones and the band route lengths
/// (`partition._rebalanced`, `mow_assign::rebalanced`).
pub type Rebalance = fn(&[f64], &[u64]) -> Vec<f64>;

/// What a partitioned run needs: the bands and how to move work between them.
#[derive(Clone, Copy)]
pub struct Partitioning<'a> {
    pub partitioner: &'a dyn Partitioner,
    pub rebalance: Rebalance,
}

/// Every step of one run. The pipeline only ever sees these traits; which
/// concrete plugin sits behind each is the caller's choice (see the crate
/// docs for how `mow-cli` maps the Python methods).
#[derive(Clone, Copy)]
pub struct Plugins<'a> {
    pub decomposer: &'a dyn Decomposer,
    pub orderer: &'a OrdererFactory<'a>,
    pub splitter: &'a dyn Splitter,
    /// Needed by [`crate::Strategy::Grouped`] only.
    pub assigner: Option<&'a dyn Assigner>,
    /// Needed by [`crate::Strategy::Partitioned`] only.
    pub partitioning: Option<Partitioning<'a>>,
    pub repairer: &'a dyn Repairer,
    pub improver: &'a dyn Improver,
    pub observer: &'a dyn Observer,
}

/// Python's default `time_limit` knob of the constructors, in seconds.
pub const CONSTRUCTION_SECONDS: f64 = 10.0;

/// More segments need more kicks before the order settles
/// (`segments.auto_kick_seconds`): `min(60, 2 + n / 50)` seconds.
pub fn auto_kick_seconds(segment_count: usize) -> f64 {
    (2.0 + segment_count as f64 / 50.0).min(60.0)
}

/// The time construction gets, apart from the job's time limit: the
/// `time_limit` knob plus the kick time, as `walk_segments` and
/// `groups_horizontal` set their deadlines.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Construction {
    pub time_limit: f64,
    pub kick_seconds: Option<f64>,
}

impl Construction {
    /// The `time_limit` and `kick_seconds` params of a config.
    pub fn of(config: &SolveConfig) -> Self {
        Self {
            time_limit: config.param_or("time_limit", CONSTRUCTION_SECONDS),
            kick_seconds: config.param("kick_seconds"),
        }
    }

    pub fn kick_seconds(self, segment_count: usize) -> f64 {
        self.kick_seconds
            .unwrap_or_else(|| auto_kick_seconds(segment_count))
    }

    /// Ordering one walk: `time_limit + kick_seconds`.
    pub fn walk_time(self, segment_count: usize) -> Duration {
        seconds(self.time_limit + self.kick_seconds(segment_count))
    }

    /// Building groups orders twice: `time_limit + 2 * kick_seconds`.
    pub fn groups_time(self, segment_count: usize) -> Duration {
        seconds(self.time_limit + 2.0 * self.kick_seconds(segment_count))
    }
}

/// Longer than any run, short enough that a deadline never overflows.
pub(crate) const MAX_SECONDS: f64 = 365.0 * 24.0 * 3600.0;

/// Seconds as a duration; negative or NaN is zero, and anything above a
/// year is a year, so `Instant::now() + duration` cannot overflow.
pub(crate) fn seconds(value: f64) -> Duration {
    if value.is_nan() || value <= 0.0 {
        return Duration::ZERO;
    }

    Duration::from_secs_f64(value.min(MAX_SECONDS))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kick_time_grows_with_the_segments_up_to_a_minute() {
        assert_eq!(auto_kick_seconds(0), 2.0);
        assert_eq!(auto_kick_seconds(100), 4.0);
        assert_eq!(auto_kick_seconds(10_000), 60.0);
    }

    #[test]
    fn construction_time_follows_the_config() {
        let default = Construction::of(&SolveConfig::new("segments-h"));
        assert_eq!(default.walk_time(100), Duration::from_secs(14));
        assert_eq!(default.groups_time(100), Duration::from_secs(18));

        let config = SolveConfig::new("segments-h")
            .with_param("kick_seconds", 30.0)
            .with_param("time_limit", 1.0);
        let custom = Construction::of(&config);
        assert_eq!(custom.walk_time(5), Duration::from_secs(31));
    }

    #[test]
    fn odd_seconds_never_panic() {
        assert_eq!(seconds(-1.0), Duration::ZERO);
        assert_eq!(seconds(f64::NAN), Duration::ZERO);
        assert_eq!(seconds(f64::INFINITY), Duration::from_secs_f64(MAX_SECONDS));
    }
}
