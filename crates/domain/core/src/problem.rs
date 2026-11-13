//! An instance as the solver sees it, built once and borrowed by every step.

use crate::cutter::Cutter;
use crate::error::ModelError;
use crate::geom::{Bounds, Point};
use crate::grid::BitGrid;

/// The anchors whose footprint can touch the region. Anchors outside it
/// sweep nothing that matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    pub bounds: Bounds,
}

impl Frame {
    pub const fn contains(self, p: Point) -> bool {
        self.bounds.contains(p)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    uid: String,
    region: BitGrid,
    region_bounds: Bounds,
    cells: usize,
    cutter: Cutter,
    k: usize,
}

impl Problem {
    pub fn new(
        uid: impl Into<String>,
        region: BitGrid,
        cutter: Cutter,
        k: usize,
    ) -> Result<Self, ModelError> {
        if !(1..=3).contains(&k) {
            return Err(ModelError::CutterCount(k));
        }

        let region_bounds = region.cell_bounds().ok_or(ModelError::EmptyRegion)?;
        let cells = region.len();

        Ok(Self {
            uid: uid.into(),
            region,
            region_bounds,
            cells,
            cutter,
            k,
        })
    }

    pub fn uid(&self) -> &str {
        &self.uid
    }

    pub const fn region(&self) -> &BitGrid {
        &self.region
    }

    /// The box of the region's cells.
    pub const fn region_bounds(&self) -> Bounds {
        self.region_bounds
    }

    pub const fn num_cells(&self) -> usize {
        self.cells
    }

    pub const fn cutter(&self) -> &Cutter {
        &self.cutter
    }

    pub const fn k(&self) -> usize {
        self.k
    }

    /// The smallest box holding every anchor whose footprint can touch the
    /// region (`Problem.anchor_frame` in Python).
    pub const fn anchor_frame(&self) -> Frame {
        let r = self.region_bounds;
        let c = self.cutter.bounds();

        Frame {
            bounds: Bounds {
                x_min: r.x_min - c.x_max,
                y_min: r.y_min - c.y_max,
                x_max: r.x_max - c.x_min,
                y_max: r.y_max - c.y_min,
            },
        }
    }

    /// The same problem mirrored across `x = y`, so a step written for
    /// horizontal lanes also yields vertical ones.
    pub fn transposed(&self) -> Self {
        Self {
            uid: self.uid.clone(),
            region: self.region.transposed(),
            region_bounds: self.region_bounds.transposed(),
            cells: self.cells,
            cutter: self.cutter.transposed(),
            k: self.k,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A `width x height` rectangle of cells at the origin, mowed by a
    /// centered `side x side` square cutter.
    pub(crate) fn rectangle(width: i32, height: i32, side: i32, k: usize) -> Problem {
        let mut region = BitGrid::empty(Bounds {
            x_min: 0,
            y_min: 0,
            x_max: width - 1,
            y_max: height - 1,
        })
        .unwrap();
        for y in 0..height {
            region.insert_run(y, 0, width - 1);
        }

        let half = side / 2;
        let mut cutter = BitGrid::empty(Bounds {
            x_min: -half,
            y_min: -half,
            x_max: side - 1 - half,
            y_max: side - 1 - half,
        })
        .unwrap();
        for y in -half..side - half {
            cutter.insert_run(y, -half, side - 1 - half);
        }

        Problem::new("rect", region, Cutter::new(cutter).unwrap(), k).unwrap()
    }

    #[test]
    fn anchor_frame_reaches_every_touching_anchor() {
        let p = rectangle(10, 4, 3, 1);
        let frame = p.anchor_frame().bounds;

        assert_eq!(
            frame,
            Bounds {
                x_min: -1,
                y_min: -1,
                x_max: 10,
                y_max: 4
            }
        );
    }

    #[test]
    fn rejects_bad_cutter_counts() {
        let p = rectangle(2, 2, 1, 1);
        let again = Problem::new("x", p.region().clone(), p.cutter().clone(), 4);

        assert_eq!(again.unwrap_err(), ModelError::CutterCount(4));
    }

    #[test]
    fn transposed_swaps_the_region_box() {
        let p = rectangle(10, 4, 3, 2).transposed();

        assert_eq!(
            p.region_bounds(),
            Bounds {
                x_min: 0,
                y_min: 0,
                x_max: 3,
                y_max: 9
            }
        );
        assert_eq!(p.num_cells(), 40);
        assert_eq!(p.k(), 2);
    }
}
