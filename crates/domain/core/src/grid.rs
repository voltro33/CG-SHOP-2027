//! Sets of cells (or lattice points) as bitsets over a box.
//!
//! One bit per cell, rows padded to whole `u64` words so a row is a slice.
//! Bit `c % 64` of word `c / 64` in row `r` is the cell
//! `(origin.x + c, origin.y + r)`.

use crate::error::ModelError;
use crate::geom::{Bounds, Point};

/// The most bits a grid may hold, 2 GiB of words. Far above any instance.
const MAX_BITS: u64 = 1 << 34;

const WORD: u32 = u64::BITS;

#[derive(Clone, PartialEq, Eq)]
pub struct BitGrid {
    origin: Point,
    width: u32,
    height: u32,
    stride: usize,
    words: Vec<u64>,
}

/// A maximal horizontal run of cells: `x_min..=x_max` in row `y`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Run {
    pub y: i32,
    pub x_min: i32,
    pub x_max: i32,
}

impl Run {
    pub const fn len(self) -> u32 {
        (self.x_max - self.x_min + 1) as u32
    }

    pub const fn is_empty(self) -> bool {
        false
    }
}

impl BitGrid {
    /// An empty grid over the box `frame`.
    pub fn empty(frame: Bounds) -> Result<Self, ModelError> {
        let width = u64::try_from(i64::from(frame.x_max) - i64::from(frame.x_min) + 1).unwrap_or(0);
        let height = u64::try_from(i64::from(frame.y_max) - i64::from(frame.y_min) + 1).unwrap_or(0);

        let too_large = width == 0 || height == 0 || width * height > MAX_BITS;
        if too_large || width > u64::from(u32::MAX) || height > u64::from(u32::MAX) {
            return Err(ModelError::GridTooLarge { width, height });
        }

        let stride = width.div_ceil(u64::from(WORD)) as usize;
        let words = vec![0; stride * height as usize];

        Ok(Self {
            origin: Point::new(frame.x_min, frame.y_min),
            width: width as u32,
            height: height as u32,
            stride,
            words,
        })
    }

    /// The smallest grid holding exactly `cells`. `None` if there are none.
    pub fn from_cells(cells: &[Point]) -> Result<Option<Self>, ModelError> {
        let Some(frame) = bounding_box(cells) else {
            return Ok(None);
        };

        let mut grid = Self::empty(frame)?;
        for &cell in cells {
            grid.insert(cell);
        }

        Ok(Some(grid))
    }

    // -- shape ---------------------------------------------------------------

    pub const fn origin(&self) -> Point {
        self.origin
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    /// The box the grid can hold, not the box of the cells it holds.
    pub const fn frame(&self) -> Bounds {
        Bounds {
            x_min: self.origin.x,
            y_min: self.origin.y,
            x_max: self.origin.x + self.width as i32 - 1,
            y_max: self.origin.y + self.height as i32 - 1,
        }
    }

    /// The box of the cells the grid holds. `None` if it is empty.
    pub fn cell_bounds(&self) -> Option<Bounds> {
        let mut bounds: Option<Bounds> = None;

        for run in self.runs() {
            let b = bounds.get_or_insert(Bounds {
                x_min: run.x_min,
                y_min: run.y,
                x_max: run.x_max,
                y_max: run.y,
            });
            b.x_min = b.x_min.min(run.x_min);
            b.x_max = b.x_max.max(run.x_max);
            b.y_max = run.y;
        }

        bounds
    }

    // -- membership ----------------------------------------------------------

    pub fn contains(&self, p: Point) -> bool {
        match self.locate(p) {
            Some((index, bit)) => self.words[index] >> bit & 1 == 1,
            None => false,
        }
    }

    /// Adds `p`. Returns false, and changes nothing, if `p` is outside the frame.
    pub fn insert(&mut self, p: Point) -> bool {
        match self.locate(p) {
            Some((index, bit)) => {
                self.words[index] |= 1 << bit;
                true
            }
            None => false,
        }
    }

    pub fn remove(&mut self, p: Point) -> bool {
        match self.locate(p) {
            Some((index, bit)) => {
                self.words[index] &= !(1 << bit);
                true
            }
            None => false,
        }
    }

    /// Adds the cells `x_min..=x_max` of row `y`, clipped to the frame.
    pub fn insert_run(&mut self, y: i32, x_min: i32, x_max: i32) {
        let row = i64::from(y) - i64::from(self.origin.y);
        if row < 0 || row >= i64::from(self.height) {
            return;
        }

        let low = (i64::from(x_min) - i64::from(self.origin.x)).max(0);
        let high = (i64::from(x_max) - i64::from(self.origin.x)).min(i64::from(self.width) - 1);
        if low > high {
            return;
        }

        let row_words = self.row_words_mut(row as u32);
        set_bits(row_words, low as usize, high as usize);
    }

    pub fn len(&self) -> usize {
        self.words.iter().map(|w| w.count_ones() as usize).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.words.iter().all(|&w| w == 0)
    }

    // -- raw rows, for fast passes in other crates --------------------------

    /// Words per row.
    pub const fn stride(&self) -> usize {
        self.stride
    }

    pub fn row_words(&self, row: u32) -> &[u64] {
        let start = row as usize * self.stride;
        &self.words[start..start + self.stride]
    }

    pub fn row_words_mut(&mut self, row: u32) -> &mut [u64] {
        let start = row as usize * self.stride;
        &mut self.words[start..start + self.stride]
    }

    // -- iteration -----------------------------------------------------------

    /// The cells, row by row from the bottom, left to right.
    pub fn cells(&self) -> Cells<'_> {
        Cells {
            grid: self,
            index: 0,
            word: self.words.first().copied().unwrap_or(0),
        }
    }

    /// The maximal horizontal runs, in the same order as [`Self::cells`].
    pub fn runs(&self) -> Runs<'_> {
        Runs {
            cells: self.cells().peekable(),
        }
    }

    // -- new grids -----------------------------------------------------------

    /// The same cells moved by `(dx, dy)`.
    pub fn translated(&self, dx: i32, dy: i32) -> Self {
        Self {
            origin: self.origin.shifted(dx, dy),
            ..self.clone()
        }
    }

    /// Mirrored across the diagonal `x = y`.
    pub fn transposed(&self) -> Self {
        let mut result =
            Self::empty(self.frame().transposed()).expect("a transposed frame has the same size");
        for cell in self.cells() {
            result.insert(cell.transposed());
        }

        result
    }

    /// The cells of `self` not in `other`, over the frame of `self`.
    pub fn difference(&self, other: &Self) -> Self {
        let mut result = self.clone();
        for cell in self.cells().filter(|&c| other.contains(c)) {
            result.remove(cell);
        }

        result
    }

    fn locate(&self, p: Point) -> Option<(usize, u32)> {
        let col = i64::from(p.x) - i64::from(self.origin.x);
        let row = i64::from(p.y) - i64::from(self.origin.y);

        let inside = (0..i64::from(self.width)).contains(&col) && (0..i64::from(self.height)).contains(&row);
        if !inside {
            return None;
        }

        let index = row as usize * self.stride + col as usize / WORD as usize;
        Some((index, col as u32 % WORD))
    }
}

impl std::fmt::Debug for BitGrid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BitGrid")
            .field("frame", &self.frame())
            .field("cells", &self.len())
            .finish()
    }
}

/// Sets bits `low..=high` of a row.
fn set_bits(row: &mut [u64], low: usize, high: usize) {
    let word_size = WORD as usize;
    let (first, last) = (low / word_size, high / word_size);

    for (i, word) in row.iter_mut().enumerate().take(last + 1).skip(first) {
        let from = if i == first { low % word_size } else { 0 };
        let to = if i == last {
            high % word_size
        } else {
            word_size - 1
        };

        let width = to - from + 1;
        let mask = if width == word_size {
            u64::MAX
        } else {
            ((1u64 << width) - 1) << from
        };
        *word |= mask;
    }
}

fn bounding_box(cells: &[Point]) -> Option<Bounds> {
    let first = cells.first()?;
    let start = Bounds {
        x_min: first.x,
        y_min: first.y,
        x_max: first.x,
        y_max: first.y,
    };

    Some(cells.iter().fold(start, |b, p| Bounds {
        x_min: b.x_min.min(p.x),
        y_min: b.y_min.min(p.y),
        x_max: b.x_max.max(p.x),
        y_max: b.y_max.max(p.y),
    }))
}

/// Iterator over the cells of a [`BitGrid`], borrowing it.
pub struct Cells<'a> {
    grid: &'a BitGrid,
    index: usize,
    word: u64,
}

impl Iterator for Cells<'_> {
    type Item = Point;

    fn next(&mut self) -> Option<Point> {
        while self.word == 0 {
            self.index += 1;
            self.word = *self.grid.words.get(self.index)?;
        }

        let bit = self.word.trailing_zeros();
        self.word &= self.word - 1;

        let row = self.index / self.grid.stride;
        let col = (self.index % self.grid.stride) as u32 * WORD + bit;

        Some(self.grid.origin.shifted(col as i32, row as i32))
    }
}

/// Iterator over the maximal horizontal runs of a [`BitGrid`], borrowing it.
pub struct Runs<'a> {
    cells: std::iter::Peekable<Cells<'a>>,
}

impl Iterator for Runs<'_> {
    type Item = Run;

    fn next(&mut self) -> Option<Run> {
        let first = self.cells.next()?;
        let mut run = Run {
            y: first.y,
            x_min: first.x,
            x_max: first.x,
        };

        while let Some(next) = self.cells.next_if(|c| c.y == run.y && c.x == run.x_max + 1) {
            run.x_max = next.x;
        }

        Some(run)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(x_min: i32, y_min: i32, x_max: i32, y_max: i32) -> Bounds {
        Bounds {
            x_min,
            y_min,
            x_max,
            y_max,
        }
    }

    #[test]
    fn insert_contains_and_clip() {
        let mut grid = BitGrid::empty(frame(-2, -1, 70, 1)).unwrap();

        assert!(grid.insert(Point::new(-2, -1)));
        assert!(grid.insert(Point::new(70, 1)));
        assert!(!grid.insert(Point::new(71, 1)));

        assert!(grid.contains(Point::new(70, 1)));
        assert!(!grid.contains(Point::new(0, 0)));
        assert_eq!(grid.len(), 2);
        assert_eq!(grid.stride(), 2);
    }

    #[test]
    fn runs_cross_word_borders_and_rows_stay_apart() {
        let mut grid = BitGrid::empty(frame(0, 0, 199, 1)).unwrap();
        grid.insert_run(0, 60, 130);
        grid.insert_run(0, 140, 140);
        grid.insert_run(1, 0, 0);
        grid.insert_run(1, -5, -1);
        grid.insert_run(2, 0, 10);

        let runs: Vec<Run> = grid.runs().collect();

        assert_eq!(
            runs,
            vec![
                Run {
                    y: 0,
                    x_min: 60,
                    x_max: 130
                },
                Run {
                    y: 0,
                    x_min: 140,
                    x_max: 140
                },
                Run {
                    y: 1,
                    x_min: 0,
                    x_max: 0
                },
            ]
        );
        assert_eq!(grid.len(), 71 + 1 + 1);
    }

    #[test]
    fn full_word_runs_set_every_bit() {
        let mut grid = BitGrid::empty(frame(0, 0, 127, 0)).unwrap();
        grid.insert_run(0, 0, 127);

        assert_eq!(grid.len(), 128);
        assert_eq!(grid.runs().count(), 1);
    }

    #[test]
    fn from_cells_is_tight_and_transpose_round_trips() {
        let cells = [Point::new(3, -1), Point::new(5, 2), Point::new(4, 2)];
        let grid = BitGrid::from_cells(&cells).unwrap().unwrap();

        assert_eq!(grid.frame(), frame(3, -1, 5, 2));
        assert_eq!(grid.cell_bounds(), Some(frame(3, -1, 5, 2)));

        let back = grid.transposed().transposed();
        assert_eq!(back.cells().collect::<Vec<_>>(), grid.cells().collect::<Vec<_>>());
        assert!(grid.transposed().contains(Point::new(-1, 3)));
    }

    #[test]
    fn difference_and_translation() {
        let a = BitGrid::from_cells(&[Point::new(0, 0), Point::new(1, 0)])
            .unwrap()
            .unwrap();
        let b = BitGrid::from_cells(&[Point::new(1, 0), Point::new(9, 9)])
            .unwrap()
            .unwrap();

        assert_eq!(
            a.difference(&b).cells().collect::<Vec<_>>(),
            vec![Point::new(0, 0)]
        );
        assert!(a.translated(-1, 5).contains(Point::new(0, 5)));
    }

    #[test]
    fn empty_inputs() {
        assert!(BitGrid::from_cells(&[]).unwrap().is_none());
        assert!(BitGrid::empty(frame(0, 0, -1, 0)).is_err());
    }
}
