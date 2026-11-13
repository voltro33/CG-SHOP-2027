//! From a config to the concrete plugins of one run, exactly as the table in
//! the `mow-pipeline` crate docs maps the Python methods.
//!
//! A [`Kit`] owns the plugin values for one run and lends them to the
//! pipeline as [`Plugins`]. Params a Python constructor does not take are
//! refused, as Python's keyword call would refuse them.

use mow_assign::{rebalanced, Bands, Groups, SplitWalk};
use mow_core::{Assigner, Axis, Budget, Decomposer, Observer, Orderer, Problem, Route, Segment, SolveError};
use mow_decompose::{LaneSegments, Lanes, MixedLanes};
use mow_improve::{CornerCut, Detours};
use mow_order::{IteratedLocalSearch, Snake};
use mow_pipeline::{auto_kick_seconds, Partition, Partitioning, Plugins, SolveConfig, Strategy};

/// The params every ordering constructor takes (`walk_segments`).
const ORDER_PARAMS: [&str; 4] = ["kick_seconds", "seed", "anneal", "time_limit"];

const SEGMENT_PARAMS: [&str; 4] = ["max_gap", "offset", "gap_factor", "shift"];
const MIXED_PARAMS: [&str; 5] = ["tile_lanes", "segment_cost", "gap_factor", "shift", "flip_rounds"];
const GROUP_PARAMS: [&str; 5] = ["gap_factor", "shift", "kick_seconds", "seed", "time_limit"];

type Factory = dyn Fn(usize) -> Box<dyn Orderer> + Send + Sync;

/// The plugins of one run, owned.
pub struct Kit {
    strategy: Strategy,
    decomposer: Box<dyn Decomposer>,
    orderer: Box<Factory>,
    splitter: SplitWalk,
    assigner: Option<ScaledGroups>,
    partitioner: Option<Bands>,
    repairer: Detours,
    improver: CornerCut,
}

impl Kit {
    /// The plugins for `config`. `kick_scale` multiplies every kick time of
    /// the order search (1 is Python's time).
    pub fn new(config: &SolveConfig, kick_scale: f64) -> Result<Self, String> {
        if !(kick_scale.is_finite() && kick_scale > 0.0) {
            return Err(format!("the kick scale must be above 0, not {kick_scale}"));
        }

        let strategy = Strategy::of(config)?;
        check_params(config)?;

        let assigner = config.is_grouped().then(|| ScaledGroups {
            kick_seconds: config.param("kick_seconds"),
            scale: kick_scale,
        });
        let partitioner = match strategy {
            Strategy::Partitioned { .. } => config.partition.map(bands),
            _ => None,
        };

        Ok(Self {
            strategy,
            decomposer: decomposer(config)?,
            orderer: orderer(config, kick_scale),
            splitter: SplitWalk,
            assigner,
            partitioner,
            repairer: Detours,
            improver: CornerCut,
        })
    }

    pub fn strategy(&self) -> Strategy {
        self.strategy
    }

    /// The plugins lent to the pipeline for one run.
    pub fn plugins<'a>(&'a self, observer: &'a dyn Observer) -> Plugins<'a> {
        Plugins {
            decomposer: self.decomposer.as_ref(),
            orderer: &*self.orderer,
            splitter: &self.splitter,
            assigner: self.assigner.as_ref().map(|a| a as &dyn Assigner),
            partitioning: self.partitioner.as_ref().map(|partitioner| Partitioning {
                partitioner,
                rebalance: rebalanced,
            }),
            repairer: &self.repairer,
            improver: &self.improver,
            observer,
        }
    }

    pub fn decomposer_label(&self) -> String {
        self.decomposer.label()
    }
}

fn axis_of(method: &str) -> Axis {
    match method.ends_with("-v") {
        true => Axis::Vertical,
        false => Axis::Horizontal,
    }
}

/// `bands-x` cuts along columns, `bands-y` along rows.
fn bands(partition: Partition) -> Bands {
    match partition {
        Partition::BandsX => Bands { axis: Axis::Vertical },
        Partition::BandsY => Bands {
            axis: Axis::Horizontal,
        },
    }
}

fn allowed_params(method: &str) -> Vec<&'static str> {
    match method {
        "lanes-h" | "lanes-v" => Vec::new(),
        "segments-h" | "segments-v" => [&SEGMENT_PARAMS[..], &ORDER_PARAMS[..]].concat(),
        "segments-mixed" => [&MIXED_PARAMS[..], &ORDER_PARAMS[..]].concat(),
        _ => GROUP_PARAMS.to_vec(),
    }
}

fn check_params(config: &SolveConfig) -> Result<(), String> {
    let allowed = allowed_params(&config.method);

    for (key, value) in &config.params {
        if !allowed.contains(&key.as_str()) {
            return Err(format!("{} takes no param {key:?}", config.method));
        }
        if !value.is_finite() {
            return Err(format!("param {key} must be a number, not {value}"));
        }
    }

    Ok(())
}

fn decomposer(config: &SolveConfig) -> Result<Box<dyn Decomposer>, String> {
    let axis = axis_of(&config.method);
    let shift = config.param_or("shift", 0.0);

    let decomposer: Box<dyn Decomposer> = match config.method.as_str() {
        "lanes-h" | "lanes-v" => Box::new(Lanes::new(axis)),
        "segments-h" | "segments-v" => Box::new(LaneSegments {
            axis,
            gap_factor: config.param_or("gap_factor", 2.0),
            max_gap: whole(config, "max_gap")?,
            offset: whole(config, "offset")?.unwrap_or(0),
            shift,
        }),
        "segments-mixed" => Box::new(mixed(config)?),
        _ => Box::new(LaneSegments {
            gap_factor: config.param_or("gap_factor", 0.0),
            shift,
            ..LaneSegments::new(axis)
        }),
    };

    Ok(decomposer)
}

fn mixed(config: &SolveConfig) -> Result<MixedLanes, String> {
    let defaults = MixedLanes::new();
    let count = |name: &str, default: u32| -> Result<u32, String> {
        match whole(config, name)? {
            None => Ok(default),
            Some(value) => u32::try_from(value).map_err(|_| format!("{name} must not be negative")),
        }
    };

    Ok(MixedLanes {
        tile_lanes: count("tile_lanes", defaults.tile_lanes)?,
        segment_cost: config.param_or("segment_cost", defaults.segment_cost),
        gap_factor: config.param_or("gap_factor", defaults.gap_factor),
        shift: config.param_or("shift", defaults.shift),
        flip_rounds: count("flip_rounds", defaults.flip_rounds)?,
    })
}

/// An integer param; `2.0` reads as 2, `2.5` is refused.
fn whole(config: &SolveConfig, name: &str) -> Result<Option<i32>, String> {
    let Some(value) = config.param(name) else {
        return Ok(None);
    };

    let fits = value.fract() == 0.0 && value.abs() <= f64::from(i32::MAX);
    match fits {
        true => Ok(Some(value as i32)),
        false => Err(format!("{name} must be a whole number, not {value}")),
    }
}

/// Plain lanes snake; everything else searches, with the kick time of the
/// config (or the automatic one) times the scale. Groups never anneal, and
/// their own cap decides how much of the kick time each call gets.
fn orderer(config: &SolveConfig, scale: f64) -> Box<Factory> {
    if config.method.starts_with("lanes-") {
        return Box::new(|_| Box::new(Snake));
    }

    let kick = config.param("kick_seconds");
    let anneal = match config.is_grouped() {
        true => 0.0,
        false => config.param_or("anneal", 0.0),
    };

    Box::new(move |segments| {
        let kick_seconds = kick.unwrap_or_else(|| auto_kick_seconds(segments)) * scale;

        Box::new(IteratedLocalSearch {
            kick_seconds,
            anneal,
            ..IteratedLocalSearch::default()
        })
    })
}

/// [`Groups`] with its kick time scaled. The automatic kick time depends on
/// the segment count, so it is settled only when the segments are known.
#[derive(Debug, Clone, Copy)]
struct ScaledGroups {
    kick_seconds: Option<f64>,
    scale: f64,
}

impl ScaledGroups {
    fn groups(&self, segment_count: usize) -> Groups {
        let kick = self
            .kick_seconds
            .unwrap_or_else(|| auto_kick_seconds(segment_count));

        Groups {
            kick_seconds: Some(kick * self.scale),
        }
    }
}

impl Assigner for ScaledGroups {
    fn label(&self) -> String {
        Groups {
            kick_seconds: self.kick_seconds,
        }
        .label()
    }

    fn assign(
        &self,
        problem: &Problem,
        segments: &[Segment],
        orderer: &dyn Orderer,
        budget: Budget,
    ) -> Result<Vec<Route>, SolveError> {
        self.groups(segments.len())
            .assign(problem, segments, orderer, budget)
    }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
