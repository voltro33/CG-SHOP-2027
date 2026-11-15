//! Re-covering the hole: lane segments over the cells left bare, built by
//! `mow-decompose` on a problem whose region is just those cells. Whatever
//! they still miss gets single anchors ([`crate::anchors`]).

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

#[cfg(test)]
mod tests {
    use super::*;
    use mow_core::{Bounds, Cutter};
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
    fn names_are_distinct() {
        let names: HashSet<String> = Recreate::ALL.iter().map(|r| r.name()).collect();
        assert_eq!(names.len(), Recreate::ALL.len());
    }
}
