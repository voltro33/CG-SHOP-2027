//! Incremental coverage: how often each anchor is visited, and how many
//! visited anchors sweep each cell (`improve.VisitCounts` in Python).
//!
//! Two dense arrays, both row major:
//!
//! - visits, `u16` per anchor of the anchor frame;
//! - cover, `u16` per cell of the *cover box*: the anchor frame grown by the
//!   cutter's extent, so every footprint of every frame anchor lies inside
//!   it and stamping needs no bounds check. Cells outside the region hold
//!   [`OUTSIDE`] and are never counted.
//!
//! A cell is mowed while its cover is positive. Cover never exceeds the
//! cutter's area, which is checked to fit.
//!
//! Queries never mutate the counts. They tally pending removals in a
//! [`Scratch`] passed by the caller: dense arrays the size of the counts plus
//! the list of entries they touched, reset after each query. After the first
//! query has sized it, a query allocates nothing. Dense beats a hash map here
//! because a query touches a few hundred entries in a few cache lines.

use mow_core::{Bounds, ModelError, Point, Problem, Route};

use crate::error::CoverageError;

/// Cover value of a cell outside the region.
const OUTSIDE: u16 = u16::MAX;

/// Entries the counts may hold, far above any instance.
const MAX_ENTRIES: u64 = 1 << 32;

#[derive(Debug, Clone)]
pub struct VisitCounts {
    frame: Bounds,
    frame_width: usize,
    visits: Vec<u16>,

    cover_width: usize,
    cover: Vec<u16>,

    /// Cover index of each cutter cell, relative to the cover index of the
    /// cutter box's low corner.
    footprint: Vec<usize>,
    uncovered: usize,
}

impl VisitCounts {
    /// No anchor visited yet: every region cell is uncovered.
    pub fn new(problem: &Problem) -> Result<Self, CoverageError> {
        let area = problem.cutter().area();
        if area >= usize::from(OUTSIDE) {
            return Err(CoverageError::CutterTooLarge(area));
        }

        let frame = problem.anchor_frame().bounds;
        let c = problem.cutter().bounds();
        let cover_box = Bounds {
            x_min: frame.x_min + c.x_min,
            y_min: frame.y_min + c.y_min,
            x_max: frame.x_max + c.x_max,
            y_max: frame.y_max + c.y_max,
        };

        let visits = vec![0; entries(frame)?];
        let mut cover = vec![OUTSIDE; entries(cover_box)?];
        let cover_width = cover_box.width() as usize;

        for run in problem.region().runs() {
            let start = index(cover_box, cover_width, Point::new(run.x_min, run.y));
            cover[start..start + run.len() as usize].fill(0);
        }

        let footprint = problem
            .cutter()
            .offsets()
            .iter()
            .map(|o| (o.y - c.y_min) as usize * cover_width + (o.x - c.x_min) as usize)
            .collect();

        Ok(Self {
            frame,
            frame_width: frame.width() as usize,
            visits,
            cover_width,
            cover,
            footprint,
            uncovered: problem.num_cells(),
        })
    }

    /// Every position of every route visited once per pass.
    pub fn from_routes(problem: &Problem, routes: &[Route]) -> Result<Self, CoverageError> {
        let mut counts = Self::new(problem)?;
        for route in routes {
            counts.add_positions(route.positions())?;
        }

        Ok(counts)
    }

    pub const fn is_covered(&self) -> bool {
        self.uncovered == 0
    }

    pub const fn uncovered_count(&self) -> usize {
        self.uncovered
    }

    /// How often `p` is visited. Anchors outside the frame count as never.
    pub fn visits(&self, p: Point) -> u16 {
        self.anchor(p).map_or(0, |i| self.visits[i])
    }

    /// Visits each position once more. Positions outside the anchor frame
    /// sweep no region cell and are skipped. On error the positions before
    /// the bad one stay added.
    pub fn add_positions(&mut self, positions: impl IntoIterator<Item = Point>) -> Result<(), CoverageError> {
        for p in positions {
            let Some(i) = self.anchor(p) else { continue };

            let before = self.visits[i];
            self.visits[i] = before.checked_add(1).ok_or(CoverageError::VisitOverflow(p))?;
            if before == 0 {
                self.stamp_up(self.cover_base(i));
            }
        }

        Ok(())
    }

    /// Visits each position once less. On error the positions before the
    /// bad one stay removed.
    pub fn remove_positions(
        &mut self,
        positions: impl IntoIterator<Item = Point>,
    ) -> Result<(), CoverageError> {
        for p in positions {
            let Some(i) = self.anchor(p) else { continue };

            let after = self.visits[i]
                .checked_sub(1)
                .ok_or(CoverageError::NotVisited(p))?;
            self.visits[i] = after;
            if after == 0 {
                self.stamp_down(self.cover_base(i));
            }
        }

        Ok(())
    }

    /// How many mowed region cells would be left unmowed with one visit of
    /// each of `positions` taken away. A position listed more often than it
    /// is visited loses all its visits, no more. The counts do not change.
    pub fn cells_lost_if_removed(
        &self,
        scratch: &mut Scratch,
        positions: impl IntoIterator<Item = Point>,
    ) -> usize {
        self.tally(scratch, positions, false)
    }

    /// True if every mowed cell stays mowed without `positions`, as
    /// [`Self::cells_lost_if_removed`] but stopping at the first lost cell.
    pub fn still_covered_without(
        &self,
        scratch: &mut Scratch,
        positions: impl IntoIterator<Item = Point>,
    ) -> bool {
        self.tally(scratch, positions, true) == 0
    }

    /// True if every region cell in the footprint of any of `positions` is
    /// mowed now (`VisitCounts.all_swept_around` in Python). Unlike
    /// [`Self::still_covered_without`] this asks about the counts as they
    /// are, so a caller applies its edit first, and a cell that was never
    /// mowed fails the test too.
    ///
    /// A position still visited mows its whole footprint itself, so only the
    /// footprints of unvisited positions are looked at.
    pub fn all_swept_around(&self, positions: impl IntoIterator<Item = Point>) -> bool {
        positions.into_iter().all(|p| {
            let Some(i) = self.anchor(p) else { return true };
            if self.visits[i] > 0 {
                return true;
            }

            let base = self.cover_base(i);
            self.footprint.iter().all(|&d| self.cover[base + d] != 0)
        })
    }

    fn tally(&self, scratch: &mut Scratch, positions: impl IntoIterator<Item = Point>, first: bool) -> usize {
        scratch.fit(self.visits.len(), self.cover.len());
        let mut lost = 0;

        for p in positions {
            let Some(i) = self.anchor(p) else { continue };
            if !scratch.take_visit(i, self.visits[i]) {
                continue;
            }

            // the last visit of this anchor is gone
            lost += self.lose_footprint(scratch, self.cover_base(i), first);
            if first && lost > 0 {
                break;
            }
        }

        scratch.reset();
        lost
    }

    /// Takes one pending cover from each footprint cell, counting the cells
    /// whose cover runs out.
    fn lose_footprint(&self, scratch: &mut Scratch, base: usize, first: bool) -> usize {
        let mut lost = 0;

        for &d in &self.footprint {
            let cell = base + d;
            if scratch.take_cover(cell, self.cover[cell]) {
                lost += 1;
                if first {
                    break;
                }
            }
        }

        lost
    }

    fn stamp_up(&mut self, base: usize) {
        for &d in &self.footprint {
            let cover = &mut self.cover[base + d];
            if *cover != OUTSIDE {
                self.uncovered -= usize::from(*cover == 0);
                *cover += 1;
            }
        }
    }

    fn stamp_down(&mut self, base: usize) {
        for &d in &self.footprint {
            let cover = &mut self.cover[base + d];
            if *cover != OUTSIDE {
                *cover -= 1;
                self.uncovered += usize::from(*cover == 0);
            }
        }
    }

    fn anchor(&self, p: Point) -> Option<usize> {
        self.frame
            .contains(p)
            .then(|| index(self.frame, self.frame_width, p))
    }

    /// The cover index of the low corner of anchor `i`'s cutter box. The
    /// cover box starts that corner's offset below the frame, so it is the
    /// anchor's own column and row in the cover's row width.
    fn cover_base(&self, i: usize) -> usize {
        (i / self.frame_width) * self.cover_width + i % self.frame_width
    }
}

fn entries(b: Bounds) -> Result<usize, ModelError> {
    let (width, height) = (u64::from(b.width()), u64::from(b.height()));
    if width * height > MAX_ENTRIES {
        return Err(ModelError::GridTooLarge { width, height });
    }

    Ok((width * height) as usize)
}

/// Row major index of `p` in `b`, which must contain it.
fn index(b: Bounds, width: usize, p: Point) -> usize {
    (p.y - b.y_min) as usize * width + (p.x - b.x_min) as usize
}

/// Pending removals of one query, kept between queries so they allocate
/// nothing once sized. Owned by the caller, one per search thread.
#[derive(Debug, Clone, Default)]
pub struct Scratch {
    taken_visits: Vec<u16>,
    taken_cover: Vec<u16>,
    touched_anchors: Vec<usize>,
    touched_cells: Vec<usize>,
}

impl Scratch {
    /// Sized up front for `counts`, so even the first query allocates nothing.
    pub fn for_counts(counts: &VisitCounts) -> Self {
        let mut scratch = Self::default();
        scratch.fit(counts.visits.len(), counts.cover.len());
        scratch
    }

    fn fit(&mut self, anchors: usize, cells: usize) {
        if self.taken_visits.len() != anchors {
            self.taken_visits = vec![0; anchors];
        }
        if self.taken_cover.len() != cells {
            self.taken_cover = vec![0; cells];
        }
    }

    /// Takes one of anchor `i`'s `visits`. True if that was its last one.
    fn take_visit(&mut self, i: usize, visits: u16) -> bool {
        let taken = &mut self.taken_visits[i];
        if *taken >= visits {
            return false;
        }

        if *taken == 0 {
            self.touched_anchors.push(i);
        }
        *taken += 1;
        *taken == visits
    }

    /// Takes one of cell `i`'s `cover`. True if that was its last one.
    fn take_cover(&mut self, i: usize, cover: u16) -> bool {
        if cover == OUTSIDE || cover == 0 {
            return false;
        }

        let taken = &mut self.taken_cover[i];
        if *taken == 0 {
            self.touched_cells.push(i);
        }
        *taken += 1;
        *taken == cover
    }

    fn reset(&mut self) {
        for i in self.touched_anchors.drain(..) {
            self.taken_visits[i] = 0;
        }
        for i in self.touched_cells.drain(..) {
            self.taken_cover[i] = 0;
        }
    }
}
