//! The region seen along one lane direction: its runs, cut to the tiles of
//! that direction, and the lanes over them.
//!
//! Vertical lanes are horizontal lanes of the mirrored problem, so both
//! directions share this code; a view of the mirrored problem keeps the
//! vertical tiles instead of the horizontal ones.

use std::ops::Range;

use mow_core::{Point, Problem, RowExtent, Run};

use crate::rows::{Band, RowRuns};
use crate::segments::{cut_lanes, Span};

use super::tiles::{Directions, TileGrid};

pub(super) struct View {
    rows: RowRuns,
    extents: Vec<RowExtent>,
    band: Band,
    /// The lowest left cell of the region's box, mask index `(0, 0)`.
    origin: Point,
    tiles: TileGrid,
    mirrored: bool,
}

impl View {
    /// `tiles` must be the tile grid as seen in `problem`, so transposed
    /// along with it when `mirrored`.
    pub(super) fn new(problem: &Problem, tiles: TileGrid, mirrored: bool) -> Self {
        let bounds = problem.region_bounds();

        Self {
            rows: RowRuns::new(problem.region()),
            extents: problem.cutter().row_extents().to_vec(),
            band: Band::of(problem.cutter()),
            origin: Point::new(bounds.x_min, bounds.y_min),
            tiles,
            mirrored,
        }
    }

    pub(super) const fn tiles(&self) -> TileGrid {
        self.tiles
    }

    /// `length + segment_cost * band` over the segments of the cells this
    /// view keeps in tile row `t`, cut into lanes together, so lanes run on
    /// across tile borders (`_strip_cost` in Python).
    pub(super) fn strip_cost(&self, t: usize, directions: &Directions, segment_cost: f64) -> f64 {
        let runs = self.kept_runs(self.tiles.rows.slice(t), |_| t, directions);
        if runs.is_empty() {
            return 0.0;
        }

        let spans = self.spans(runs, 0);
        let length: i64 = spans.iter().map(|&(_, a, b)| b - a).sum();

        // the same float operations in the same order as Python
        length as f64 + segment_cost * self.band.height as f64 * spans.len() as f64
    }

    /// The lanes over every cell this view keeps, bottom to top.
    pub(super) fn part_spans(&self, directions: &Directions, max_gap: i64) -> Vec<Span> {
        let all_rows = 0..self.tiles.rows.length;
        let rows = self.tiles.rows;

        let runs = self.kept_runs(all_rows, |r| rows.of(r), directions);
        self.spans(runs, max_gap)
    }

    /// Every tile `(t, s)` some region cell lies in, repeats included.
    pub(super) fn cell_tiles(&self) -> Vec<(usize, usize)> {
        let (x0, y0) = (i64::from(self.origin.x), i64::from(self.origin.y));
        let (rows, columns) = (self.tiles.rows, self.tiles.columns);
        let mut tiles = Vec::new();

        for r in 0..rows.length {
            let t = rows.of(r);

            for run in self.rows.row(y0 + r) {
                let first = columns.of(i64::from(run.x_min) - x0);
                let last = columns.of(i64::from(run.x_max) - x0);
                tiles.extend((first..=last).map(|s| (t, s)));
            }
        }

        tiles
    }

    /// Lane segments of `runs` with lane borders on the tile grid, wherever
    /// the lowest of them lies (`_horizontal_ends` in Python).
    fn spans(&self, runs: Vec<Run>, max_gap: i64) -> Vec<Span> {
        let rows = RowRuns::from_runs(runs);
        let Some((lowest, _)) = rows.span() else {
            return Vec::new();
        };

        // lanes start at the lowest row, an offset moves them onto the grid
        let grid_start = i64::from(self.origin.y) + self.tiles.rows.first;
        let offset = (lowest - grid_start).rem_euclid(self.band.height);

        cut_lanes(&rows, &self.extents, self.band, offset, max_gap)
    }

    /// The region's runs in mask rows `rows`, cut to the tiles this view
    /// keeps. `tile_row` names the tile row whose directions apply to a mask
    /// row. Pieces in neighbouring kept tiles stay one run.
    fn kept_runs(
        &self,
        rows: Range<i64>,
        tile_row: impl Fn(i64) -> usize,
        directions: &Directions,
    ) -> Vec<Run> {
        let (x0, y0) = (i64::from(self.origin.x), i64::from(self.origin.y));
        let mut kept = Vec::new();

        for r in rows {
            let t = tile_row(r);
            let y = (y0 + r) as i32;

            for run in self.rows.row(y0 + r) {
                let low = i64::from(run.x_min) - x0;
                let high = i64::from(run.x_max) - x0;

                for (start, end) in self.kept_pieces(t, low, high, directions) {
                    kept.push(Run {
                        y,
                        x_min: (x0 + start) as i32,
                        x_max: (x0 + end) as i32,
                    });
                }
            }
        }

        kept
    }

    /// The parts of mask columns `low..=high` in tiles this view keeps, in
    /// tile row `t`, joined where they touch.
    fn kept_pieces(&self, t: usize, low: i64, high: i64, directions: &Directions) -> Vec<(i64, i64)> {
        let columns = self.tiles.columns;
        let mut pieces: Vec<(i64, i64)> = Vec::new();

        let mut start = low;
        while start <= high {
            let end = high.min(columns.next_border(start) - 1);

            if self.keeps(t, columns.of(start), directions) {
                match pieces.last_mut() {
                    Some(last) if last.1 + 1 == start => last.1 = end,
                    _ => pieces.push((start, end)),
                }
            }

            start = end + 1;
        }

        pieces
    }

    /// Whether tile `(t, s)` of this view sweeps along this view's lanes.
    fn keeps(&self, t: usize, s: usize, directions: &Directions) -> bool {
        let vertical = if self.mirrored {
            directions.get(s, t)
        } else {
            directions.get(t, s)
        };

        vertical == self.mirrored
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{problem, rectangle};

    #[test]
    fn pieces_of_kept_tiles_join_and_others_are_cut_out() {
        // one cell cutter: lanes one row high, tiles two cells wide
        let p = problem(&rectangle(8, 2), &[(0, 0)]);
        let grid = TileGrid::new(&p, 2, 0.0).unwrap();
        let view = View::new(&p, grid, false);

        let mut directions = Directions::horizontal(grid);
        directions.set(0, 2, true);

        assert_eq!(view.kept_pieces(0, 0, 7, &directions), vec![(0, 3), (6, 7)]);
        assert_eq!(
            view.part_spans(&directions, 0),
            vec![(0, 0, 3), (0, 6, 7), (1, 0, 3), (1, 6, 7)]
        );
    }

    #[test]
    fn mirrored_view_keeps_the_vertical_tiles() {
        let p = problem(&rectangle(8, 2), &[(0, 0)]);
        let grid = TileGrid::new(&p, 2, 0.0).unwrap();
        let view = View::new(&p.transposed(), grid.transposed(), true);

        let mut directions = Directions::horizontal(grid);
        directions.set(0, 2, true);

        // mirrored lanes are the columns 4 and 5, rows 0..=1
        assert_eq!(view.part_spans(&directions, 0), vec![(4, 0, 1), (5, 0, 1)]);
    }

    #[test]
    fn strip_cost_counts_length_and_segments() {
        let p = problem(&rectangle(8, 2), &[(0, 0)]);
        let grid = TileGrid::new(&p, 2, 0.0).unwrap();
        let view = View::new(&p, grid, false);
        let directions = Directions::horizontal(grid);

        // two lanes of length 7, each costing 3 * band 1 more
        assert_eq!(view.strip_cost(0, &directions, 3.0), 14.0 + 6.0);
    }
}
