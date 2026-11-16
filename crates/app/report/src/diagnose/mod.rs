//! Where the tour length goes (`diagnose.py`), for any verified solution.
//!
//! The tours are walked one after the other, unit step by unit step. A step
//! from `p` to `q` brings `gain_e` fresh footprint cells, the cells of the
//! footprint at `q` that the footprint at `p` did not hold. The area bound
//! assumes every step brings `gain` new region cells. Each step's unit of
//! length is split by what its fresh cells are:
//!
//! - productive: new region cells, `new / gain`
//! - overshoot: cells outside the region, `outside / gain`
//! - near resweep: region cells this sweep took within the last cutter span
//! - far resweep: region cells swept longer ago
//! - direction: `(gain - gain_e) / gain`, a direction that brings fewer
//!   fresh cells than the best one
//! - dead: a step with no new region cell counts fully here instead
//!
//! The parts add up to the summed tour length. Waste that comes with every
//! lane (a straight run that sweeps a new cell) is split out: overshoot in
//! the first or last span of a lane, dead stretches shorter than a span
//! (turns), and dead stretches of four spans or more (travel). The lane
//! reference `(cells - k |C|) / (k lane_gain)` is not a proven bound, but
//! next to the area bound it shows how loose that bound is.
//!
//! What splitting the lawn between cutters costs is measured next to the
//! parts, in [`Split`]: [`Breakdown::to_full_json`] writes it as `split`,
//! [`Breakdown::to_json`] leaves it out and stays `diagnose.py`'s.

mod ends;
mod lanes;
mod split;
mod sweep;

use std::collections::HashSet;

use mow_core::Problem;

use crate::pyfmt::{percent, Json};

pub use ends::{LaneEnds, END_KINDS};
pub use split::{split_header, split_row, Split};
pub use sweep::diagnose;

/// The parts of a step, in the order they are reported.
pub const PARTS: [&str; 6] = [
    "productive",
    "overshoot",
    "near_resweep",
    "far_resweep",
    "direction",
    "dead",
];

#[derive(Debug, Clone, PartialEq)]
pub struct Breakdown {
    pub instance: String,
    pub family: &'static str,
    pub k: usize,
    pub cells: usize,
    pub gain: u32,
    pub lane_gain: u32,
    pub lower_bound: u64,
    pub lane_reference: u64,
    pub objective: u64,
    pub total_length: u64,
    pub new_cells: u64,
    pub start_cells: u64,
    /// The six [`PARTS`], in that order.
    pub parts: [f64; 6],
    pub overshoot_at_lane_ends: f64,
    pub dead_short: f64,
    pub dead_travel: f64,
    pub lanes: u64,
    pub dead_steps: u64,
    pub partial_steps: u64,
    pub full_steps: u64,
    pub split: Split,
}

impl Breakdown {
    pub fn productive(&self) -> f64 {
        self.parts[0]
    }

    /// The length lost against the area bound.
    pub fn lost(&self) -> f64 {
        self.total_length as f64 - self.productive()
    }

    /// Each part as a share of the summed tour length.
    pub fn shares(&self) -> [f64; 6] {
        self.parts.map(|part| part / self.total_length as f64)
    }

    /// The breakdown as `diagnose.py --json` writes it.
    pub fn to_json(&self) -> Json {
        let mut pairs = vec![
            ("instance", Json::str(&self.instance)),
            ("family", Json::str(self.family)),
            ("k", Json::int(self.k)),
            ("cells", Json::int(self.cells)),
            ("gain", Json::int(self.gain)),
            ("lane_gain", Json::int(self.lane_gain)),
            ("lower_bound", Json::int(self.lower_bound)),
            ("lane_reference", Json::int(self.lane_reference)),
            ("objective", Json::int(self.objective)),
            ("total_length", Json::int(self.total_length)),
            ("new_cells", Json::int(self.new_cells)),
            ("start_cells", Json::int(self.start_cells)),
        ];
        pairs.extend(
            PARTS
                .iter()
                .zip(self.parts)
                .map(|(&name, value)| (name, Json::Float(value))),
        );
        pairs.extend([
            ("overshoot_at_lane_ends", Json::Float(self.overshoot_at_lane_ends)),
            ("dead_short", Json::Float(self.dead_short)),
            ("dead_travel", Json::Float(self.dead_travel)),
            ("lanes", Json::int(self.lanes)),
            ("dead_steps", Json::int(self.dead_steps)),
            ("partial_steps", Json::int(self.partial_steps)),
            ("full_steps", Json::int(self.full_steps)),
        ]);

        Json::object(pairs)
    }

    /// [`Breakdown::to_json`] with the [`Split`] measures as `split`.
    pub fn to_full_json(&self) -> Json {
        match self.to_json() {
            Json::Object(mut pairs) => {
                pairs.push(("split".to_owned(), self.split.to_json()));
                Json::Object(pairs)
            }
            other => other,
        }
    }
}

/// The new cells per step of a long straight sweep: the rows the cutter
/// touches for a horizontal sweep, the columns for a vertical one, the better.
pub fn lane_gain(problem: &Problem) -> u32 {
    let offsets = problem.cutter().offsets();
    let rows: HashSet<i32> = offsets.iter().map(|o| o.y).collect();
    let columns: HashSet<i32> = offsets.iter().map(|o| o.x).collect();

    rows.len().max(columns.len()) as u32
}

/// `(cells - k |C|) / (k lane_gain)`, rounded up to even.
pub fn lane_reference(problem: &Problem) -> u64 {
    let k = problem.k() as u64;
    let footprint = problem.cutter().area() as u64;

    let remaining = (problem.num_cells() as u64).saturating_sub(k * footprint);
    if remaining == 0 {
        return 0;
    }

    let per_cutter = remaining.div_ceil(k * u64::from(lane_gain(problem)));
    per_cutter + per_cutter % 2
}

/// The header of the printed table.
pub fn header() -> String {
    let mut pieces = vec![
        format!("{:30}", "instance"),
        "   gap".to_owned(),
        "lane/".to_owned(),
    ];
    pieces.extend(
        PARTS
            .iter()
            .map(|part| format!("{:>6}", &part[..part.len().min(6)])),
    );

    pieces.join(" ")
}

/// One printed line: gap to the bound, lane reference over the bound, shares.
pub fn row(result: &Breakdown) -> String {
    let bound = result.lower_bound as f64;
    let (loose, gap) = match result.lower_bound {
        0 => (0.0, 0.0),
        _ => (
            result.lane_reference as f64 / bound,
            result.objective as f64 / bound - 1.0,
        ),
    };

    let mut pieces = vec![
        format!("{:30}", result.instance),
        percent(gap, 6, 1),
        format!("{loose:5.2}"),
    ];
    pieces.extend(result.shares().iter().map(|&share| percent(share, 6, 1)));

    pieces.join(" ")
}
