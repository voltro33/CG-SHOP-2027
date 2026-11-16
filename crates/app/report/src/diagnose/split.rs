//! What splitting the lawn between cutters costs, next to the breakdown.
//!
//! The parts of the breakdown keep their meaning; these measures cut across
//! them:
//!
//! - cross-tour overlap: the share of the near and far resweep parts that
//!   re-mows cells another tour swept first, while this tour has not swept
//!   them yet (a tour's own later passes count as its own resweep), and of
//!   each dead step the share of its fresh footprint that is such cells. Its
//!   total does not depend on the order the tours are walked in.
//! - shared cells: region cells swept by two tours or more.
//! - lane ends by kind (see [`super::ends`]): a border end is a lane that
//!   stops inside the lawn where another tour's area begins. Its waste does
//!   not hold cross-tour overlap, so the two add up.
//! - imbalance: the longest tour less the mean one, the length the score
//!   pays for tours that are not equally long.

use super::ends::{LaneEnds, END_KINDS};
use super::Breakdown;
use crate::pyfmt::{percent, Json};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Split {
    pub tour_lengths: Vec<u64>,
    pub near_resweep_cross: f64,
    pub far_resweep_cross: f64,
    pub dead_cross: f64,
    pub shared_cells: u64,
    pub lane_ends: LaneEnds,
}

impl Split {
    /// The length spent re-mowing cells another tour mows.
    pub fn cross(&self) -> f64 {
        self.near_resweep_cross + self.far_resweep_cross + self.dead_cross
    }

    /// The waste of the lane ends at a border between two tours' areas.
    pub fn border_waste(&self) -> f64 {
        self.lane_ends.waste[2]
    }

    /// The longest tour less the mean tour.
    pub fn imbalance(&self) -> f64 {
        let longest = self.tour_lengths.iter().max().copied().unwrap_or(0);
        let total: u64 = self.tour_lengths.iter().sum();

        match self.tour_lengths.len() {
            0 => 0.0,
            k => longest as f64 - total as f64 / k as f64,
        }
    }

    pub fn to_json(&self) -> Json {
        let mut pairs = vec![
            (
                "tour_lengths",
                Json::List(self.tour_lengths.iter().map(|&l| Json::int(l)).collect()),
            ),
            ("near_resweep_cross", Json::Float(self.near_resweep_cross)),
            ("far_resweep_cross", Json::Float(self.far_resweep_cross)),
            ("dead_cross", Json::Float(self.dead_cross)),
            ("cross", Json::Float(self.cross())),
            ("shared_cells", Json::int(self.shared_cells)),
            ("imbalance", Json::Float(self.imbalance())),
        ];

        let ends = &self.lane_ends;
        let names = END_KINDS.map(|kind| (format!("lane_ends_{kind}"), format!("lane_end_waste_{kind}")));
        let counted = names.iter().zip(ends.counts.iter().zip(ends.waste));
        let end_pairs: Vec<(&str, Json)> = counted
            .flat_map(|((count_name, waste_name), (&count, waste))| {
                [
                    (count_name.as_str(), Json::int(count)),
                    (waste_name.as_str(), Json::Float(waste)),
                ]
            })
            .collect();
        pairs.extend(end_pairs);

        Json::object(pairs)
    }
}

/// The header of the split table.
pub fn split_header() -> String {
    format!(
        "{:30} {:>2} {:>6} {:>6} {:>6} {:>6} {:>6}",
        "instance", "k", "cross", "border", "b.ends", "i.ends", "imbal"
    )
}

/// One line of the split table: cross-tour overlap and border end waste as
/// shares of the summed tour length, border and inner ends per lane end,
/// imbalance as a share of the longest tour.
pub fn split_row(result: &Breakdown) -> String {
    let split = &result.split;
    let length = (result.total_length as f64).max(1.0);
    let ends = (split.lane_ends.counts.iter().sum::<u64>() as f64).max(1.0);
    let longest = (result.objective as f64).max(1.0);

    format!(
        "{:30} {:>2} {} {} {} {} {}",
        result.instance,
        result.k,
        percent(split.cross() / length, 6, 1),
        percent(split.border_waste() / length, 6, 1),
        percent(split.lane_ends.counts[2] as f64 / ends, 6, 1),
        percent(split.lane_ends.counts[1] as f64 / ends, 6, 1),
        percent(split.imbalance() / longest, 6, 2),
    )
}
