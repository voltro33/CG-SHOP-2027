//! One configuration: which method, which knobs, which partition. Its label
//! is the job label of the Python miner, so results stay comparable.

use std::fmt;

use crate::number::format_g;

/// The methods the Python pipeline knows (`CONSTRUCTORS` plus
/// `GROUP_CONSTRUCTORS` in `construct/__init__.py`).
pub const METHODS: [&str; 7] = [
    "lanes-h",
    "lanes-v",
    "segments-h",
    "segments-v",
    "segments-mixed",
    "groups-h",
    "groups-v",
];

/// Rebalancing rounds of a partitioned run, as `SolveConfig` defaults.
pub const DEFAULT_REBALANCE_ROUNDS: usize = 4;

/// How a partitioned run cuts the region into bands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Partition {
    /// Vertical bands side by side, cut along columns.
    BandsX,
    /// Horizontal bands stacked bottom to top, cut along rows.
    BandsY,
}

impl Partition {
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "bands-x" => Some(Self::BandsX),
            "bands-y" => Some(Self::BandsY),
            _ => None,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::BandsX => "bands-x",
            Self::BandsY => "bands-y",
        }
    }
}

/// A label that does not read as `method/key=value.../partition`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelError {
    pub label: String,
    pub reason: String,
}

impl fmt::Display for LabelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bad label {:?}: {}", self.label, self.reason)
    }
}

impl std::error::Error for LabelError {}

/// Everything that decides how one instance is solved, so a result can
/// always be reproduced from its config. Params keep their given order,
/// since the label writes them in that order (`jobs.Job.label`).
#[derive(Debug, Clone, PartialEq)]
pub struct SolveConfig {
    pub method: String,
    pub params: Vec<(String, f64)>,
    pub partition: Option<Partition>,
    pub rebalance_rounds: usize,
}

impl SolveConfig {
    pub fn new(method: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            params: Vec::new(),
            partition: None,
            rebalance_rounds: DEFAULT_REBALANCE_ROUNDS,
        }
    }

    /// The same config with one more param at the end.
    pub fn with_param(self, key: impl Into<String>, value: f64) -> Self {
        let mut params = self.params;
        params.push((key.into(), value));

        Self { params, ..self }
    }

    pub fn with_partition(self, partition: Partition) -> Self {
        Self {
            partition: Some(partition),
            ..self
        }
    }

    /// Reads a job label such as `segments-h/gap_factor=0/shift=0.5/bands-x`.
    /// Any method name is accepted, so every label of the job log reads;
    /// [`SolveConfig::is_known_method`] tells whether the pipeline runs it.
    pub fn parse(label: &str) -> Result<Self, LabelError> {
        let bad = |reason: &str| LabelError {
            label: label.to_string(),
            reason: reason.to_string(),
        };

        let mut parts = label.split('/');
        let method = parts.next().unwrap_or_default();
        if method.is_empty() || method.contains('=') {
            return Err(bad("it must start with a method name"));
        }

        let mut config = Self::new(method);
        for part in parts {
            if config.partition.is_some() {
                return Err(bad("the partition must come last"));
            }

            if let Some((key, value)) = part.split_once('=') {
                let value: f64 = value.parse().map_err(|_| bad("a param value is not a number"))?;
                config.params.push((key.to_string(), value));
                continue;
            }

            let partition = Partition::parse(part).ok_or_else(|| bad("unknown partition"))?;
            config.partition = Some(partition);
        }

        Ok(config)
    }

    /// `method/key=value.../partition`, values in Python's `:g` format.
    pub fn label(&self) -> String {
        let mut parts = vec![self.method.clone()];

        for (key, value) in &self.params {
            parts.push(format!("{key}={}", format_g(*value)));
        }

        if let Some(partition) = self.partition {
            parts.push(partition.name().to_string());
        }

        parts.join("/")
    }

    /// The value of a param, the last one if it repeats.
    pub fn param(&self, name: &str) -> Option<f64> {
        self.params
            .iter()
            .rev()
            .find(|(key, _)| key == name)
            .map(|&(_, value)| value)
    }

    pub fn param_or(&self, name: &str, default: f64) -> f64 {
        self.param(name).unwrap_or(default)
    }

    /// The run seed: the `seed` param, else 0. Negative or fractional
    /// seeds are truncated toward zero and clamped at 0.
    pub fn seed(&self) -> u64 {
        self.param("seed").map_or(0, |seed| seed.max(0.0) as u64)
    }

    pub fn is_known_method(&self) -> bool {
        METHODS.contains(&self.method.as_str())
    }

    /// True for `groups-h` and `groups-v`, which build k routes directly.
    pub fn is_grouped(&self) -> bool {
        self.method.starts_with("groups-")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_writes_a_full_label() {
        let label = "segments-h/gap_factor=0/shift=0.5/bands-x";
        let config = SolveConfig::parse(label).unwrap();

        assert_eq!(config.method, "segments-h");
        assert_eq!(config.param("gap_factor"), Some(0.0));
        assert_eq!(config.param("shift"), Some(0.5));
        assert_eq!(config.partition, Some(Partition::BandsX));
        assert_eq!(config.label(), label);
    }

    #[test]
    fn keeps_the_given_param_order() {
        let label = "segments-v/seed=2/kick_seconds=30/anneal=0.003";
        let config = SolveConfig::parse(label).unwrap();

        assert_eq!(config.label(), label);
        assert_eq!(config.seed(), 2);
        assert_eq!(config.param_or("missing", 7.0), 7.0);
    }

    #[test]
    fn builds_the_same_label_as_python() {
        let config = SolveConfig::new("groups-h")
            .with_param("gap_factor", 0.0)
            .with_param("seed", 1.0);

        assert_eq!(config.label(), "groups-h/gap_factor=0/seed=1");
        assert!(config.is_grouped());
        assert!(config.is_known_method());
        assert_eq!(config.rebalance_rounds, DEFAULT_REBALANCE_ROUNDS);
    }

    #[test]
    fn bare_and_foreign_methods_read() {
        assert_eq!(
            SolveConfig::parse("lanes-h").unwrap(),
            SolveConfig::new("lanes-h")
        );
        assert!(!SolveConfig::parse("cover-tsp").unwrap().is_known_method());
    }

    #[test]
    fn rejects_malformed_labels() {
        for label in [
            "",
            "gap=1",
            "segments-h/bands-z",
            "segments-h/bands-x/gap=1",
            "segments-h/gap=x",
        ] {
            assert!(SolveConfig::parse(label).is_err(), "{label}");
        }
    }
}
