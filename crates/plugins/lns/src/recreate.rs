//! Re-covering the hole: lane segments over the cells left bare, built by
//! `mow-decompose` on a problem whose region is just those cells, and single
//! anchors for whatever is still missed.

use std::cmp::Reverse;

use mow_core::{Axis, BitGrid, Bounds, Decomposer, Point, Problem, Segment};
use mow_decompose::LaneSegments;

use crate::cycle::Stop;
use crate::error::LnsError;

/// How the bare cells are swept again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recreate {
    /// No lanes: one anchor per missed cell, joined into the tour.
    Anchors,
    /// Lanes along `axis`, gaps bridged up to `gap` lane heights.
    Lanes { axis: Axis, gap: u8 },
}

impl Recreate {
    pub const ALL: [Self; 5] = [
        Self::Anchors,
        Self::Lanes {
            axis: Axis::Horizontal,
            gap: 0,
        },
        Self::Lanes {
            axis: Axis::Vertical,
            gap: 0,
        },
        Self::Lanes {
            axis: Axis::Horizontal,
            gap: 2,
        },
        Self::Lanes {
            axis: Axis::Vertical,
            gap: 2,
        },
    ];

    pub fn name(self) -> String {
        match self {
            Self::Anchors => "anchors".to_owned(),
            Self::Lanes { axis, gap } => {
                let axis = if axis == Axis::Horizontal { "h" } else { "v" };
                format!("lanes-{axis}-gap{gap}")
            }
        }
    }

    /// The new stops over `bare`, lanes moved down by `offset` rows.
    pub fn stops(self, problem: &Problem, bare: &[Point], offset: i32) -> Result<Vec<Stop>, LnsError> {
        let Self::Lanes { axis, gap } = self else {
            return Ok(Vec::new());
        };
        let Some(region) = BitGrid::from_cells(bare)? else {
            return Ok(Vec::new());
        };

        let hole = Problem::new(problem.uid(), region, problem.cutter().clone(), 1)?;
        let lanes = LaneSegments {
            gap_factor: f64::from(gap),
            offset,
            ..LaneSegments::new(axis)
        };

        Ok(lanes.decompose(&hole)?.into_iter().map(stop).collect())
    }
}

fn stop(segment: Segment) -> Stop {
    Stop {
        entry: segment.start(),
        exit: segment.end(),
    }
}

/// Anchors tried per missed cell besides the nearest: an even spread of
/// the cutter's offsets, so a large cutter costs a bounded count.
const ANCHOR_CANDIDATES: usize = 32;

/// Anchors sweeping every cell of `missing`, taken in order: for each cell
/// not yet swept, of the anchors that sweep it, the one sweeping the most
/// missed cells still left, the nearest to `near` among those.
///
/// The nearest anchor alone puts the cell on the cutter's far edge, the
/// footprint back over swept ground: with a 60 x 60 cutter that took one
/// anchor per 14 missed cells.
pub fn anchors_for(problem: &Problem, missing: &[Point], near: &[Point]) -> Vec<Point> {
    let offsets = problem.cutter().offsets();
    let stride = (offsets.len() / ANCHOR_CANDIDATES).max(1);
    let distance = |a: Point| near.iter().map(|&p| p.l1(a)).min().unwrap_or(0);

    let Some(mut left) = Missed::new(missing) else {
        return Vec::new();
    };
    let mut picked = Vec::new();

    for &cell in missing {
        if !left.contains(cell) {
            continue;
        }

        let nearest = offsets
            .iter()
            .map(|o| cell.shifted(-o.x, -o.y))
            .min_by_key(|&a| distance(a))
            .unwrap_or(cell);
        let spread = offsets.iter().step_by(stride).map(|o| cell.shifted(-o.x, -o.y));

        let anchor = std::iter::once(nearest)
            .chain(spread)
            .max_by_key(|&a| (left.count_under(a, offsets), Reverse(distance(a))))
            .unwrap_or(cell);

        left.sweep(anchor, offsets);
        picked.push(anchor);
    }

    picked
}

/// The missed cells not swept yet, as a bitmap over their bounding box.
struct Missed {
    bounds: Bounds,
    width: usize,
    left: Vec<bool>,
}

impl Missed {
    /// `None` for no cells.
    fn new(cells: &[Point]) -> Option<Self> {
        let first = *cells.first()?;
        let bounds = cells.iter().fold(
            Bounds {
                x_min: first.x,
                y_min: first.y,
                x_max: first.x,
                y_max: first.y,
            },
            |b, c| Bounds {
                x_min: b.x_min.min(c.x),
                y_min: b.y_min.min(c.y),
                x_max: b.x_max.max(c.x),
                y_max: b.y_max.max(c.y),
            },
        );

        let width = bounds.width() as usize;
        let mut missed = Self {
            bounds,
            width,
            left: vec![false; width * bounds.height() as usize],
        };
        for &c in cells {
            if let Some(i) = missed.index(c) {
                missed.left[i] = true;
            }
        }

        Some(missed)
    }

    fn index(&self, p: Point) -> Option<usize> {
        let b = self.bounds;
        self.bounds
            .contains(p)
            .then(|| (p.y - b.y_min) as usize * self.width + (p.x - b.x_min) as usize)
    }

    fn contains(&self, p: Point) -> bool {
        self.index(p).is_some_and(|i| self.left[i])
    }

    /// How many cells still left the cutter at `anchor` sweeps.
    fn count_under(&self, anchor: Point, offsets: &[Point]) -> usize {
        offsets
            .iter()
            .filter(|o| self.contains(anchor.shifted(o.x, o.y)))
            .count()
    }

    fn sweep(&mut self, anchor: Point, offsets: &[Point]) {
        for o in offsets {
            if let Some(i) = self.index(anchor.shifted(o.x, o.y)) {
                self.left[i] = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mow_core::Cutter;
    use std::collections::HashSet;

    fn square_cutter(side: i32) -> Cutter {
        let half = side / 2;
        let cells: Vec<Point> = (-half..side - half)
            .flat_map(|y| (-half..side - half).map(move |x| Point::new(x, y)))
            .collect();
        Cutter::new(BitGrid::from_cells(&cells).unwrap().unwrap()).unwrap()
    }

    fn problem() -> Problem {
        let mut region = BitGrid::empty(Bounds {
            x_min: 0,
            y_min: 0,
            x_max: 19,
            y_max: 19,
        })
        .unwrap();
        for y in 0..20 {
            region.insert_run(y, 0, 19);
        }
        Problem::new("square", region, square_cutter(3), 1).unwrap()
    }

    #[test]
    fn lanes_cover_a_block_of_bare_cells() {
        let p = problem();
        let bare: Vec<Point> = (5..11)
            .flat_map(|y| (2..8).map(move |x| Point::new(x, y)))
            .collect();

        let h = Recreate::Lanes {
            axis: Axis::Horizontal,
            gap: 0,
        }
        .stops(&p, &bare, 0)
        .unwrap();
        let v = Recreate::Lanes {
            axis: Axis::Vertical,
            gap: 0,
        }
        .stops(&p, &bare, 0)
        .unwrap();

        assert_eq!(h.len(), 2);
        assert!(h.iter().all(|s| s.entry.y == s.exit.y));
        assert_eq!(v.len(), 2);
        assert!(v.iter().all(|s| s.entry.x == s.exit.x));
        assert!(Recreate::Anchors.stops(&p, &bare, 0).unwrap().is_empty());
    }

    #[test]
    fn anchors_sweep_every_missing_cell_with_few_stops() {
        let p = problem();
        let missing: Vec<Point> = (0..3)
            .flat_map(|y| (0..3).map(move |x| Point::new(x + 10, y + 10)))
            .collect();

        let anchors = anchors_for(&p, &missing, &[Point::new(11, 11)]);

        assert_eq!(anchors, vec![Point::new(11, 11)]);
    }

    #[test]
    fn anchors_prefer_covering_more_over_standing_nearer() {
        let p = problem();
        let missing: Vec<Point> = (0..3)
            .flat_map(|y| (0..3).map(move |x| Point::new(x + 10, y + 10)))
            .collect();

        let anchors = anchors_for(&p, &missing, &[Point::new(0, 0)]);

        assert_eq!(anchors, vec![Point::new(11, 11)]);
    }

    #[test]
    fn anchors_sweep_every_missing_cell() {
        let p = problem();
        let missing: Vec<Point> = [(2, 2), (3, 2), (9, 4), (15, 15), (16, 17), (4, 12)]
            .map(Point::from)
            .to_vec();

        let anchors = anchors_for(&p, &missing, &[Point::new(0, 0)]);

        let offsets = p.cutter().offsets();
        let swept: HashSet<Point> = anchors
            .iter()
            .flat_map(|a| offsets.iter().map(move |o| a.shifted(o.x, o.y)))
            .collect();
        assert!(missing.iter().all(|c| swept.contains(c)));
        assert_eq!(anchors.len(), 4);
    }

    #[test]
    fn names_are_distinct() {
        let names: HashSet<String> = Recreate::ALL.iter().map(|r| r.name()).collect();
        assert_eq!(names.len(), Recreate::ALL.len());
    }
}
