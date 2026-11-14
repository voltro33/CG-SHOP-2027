//! The newer lane patterns, reached as params of the lane methods
//! (`segments-h`, `segments-v`, `groups-h`, `groups-v`), so a label such as
//! `groups-h/gap_factor=0/hug=4` names its base config plus the change:
//!
//! - `hug=S`: [`HuggedLanes`], the lane offset chosen per strip of `S` lane
//!   heights (0: one offset for the whole region); `hug_cost` prices a
//!   segment end in lane heights (default 2).
//!
//! Joining lanes across shallow gaps and spirals were tried and lost on the
//! bench (2026-11-13), so they are not here.

use mow_core::{Axis, Decomposer};
use mow_decompose::HuggedLanes;
use mow_pipeline::SolveConfig;

/// The params the patterns add to the lane methods.
pub(crate) const PATTERN_PARAMS: [&str; 2] = ["hug", "hug_cost"];

const DEFAULT_HUG_COST: f64 = 2.0;

/// Params that move the global lane grid, which hug replaces.
const GRID_PARAMS: [&str; 3] = ["shift", "offset", "max_gap"];

/// The decomposer of the pattern `config` asks for, or `None` for the
/// plain lane segments. `gap_factor` is the method's own default.
pub(crate) fn decomposer(
    config: &SolveConfig,
    axis: Axis,
    gap_factor: f64,
) -> Result<Option<Box<dyn Decomposer>>, String> {
    if config.param("hug").is_none() {
        return Ok(None);
    }
    if let Some(knob) = GRID_PARAMS.iter().find(|knob| config.param(knob).is_some()) {
        return Err(format!("hug takes no {knob}"));
    }

    let gap_factor = config.param_or("gap_factor", gap_factor);

    Ok(Some(Box::new(hugged(config, axis, gap_factor)?)))
}

fn hugged(config: &SolveConfig, axis: Axis, gap_factor: f64) -> Result<HuggedLanes, String> {
    let strips = config.param_or("hug", 0.0);
    let strip_lanes = count("hug", strips)?;

    Ok(HuggedLanes {
        axis,
        strip_lanes,
        segment_cost: config.param_or("hug_cost", DEFAULT_HUG_COST),
        gap_factor,
    })
}

/// A whole number of at least 0.
fn count(name: &str, value: f64) -> Result<u32, String> {
    let fits = value.fract() == 0.0 && (0.0..=f64::from(u32::MAX)).contains(&value);

    match fits {
        true => Ok(value as u32),
        false => Err(format!(
            "{name} must be a whole number of at least 0, not {value}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pattern(label: &str) -> Result<Option<String>, String> {
        let config = SolveConfig::parse(label).unwrap();
        let decomposer = decomposer(&config, Axis::Horizontal, 0.0)?;

        Ok(decomposer.map(|d| d.label()))
    }

    #[test]
    fn plain_labels_keep_the_lane_segments() {
        assert_eq!(pattern("groups-h/gap_factor=0"), Ok(None));
    }

    #[test]
    fn hug_reads_its_knobs() {
        assert_eq!(
            pattern("groups-h/gap_factor=1/hug=4/hug_cost=3")
                .unwrap()
                .unwrap(),
            "hug-h/strip_lanes=4/segment_cost=3.0/gap_factor=1.0"
        );
        assert_eq!(
            pattern("segments-h/hug=0").unwrap().unwrap(),
            "hug-h/strip_lanes=0/segment_cost=2.0/gap_factor=0.0"
        );
    }

    #[test]
    fn hug_refuses_grid_moves_and_odd_strips() {
        assert!(pattern("segments-h/hug=4/shift=0.5").is_err());
        assert!(pattern("segments-h/hug=2.5").is_err());
        assert!(pattern("segments-h/hug=-1").is_err());
    }

    #[test]
    fn hug_refuses_bad_values() {
        assert!(pattern("segments-h/hug=-1").is_err());
        assert!(pattern("segments-h/hug=0/offset=1").is_err());
    }
}
