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
use crate::runs::{runs, Strips};

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
    /// The cell at cover index 0.
    cover_origin: Point,

    /// Cover index of each cutter cell, relative to the cover index of the
    /// cutter box's low corner.
    footprint: Vec<usize>,
    /// The footprint as row and column strips, for stamping runs.
    strips: Strips,
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

        let footprint: Vec<usize> = problem
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
            cover_origin: Point::new(cover_box.x_min, cover_box.y_min),
            strips: Strips::new(&footprint, cover_width),
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
        let mut first_visits = Vec::new();
        let mut result = Ok(());

        for p in positions {
            let Some(i) = self.anchor(p) else { continue };

            let before = self.visits[i];
            let Some(after) = before.checked_add(1) else {
                result = Err(CoverageError::VisitOverflow(p));
                break;
            };
            self.visits[i] = after;
            if before == 0 {
                first_visits.push(i);
            }
        }

        self.stamp_up(&first_visits);
        result
    }

    /// Visits each position once less. On error the positions before the
    /// bad one stay removed.
    pub fn remove_positions(
        &mut self,
        positions: impl IntoIterator<Item = Point>,
    ) -> Result<(), CoverageError> {
        self.remove(positions, &mut None)
    }

    /// As [`Self::remove_positions`], returning the region cells it left
    /// unmowed, each once, row major. With every cell mowed before, these
    /// are what [`Self::unswept_around`] lists after, found without a second
    /// pass over the footprints.
    pub fn remove_positions_baring(
        &mut self,
        positions: impl IntoIterator<Item = Point>,
    ) -> Result<Vec<Point>, CoverageError> {
        let mut bared = Some(Vec::new());
        self.remove(positions, &mut bared)?;

        let mut bared = bared.unwrap_or_default();
        bared.sort_unstable();

        let width = self.cover_width as i32;
        Ok(bared
            .into_iter()
            .map(|c| self.cover_origin.shifted(c as i32 % width, c as i32 / width))
            .collect())
    }

    /// Removes one visit of each position, pushing the cover index of each
    /// cell whose cover runs out to `bared` if given. A cover runs out
    /// once per removal, so no index is pushed twice.
    fn remove(
        &mut self,
        positions: impl IntoIterator<Item = Point>,
        bared: &mut Option<Vec<usize>>,
    ) -> Result<(), CoverageError> {
        let mut last_visits = Vec::new();
        let mut result = Ok(());

        for p in positions {
            let Some(i) = self.anchor(p) else { continue };

            let Some(after) = self.visits[i].checked_sub(1) else {
                result = Err(CoverageError::NotVisited(p));
                break;
            };
            self.visits[i] = after;
            if after == 0 {
                last_visits.push(i);
            }
        }

        self.stamp_down(&last_visits, bared);
        result
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

    /// The region cells of `cells` left unmowed now, in the order given.
    ///
    /// After [`Self::remove_positions_baring`] only the cells it returned
    /// can be unmowed, however many positions are added back, so asking
    /// about them alone matches [`Self::unswept_around`] the removed
    /// positions, without walking their footprints again.
    pub fn unswept_of(&self, cells: &[Point]) -> Vec<Point> {
        cells.iter().copied().filter(|&c| self.is_bare(c)).collect()
    }

    /// True if every region cell of `cells` is mowed now.
    pub fn all_swept_of(&self, cells: &[Point]) -> bool {
        !cells.iter().any(|&c| self.is_bare(c))
    }

    /// A region cell no visited position mows. Cells off the region never are.
    fn is_bare(&self, c: Point) -> bool {
        let (x, y) = (c.x - self.cover_origin.x, c.y - self.cover_origin.y);
        if x < 0 || y < 0 || x as usize >= self.cover_width {
            return false;
        }

        let i = y as usize * self.cover_width + x as usize;
        self.cover.get(i) == Some(&0)
    }

    /// The region cells left unmowed in the footprints of the unvisited
    /// `positions`, each once, row major. Empty exactly when
    /// [`Self::all_swept_around`] is true.
    pub fn unswept_around(&self, positions: impl IntoIterator<Item = Point>) -> Vec<Point> {
        let mut bases: Vec<usize> = positions
            .into_iter()
            .filter_map(|p| self.anchor(p))
            .filter(|&i| self.visits[i] == 0)
            .map(|i| self.cover_base(i))
            .collect();
        bases.sort_unstable();
        bases.dedup();

        let Some(area) = Area::around(&bases, &self.footprint, self.cover_width) else {
            return Vec::new();
        };
        let mut bare = vec![false; area.cells()];

        for &base in &bases {
            for &d in &self.footprint {
                if self.cover[base + d] == 0 {
                    bare[area.local(base + d)] = true;
                }
            }
        }

        let width = self.cover_width as i32;
        (0..bare.len())
            .filter(|&l| bare[l])
            .map(|l| {
                let c = area.cover(l) as i32;
                self.cover_origin.shifted(c % width, c / width)
            })
            .collect()
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

    /// Stamps the footprints of the anchors `first_visits`, distinct, a
    /// straight run of them at a time.
    fn stamp_up(&mut self, first_visits: &[usize]) {
        let Self {
            cover,
            strips,
            uncovered,
            ..
        } = self;

        for run in runs(first_visits, self.frame_width) {
            let base = cover_base(run.first, self.frame_width, self.cover_width);
            strips.for_each_cell(run, base, |c, n| {
                let cover = &mut cover[c];
                if *cover != OUTSIDE {
                    *uncovered -= usize::from(*cover == 0);
                    *cover += n;
                }
            });
        }
    }

    /// Lifts the footprints of the anchors `last_visits`, distinct, pushing
    /// each cell whose cover runs out to `bared` if given.
    fn stamp_down(&mut self, last_visits: &[usize], bared: &mut Option<Vec<usize>>) {
        let Self {
            cover,
            strips,
            uncovered,
            ..
        } = self;

        for run in runs(last_visits, self.frame_width) {
            let base = cover_base(run.first, self.frame_width, self.cover_width);
            strips.for_each_cell(run, base, |c, n| {
                let cover = &mut cover[c];
                if *cover == OUTSIDE {
                    return;
                }

                *cover -= n;
                if *cover == 0 {
                    *uncovered += 1;
                    if let Some(bared) = bared {
                        bared.push(c);
                    }
                }
            });
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
        cover_base(i, self.frame_width, self.cover_width)
    }
}

/// [`VisitCounts::cover_base`] without the counts, for use while they are
/// borrowed in parts.
const fn cover_base(i: usize, frame_width: usize, cover_width: usize) -> usize {
    (i / frame_width) * cover_width + i % frame_width
}

/// The rectangle of the cover box under some footprints, for marking cells
/// in it densely: each cell once and in row major order, without sorting.
struct Area {
    cover_width: usize,
    row: usize,
    col: usize,
    width: usize,
    height: usize,
}

impl Area {
    /// Around the footprints at the cover indices `bases`, sorted. `None`
    /// for no bases.
    fn around(bases: &[usize], footprint: &[usize], cover_width: usize) -> Option<Self> {
        let (first, last) = (*bases.first()?, *bases.last()?);
        let col = bases.iter().map(|b| b % cover_width).min()?;
        let col_max = bases.iter().map(|b| b % cover_width).max()?;
        let rows = footprint.iter().map(|d| d / cover_width).max()? + 1;
        let cols = footprint.iter().map(|d| d % cover_width).max()? + 1;

        Some(Self {
            cover_width,
            row: first / cover_width,
            col,
            width: col_max - col + cols,
            height: last / cover_width - first / cover_width + rows,
        })
    }

    fn cells(&self) -> usize {
        self.width * self.height
    }

    fn local(&self, cover: usize) -> usize {
        let (row, col) = (cover / self.cover_width, cover % self.cover_width);
        (row - self.row) * self.width + (col - self.col)
    }

    fn cover(&self, local: usize) -> usize {
        let (row, col) = (local / self.width, local % self.width);
        (row + self.row) * self.cover_width + col + self.col
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
