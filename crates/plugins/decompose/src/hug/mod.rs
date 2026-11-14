//! Hugged lanes: the lane grid moved per strip of columns, so the first and
//! last lanes of each part of the region sit flush with its edges.
//!
//! One global lane grid straddles the region's upper and lower edges
//! wherever they do not fall on a lane border, and pays for it on every
//! lane. Here the region is cut into vertical strips of `strip_lanes` lane
//! heights, and every strip picks a residue (the lane heights modulo the
//! lane height). Picking per strip on its own would pay for the lane ends
//! at every strip border, so the residues are chosen jointly
//! ([`choose::residues`]): keeping a residue across a border earns what
//! running the lanes on saves. Every run of strips with one residue is then
//! cut into lane segments like [`crate::LaneSegments`] does, on its own
//! columns only, so together the runs sweep every cell.

mod choose;
mod costs;

use mow_core::{Axis, Decomposer, Problem, Segment, SolveError};

use crate::axis::{along, segment, suffix};
use crate::rows::{Band, RowRuns};
use crate::segments::{cut_lanes, truncated, Span};

use costs::{costs, Pricing, Strips};

const STEP: &str = "hug";

/// Lane segments with a lane offset per strip of columns, chosen jointly.
///
/// `strip_lanes` is the strip width in lane heights; 0 makes the whole
/// region one strip, which just picks its best offset. Each segment is
/// priced at its length plus `segment_cost` lane heights. Gaps up to
/// `gap_factor` lane heights are bridged, as in [`crate::LaneSegments`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HuggedLanes {
    pub axis: Axis,
    pub strip_lanes: u32,
    pub segment_cost: f64,
    pub gap_factor: f64,
}

impl HuggedLanes {
    fn horizontal(&self, problem: &Problem) -> Result<Vec<Segment>, SolveError> {
        if !(self.segment_cost.is_finite() && self.segment_cost >= 0.0) {
            return Err(SolveError::Unsupported {
                step: STEP,
                reason: format!(
                    "segment cost {} must be a number of at least 0",
                    self.segment_cost
                ),
            });
        }

        let band = Band::of(problem.cutter());
        let max_gap = truncated(STEP, self.gap_factor, band.height)?;
        let rows = RowRuns::new(problem.region());
        let (Some((x0, x1)), Some((lowest, _))) = (rows.x_span(), rows.span()) else {
            return Ok(Vec::new());
        };

        let width = match self.strip_lanes {
            0 => x1 - x0 + 1,
            lanes => i64::from(lanes) * band.height,
        };
        let pricing = Pricing {
            rows: &rows,
            extents: problem.cutter().row_extents(),
            band,
            strips: Strips::covering(x0, x1, width),
            max_gap,
            end_cost: self.segment_cost * band.height as f64,
        };

        // the plain lane grid's residue wins ties
        let plain = (lowest - band.low).rem_euclid(band.height) as usize;
        let chosen = choose::residues(&costs(&pricing), plain);

        let mut spans = lanes_of_runs(&pricing, &chosen);
        spans.sort_unstable();

        spans
            .into_iter()
            .map(|(y, a, b)| segment(STEP, y, a, b))
            .collect()
    }
}

/// The lane segments of every run of strips that share a residue, each run
/// cut on its own columns.
fn lanes_of_runs(pricing: &Pricing, chosen: &[usize]) -> Vec<Span> {
    let band = pricing.band;
    let mut spans = Vec::new();

    let mut first = 0;
    while first < chosen.len() {
        let residue = chosen[first] as i64;
        let last = first
            + chosen[first..]
                .iter()
                .take_while(|&&r| r as i64 == residue)
                .count()
            - 1;

        let (x_min, x_max) = pricing.strips.columns(first, last);
        let rows = pricing.rows.clipped(x_min, x_max);

        if let Some((lowest, _)) = rows.span() {
            // cut_lanes puts its first lane at lowest - band.low - offset
            let offset = (lowest - band.low - residue).rem_euclid(band.height);
            spans.extend(cut_lanes(&rows, pricing.extents, band, offset, pricing.max_gap));
        }

        first = last + 1;
    }

    spans
}

impl Decomposer for HuggedLanes {
    /// `hug-h` or `hug-v`, then every knob.
    fn label(&self) -> String {
        format!(
            "{STEP}-{}/strip_lanes={}/segment_cost={:?}/gap_factor={:?}",
            suffix(self.axis),
            self.strip_lanes,
            self.segment_cost,
            self.gap_factor
        )
    }

    fn decompose(&self, problem: &Problem) -> Result<Vec<Segment>, SolveError> {
        along(self.axis, problem, |p| self.horizontal(p))
    }
}

#[cfg(test)]
mod tests;
