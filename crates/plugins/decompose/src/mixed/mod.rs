//! Mixed lanes: horizontal lanes where the region runs wide, vertical where
//! it runs tall (port of `construct/mixed.py`, up to `mixed_segment_ends`).
//!
//! One sweep direction for the whole region pays for every lane end, turn
//! and shoulder where the boundary is ragged across the lanes. So the region
//! is cut into square tiles on the lane grid, and each tile sweeps one way:
//!
//! 1. Tiles are `tile_lanes` lanes high and wide, with borders on lane
//!    borders in both directions, so a lane never crosses a tile border
//!    sideways.
//! 2. The cost of a choice is `length + segment_cost * band` summed over the
//!    segments it makes, where all horizontal tiles of one tile row are cut
//!    into segments together, and the same for the vertical tiles of one
//!    tile column.
//! 3. The search starts from the cheaper of all horizontal and all vertical,
//!    then flips single tiles while that lowers the cost.
//!
//! Ordering both sets of segments together is the pipeline's job.

mod search;
mod tiles;
mod view;

use mow_core::{Decomposer, Problem, Segment, SolveError};

use crate::axis::segment;
use crate::segments::truncated;

use search::tile_directions;
use tiles::TileGrid;
use view::View;

const STEP: &str = "segments-mixed";

const DEFAULT_TILE_LANES: u32 = 4;
const DEFAULT_SEGMENT_COST: f64 = 3.0;
const DEFAULT_GAP_FACTOR: f64 = 0.0;
const DEFAULT_SHIFT: f64 = 0.0;
const DEFAULT_FLIP_ROUNDS: u32 = 4;

/// Horizontal and vertical lane segments, each tile of the region swept the
/// cheaper way. The horizontal segments come first, bottom to top, then the
/// vertical ones, left to right, as in Python.
///
/// Within the final lanes, gaps up to `gap_factor` lane heights are bridged.
/// `shift` moves every lane and tile border down and left by that fraction
/// of a lane; it must stay below `tile_lanes`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MixedLanes {
    pub tile_lanes: u32,
    pub segment_cost: f64,
    pub gap_factor: f64,
    pub shift: f64,
    pub flip_rounds: u32,
}

impl MixedLanes {
    /// The Python defaults: tiles of 4 lanes, segment cost 3, no gap
    /// bridging, no shift, at most 4 flip rounds.
    pub const fn new() -> Self {
        Self {
            tile_lanes: DEFAULT_TILE_LANES,
            segment_cost: DEFAULT_SEGMENT_COST,
            gap_factor: DEFAULT_GAP_FACTOR,
            shift: DEFAULT_SHIFT,
            flip_rounds: DEFAULT_FLIP_ROUNDS,
        }
    }
}

impl Default for MixedLanes {
    fn default() -> Self {
        Self::new()
    }
}

impl Decomposer for MixedLanes {
    /// `segments-mixed`, then each knob that differs from its default as
    /// `/key=value`, keys sorted as in `SolveConfig.label`.
    fn label(&self) -> String {
        let mut label = STEP.to_string();

        if self.flip_rounds != DEFAULT_FLIP_ROUNDS {
            label += &format!("/flip_rounds={}", self.flip_rounds);
        }
        if self.gap_factor != DEFAULT_GAP_FACTOR {
            label += &format!("/gap_factor={:?}", self.gap_factor);
        }
        if self.segment_cost != DEFAULT_SEGMENT_COST {
            label += &format!("/segment_cost={:?}", self.segment_cost);
        }
        if self.shift != DEFAULT_SHIFT {
            label += &format!("/shift={:?}", self.shift);
        }
        if self.tile_lanes != DEFAULT_TILE_LANES {
            label += &format!("/tile_lanes={}", self.tile_lanes);
        }

        label
    }

    fn decompose(&self, problem: &Problem) -> Result<Vec<Segment>, SolveError> {
        let grid = TileGrid::new(problem, self.tile_lanes, self.shift)?;

        // vertical lanes are horizontal lanes of the mirrored problem
        let horizontal = View::new(problem, grid, false);
        let vertical = View::new(&problem.transposed(), grid.transposed(), true);

        let directions = tile_directions(&horizontal, &vertical, self.segment_cost, self.flip_rounds);

        let band_y = grid.rows.size / i64::from(self.tile_lanes);
        let band_x = grid.columns.size / i64::from(self.tile_lanes);
        let gap_y = truncated(STEP, self.gap_factor, band_y)?;
        let gap_x = truncated(STEP, self.gap_factor, band_x)?;

        let mut segments = Vec::new();
        for (y, a, b) in horizontal.part_spans(&directions, gap_y) {
            segments.push(segment(STEP, y, a, b)?);
        }
        for (x, a, b) in vertical.part_spans(&directions, gap_x) {
            segments.push(segment(STEP, x, a, b)?.transposed());
        }

        Ok(segments)
    }
}

#[cfg(test)]
mod tests {
    use mow_core::Axis;

    use super::*;
    use crate::testing::{horizontal, problem, rectangle, square, vertical};

    #[test]
    fn a_wide_rectangle_sweeps_horizontally() {
        let p = problem(&rectangle(12, 6), &square(3));

        let segments = MixedLanes::new().decompose(&p).unwrap();

        assert_eq!(segments, horizontal(&[(1, 1, 10), (4, 1, 10)]));
    }

    #[test]
    fn a_tall_rectangle_sweeps_vertically() {
        let p = problem(&rectangle(6, 12), &square(3));

        let segments = MixedLanes::new().decompose(&p).unwrap();

        assert_eq!(segments, vertical(&[(1, 1, 10), (4, 1, 10)]));
    }

    #[test]
    fn an_l_shape_mixes_both_axes() {
        // a long bar along the bottom and a tall one up the left side
        let mut runs: Vec<(i32, i32, i32)> = (0..2).map(|y| (y, 0, 39)).collect();
        runs.extend((2..40).map(|y| (y, 0, 1)));
        let p = problem(&runs, &[(0, 0)]);

        // tiles 20 cells wide, so the upper left one holds a bare tall bar
        let segments = MixedLanes {
            tile_lanes: 20,
            ..MixedLanes::new()
        }
        .decompose(&p)
        .unwrap();

        let axes: Vec<Axis> = segments.iter().map(|s| s.axis()).collect();
        assert!(axes.contains(&Axis::Horizontal) && axes.contains(&Axis::Vertical));

        // horizontal ones first, as Python concatenates them
        let first_vertical = axes.iter().position(|&a| a == Axis::Vertical).unwrap();
        assert!(axes[first_vertical..].iter().all(|&a| a == Axis::Vertical));
    }

    #[test]
    fn bad_knobs_are_refused() {
        let p = problem(&rectangle(12, 6), &square(3));
        let with = |change: fn(&mut MixedLanes)| {
            let mut mixed = MixedLanes::new();
            change(&mut mixed);
            mixed.decompose(&p)
        };

        assert!(with(|m| m.tile_lanes = 0).is_err());
        assert!(with(|m| m.gap_factor = f64::NAN).is_err());
        assert!(with(|m| m.shift = -0.5).is_err());
    }

    #[test]
    fn labels_show_only_changed_knobs_sorted() {
        assert_eq!(MixedLanes::new().label(), "segments-mixed");

        let changed = MixedLanes {
            tile_lanes: 2,
            segment_cost: 4.0,
            gap_factor: 2.0,
            shift: 0.5,
            flip_rounds: 1,
        };
        assert_eq!(
            changed.label(),
            "segments-mixed/flip_rounds=1/gap_factor=2.0/segment_cost=4.0/shift=0.5/tile_lanes=2"
        );
    }
}
