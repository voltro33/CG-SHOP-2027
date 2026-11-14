//! Re-covering the hole: lane segments over the cells left bare, built by
//! `mow-decompose` on a problem whose region is just those cells, and single
//! anchors for whatever is still missed.

use std::collections::HashSet;

use mow_core::{Axis, BitGrid, Decomposer, Point, Problem, Segment};
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

/// One anchor per cell of `missing` not yet swept by an anchor picked
/// before it: of the anchors that sweep the cell, the one closest to the
/// nearest of `near`.
pub fn anchors_for(problem: &Problem, missing: &[Point], near: &[Point]) -> Vec<Point> {
    let offsets = problem.cutter().offsets();
    let mut swept: HashSet<Point> = HashSet::new();
    let mut picked = Vec::new();

    for &cell in missing {
        if swept.contains(&cell) {
            continue;
        }

        let distance = |a: Point| near.iter().map(|&p| p.l1(a)).min().unwrap_or(0);
        let anchor = offsets
            .iter()
            .map(|o| cell.shifted(-o.x, -o.y))
            .min_by_key(|&a| distance(a))
            .unwrap_or(cell);

        swept.extend(offsets.iter().map(|o| anchor.shifted(o.x, o.y)));
        picked.push(anchor);
    }

    picked
}

#[cfg(test)]
mod tests {
    use super::*;
    use mow_core::{Bounds, Cutter};

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
    fn names_are_distinct() {
        let names: HashSet<String> = Recreate::ALL.iter().map(|r| r.name()).collect();
        assert_eq!(names.len(), Recreate::ALL.len());
    }
}
