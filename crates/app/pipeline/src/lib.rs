//! One instance, one config: plugins wired into a solution. Port of
//! `solver/pipeline.py`.
//!
//! This is the application core of the hexagon. It sees every step only
//! through the `mow-core` traits in [`Plugins`] and never names a concrete
//! plugin crate; `mow-cli` picks the plugins for a config and calls
//! [`solve`].
//!
//! # The three paths
//!
//! - [`Strategy::SingleWalk`]: decompose, order (closed for k = 1, open
//!   otherwise), walk, repair, improve the walk with half the time left,
//!   split into k, close each piece, improve the routes until the deadline,
//!   pad with idle one point routes up to k.
//! - [`Strategy::Grouped`]: decompose, assign k routes, repair them
//!   together, normalize, improve until the deadline, pad.
//! - [`Strategy::Partitioned`]: cut into bands, per band the single cutter
//!   walk (build, close, improve for its slice of time), rebalance the shares
//!   for up to `rounds` rounds, keep the round with the shortest longest
//!   route, improve all routes together until the deadline, pad.
//!
//! Time works as in Python: the `time_limit` passed to [`solve`] is the
//! job's time for improving; ordering and assigning get their own
//! [`Construction`] time on top (`time_limit` param, default 10 s, plus the
//! kick seconds, twice the kick seconds for groups).
//!
//! Links between segments bend horizontal first, except when every segment
//! is vertical: Python builds vertical lanes on the mirrored problem, so its
//! `lanes-v` and `segments-v` links bend vertical first, and the cells a
//! link sweeps by chance depend on that.
//!
//! # How `mow-cli` maps a config to plugins
//!
//! From `construct/__init__.py`, `jobs.py` and the constructor signatures on
//! `init-solver`. `{h,v}` is the lane axis; params not listed are ignored by
//! the Python method too.
//!
//! | method | decomposer | orderer (per segment count `n`) | strategy |
//! |---|---|---|---|
//! | `lanes-h`, `lanes-v` | `Lanes { axis }` | a snake: the lanes in list order, every second one reversed (no search; `lanes.py` builds the snake directly and takes no params) | single walk, or partitioned |
//! | `segments-h`, `segments-v` | `LaneSegments { axis, gap_factor (2), max_gap, offset, shift }` | `IteratedLocalSearch { kick_seconds: param or auto_kick_seconds(n), anneal: param or 0 }` | single walk, or partitioned |
//! | `segments-mixed` | `MixedLanes { tile_lanes (4), segment_cost (3), gap_factor (0), shift, flip_rounds (4) }` | as for segments | single walk, or partitioned |
//! | `groups-h`, `groups-v` | `LaneSegments { axis, gap_factor: param or 0, shift }`, note the default gap factor 0, not 2 | as for segments, without annealing | grouped, any partition ignored |
//!
//! The other steps are the same for every method:
//!
//! - splitter: `mow_assign::SplitWalk` (`split.split_walk`);
//! - assigner (groups only): `mow_assign::Groups` (`groups_horizontal` after
//!   the lane segments: one open order, cut into k, each group closed and
//!   reordered, rebalanced);
//! - partitioner (a `/bands-x` or `/bands-y` label only): `mow_assign::Bands`
//!   cutting along columns for `bands-x` and along rows for `bands-y`, with
//!   `rebalance: mow_assign::rebalanced`;
//! - repairer: `mow_improve::Detours` (`repair_walk`, `repair_tours`);
//! - improver: `mow_improve::CornerCut` (`improve.prune_routes`);
//! - observer: `NoObserver`, or `mow_viz::SvgObserver` to replay a run.
//!
//! [`Strategy::of`] picks the strategy from a config exactly as Python
//! does, and [`SolveConfig::seed`] is the seed every budget carries. The
//! official verifier runs in `mow-cli` after [`solve`]; here only the fast
//! [`mow_coverage::is_covered`] pre-check runs.

mod attempt;
mod config;
mod number;
mod partitioned;
mod paths;
mod plugins;
mod solve;
mod strategy;

#[cfg(test)]
mod solve_tests;
#[cfg(test)]
mod testing;

pub use attempt::Attempt;
pub use config::{LabelError, Partition, SolveConfig, DEFAULT_REBALANCE_ROUNDS, METHODS};
pub use number::format_g;
pub use plugins::{
    auto_kick_seconds, Construction, OrdererFactory, Partitioning, Plugins, Rebalance, CONSTRUCTION_SECONDS,
};
pub use solve::solve;
pub use strategy::Strategy;
