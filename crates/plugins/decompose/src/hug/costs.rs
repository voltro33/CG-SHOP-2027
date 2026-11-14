//! What every lane residue costs in every strip, and what lanes running on
//! across each strip border save.
//!
//! A lane sits at a height `y` with `y mod height == residue`. Within one
//! strip, the lanes of a residue are cut into segments exactly as the plain
//! lane segments are, but only for the strip's own cells. A segment costs
//! its length plus `segment_cost` lane heights for its two ends.

use mow_core::RowExtent;

use crate::intervals::{merge, Interval};
use crate::rows::{Band, RowRuns};

/// Strips of `width` columns side by side, the first starting at `x0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Strips {
    pub(super) x0: i64,
    pub(super) width: i64,
    pub(super) count: usize,
}

impl Strips {
    /// Enough strips of `width` to hold the columns `x0..=x1`.
    pub(super) fn covering(x0: i64, x1: i64, width: i64) -> Self {
        let width = width.max(1);
        let count = ((x1 - x0) / width + 1) as usize;

        Self { x0, width, count }
    }

    pub(super) fn index(self, x: i64) -> usize {
        ((x - self.x0).div_euclid(self.width)) as usize
    }

    /// The columns of strips `first..=last`.
    pub(super) fn columns(self, first: usize, last: usize) -> (i64, i64) {
        let start = self.x0 + first as i64 * self.width;
        let end = self.x0 + (last as i64 + 1) * self.width - 1;

        (start, end)
    }
}

/// Costs per residue and strip, row major by residue.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Costs {
    pub(super) residues: usize,
    pub(super) strips: usize,
    /// `cost[r * strips + s]`: the segments of residue `r` inside strip `s`.
    pub(super) cost: Vec<f64>,
    /// `saving[r * strips + s]`: what joining strips `s` and `s + 1` under
    /// residue `r` saves, as a negative number or zero.
    pub(super) saving: Vec<f64>,
}

impl Costs {
    pub(super) fn cost(&self, residue: usize, strip: usize) -> f64 {
        self.cost[residue * self.strips + strip]
    }

    pub(super) fn saving(&self, residue: usize, strip: usize) -> f64 {
        self.saving[residue * self.strips + strip]
    }
}

/// The lane grid and pricing every residue is measured with.
#[derive(Debug, Clone, Copy)]
pub(super) struct Pricing<'a> {
    pub(super) rows: &'a RowRuns,
    pub(super) extents: &'a [RowExtent],
    pub(super) band: Band,
    pub(super) strips: Strips,
    pub(super) max_gap: i64,
    /// Charged per segment, on top of its length.
    pub(super) end_cost: f64,
}

/// The costs of every residue `0..band.height` in every strip. A border's
/// saving is the price of its two strips as one, minus the price of each.
pub(super) fn costs(pricing: &Pricing) -> Costs {
    let strips = pricing.strips;
    let residues = pricing.band.height as usize;
    let cost = strip_costs(pricing);

    // pairs (0, 1), (2, 3), ... and (1, 2), (3, 4), ... as double strips
    let double = |x0: i64| {
        let strips = Strips::covering(
            x0,
            strips.x0 + strips.count as i64 * strips.width,
            2 * strips.width,
        );
        strip_costs(&Pricing { strips, ..*pricing })
    };
    let even = double(strips.x0);
    let odd = double(strips.x0 - strips.width);
    let (even_count, odd_count) = (even.len() / residues.max(1), odd.len() / residues.max(1));

    let mut saving = vec![0.0; cost.len()];
    for residue in 0..residues {
        let at = |s: usize| cost[residue * strips.count + s];

        for border in 0..strips.count.saturating_sub(1) {
            let pair = match border % 2 {
                0 => even[residue * even_count + border / 2],
                _ => odd[residue * odd_count + border.div_ceil(2)],
            };
            saving[residue * strips.count + border] = pair - at(border) - at(border + 1);
        }
    }

    Costs {
        residues,
        strips: strips.count,
        cost,
        saving,
    }
}

/// The price of every residue in every strip, row major by residue.
fn strip_costs(pricing: &Pricing) -> Vec<f64> {
    let residues = pricing.band.height as usize;
    let mut cost = vec![0.0; residues * pricing.strips.count];

    let Some((lowest, highest)) = pricing.rows.span() else {
        return cost;
    };

    let mut scratch = Scratch::default();
    for residue in 0..residues {
        let row = &mut cost[residue * pricing.strips.count..(residue + 1) * pricing.strips.count];

        // every lane whose band touches a region row
        let mut y = first_lane(lowest - pricing.band.high, residue as i64, pricing.band.height);
        while y <= highest - pricing.band.low {
            lane_costs(pricing, y, row, &mut scratch);
            y += pricing.band.height;
        }
    }

    cost
}

/// The lowest `y >= from` with `y mod height == residue`.
pub(super) fn first_lane(from: i64, residue: i64, height: i64) -> i64 {
    from + (residue - from).rem_euclid(height)
}

#[derive(Debug, Default)]
struct Scratch {
    pieces: Vec<(usize, Interval)>,
    group: Vec<Interval>,
    merged: Vec<Interval>,
}

/// Adds the price of the lane at `y` in each strip to `row`.
fn lane_costs(pricing: &Pricing, y: i64, row: &mut [f64], scratch: &mut Scratch) {
    strip_pieces(pricing, y, &mut scratch.pieces);
    scratch.pieces.sort_unstable();

    for group in scratch.pieces.chunk_by(|a, b| a.0 == b.0) {
        scratch.group.clear();
        scratch.group.extend(group.iter().map(|&(_, interval)| interval));
        scratch.merged.clear();
        merge(&mut scratch.group, pricing.max_gap, &mut scratch.merged);

        row[group[0].0] += scratch
            .merged
            .iter()
            .map(|&i| price(i, pricing.end_cost))
            .sum::<f64>();
    }
}

/// The anchor intervals of the lane at `y`, one per region run piece, each
/// tagged with the strip its piece lies in.
fn strip_pieces(pricing: &Pricing, y: i64, out: &mut Vec<(usize, Interval)>) {
    out.clear();

    for extent in pricing.extents {
        let (low, high) = (i64::from(extent.low), i64::from(extent.high));

        for run in pricing.rows.row(y + i64::from(extent.dy)) {
            let (x_min, x_max) = (i64::from(run.x_min), i64::from(run.x_max));

            for strip in pricing.strips.index(x_min)..=pricing.strips.index(x_max) {
                let (start, end) = pricing.strips.columns(strip, strip);
                let piece = (x_min.max(start), x_max.min(end));
                out.push((strip, anchors(piece, low, high)));
            }
        }
    }
}

/// The anchors that sweep the cells `piece` with a cutter row spanning
/// `low..=high`, the middle anchor if the piece is shorter than the row
/// (as [`crate::intervals::lane_intervals`] does).
fn anchors((s, e): Interval, low: i64, high: i64) -> Interval {
    let (a, b) = (s - low, e - high);

    match a > b {
        true => {
            let middle = (a + b).div_euclid(2);
            (middle, middle)
        }
        false => (a, b),
    }
}

fn price((a, b): Interval, end_cost: f64) -> f64 {
    (b - a) as f64 + end_cost
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{problem, rectangle, square};

    fn priced(runs: &[(i32, i32, i32)], width: i64) -> Costs {
        let p = problem(runs, &square(3));
        let rows = RowRuns::new(p.region());
        let (x0, x1) = rows.x_span().unwrap();

        costs(&Pricing {
            rows: &rows,
            extents: p.cutter().row_extents(),
            band: Band::of(p.cutter()),
            strips: Strips::covering(x0, x1, width),
            max_gap: 0,
            end_cost: 6.0,
        })
    }

    #[test]
    fn strips_index_and_bound_their_columns() {
        let strips = Strips::covering(-3, 10, 5);

        assert_eq!(strips.count, 3);
        assert_eq!(strips.index(-3), 0);
        assert_eq!(strips.index(2), 1);
        assert_eq!(strips.columns(1, 2), (2, 11));
    }

    #[test]
    fn first_lane_has_the_residue() {
        assert_eq!(first_lane(-4, 1, 3), -2);
        assert_eq!(first_lane(7, 1, 3), 7);
        assert_eq!(first_lane(8, 1, 3), 10);
    }

    #[test]
    fn one_strip_prices_the_plain_lanes() {
        // 12 x 3 under a 3 x 3 cutter: residue 1 needs one lane of 10 steps
        let costs = priced(&rectangle(12, 3), 100);

        assert_eq!(costs.cost(1, 0), 9.0 + 6.0);
        assert!(costs.cost(0, 0) > costs.cost(1, 0));
        assert!(costs.cost(2, 0) > costs.cost(1, 0));
    }

    #[test]
    fn joined_strips_cost_what_one_strip_costs() {
        let whole = priced(&rectangle(12, 3), 100);
        let split = priced(&rectangle(12, 3), 6);

        for r in 0..3 {
            let summed = split.cost(r, 0) + split.cost(r, 1) + split.saving(r, 0);
            assert_eq!(summed, whole.cost(r, 0), "residue {r}");
        }
    }
}
