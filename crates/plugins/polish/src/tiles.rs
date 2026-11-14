//! The windows a polishing pass goes through: `_tiles`, `_aimed` and
//! `_grid_ids` of `solver/polish.py`.

use rand::seq::SliceRandom;
use rand::Rng;

use mow_core::{Point, Problem};

use crate::ids::{exclusive_cells, AnchorIds};
use crate::window::Window;

/// Square windows of `side` anchors over the bounding box of a tour, the
/// grid moved left and down by a random offset below `side`, in random
/// order. Windows the tour never enters are left out.
pub fn tiles(positions: &[Point], side: i32, rng: &mut impl Rng) -> Vec<Window> {
    let offset = rng.gen_range(0..side.max(1));

    let mut tiles = tiles_at(positions, side, offset);
    tiles.shuffle(rng);

    tiles
}

/// The tiles of [`tiles`] for a given offset, before the shuffle: sorted by
/// column, then row, as Python sorts its `(column, row)` pairs.
pub fn tiles_at(positions: &[Point], side: i32, offset: i32) -> Vec<Window> {
    let Some(&first) = positions.first() else {
        return Vec::new();
    };

    let corner = positions
        .iter()
        .fold(first, |l, p| Point::new(l.x.min(p.x), l.y.min(p.y)));
    let low = corner.shifted(-offset, -offset);

    let mut visited: Vec<(i32, i32)> = positions
        .iter()
        .map(|p| ((p.x - low.x).div_euclid(side), (p.y - low.y).div_euclid(side)))
        .collect();
    visited.sort_unstable();
    visited.dedup();

    visited
        .into_iter()
        .map(|(column, row)| {
            let (x0, y0) = (low.x + column * side, low.y + row * side);
            Window::new(x0, y0, x0 + side, y0 + side)
        })
        .collect()
}

/// The tiles of the longest tour, most room first; ties keep their order.
/// A tile's room is the longest tour's length inside it, less what the
/// cells only it can sweep for that tour cost at the best rate, the measure
/// of `window_map`. Cells another tour sweeps are not needed, the window
/// keeps those tours. `tiles` are those of [`tiles`] over that tour.
pub fn aimed(problem: &Problem, tours: &[Vec<Point>], longest: usize, tiles: &[Window]) -> Vec<Window> {
    let Some(grid) = Grid::of(tiles) else {
        return Vec::new();
    };

    // every other tour shares one extra window id, so its cells are never exclusive
    let others = grid.rows * grid.columns;
    let windows = others + 1;

    let Some(mut ids) = AnchorIds::over(tours) else {
        return tiles.to_vec();
    };

    let longest_ids = grid_ids(&tours[longest], &grid, others);
    for (index, positions) in tours.iter().enumerate() {
        for (at, &p) in positions.iter().enumerate() {
            let id = if index == longest { longest_ids[at] } else { others };
            ids.record(p, id as u32);
        }
    }

    let mut length = vec![0_u64; windows];
    for &id in &longest_ids {
        length[id] += 1;
    }

    let need = exclusive_cells(problem, &ids, windows);
    let gain = f64::from(problem.cutter().step_gain());
    let room = |tile: &Window| {
        let id = grid.id(Point::new(tile.x0, tile.y0)).unwrap_or(others);
        length[id] as f64 - need[id] as f64 / gain
    };

    let mut sorted = tiles.to_vec();
    sorted.sort_by(|a, b| room(b).total_cmp(&room(a)));

    sorted
}

/// The grid the tiles lie on.
struct Grid {
    x: i32,
    y: i32,
    side: i32,
    rows: usize,
    columns: usize,
}

impl Grid {
    fn of(tiles: &[Window]) -> Option<Self> {
        let side = tiles.first()?.width();
        let x = tiles.iter().map(|t| t.x0).min()?;
        let y = tiles.iter().map(|t| t.y0).min()?;

        let columns = tiles.iter().map(|t| t.x0 - x).max()? / side + 1;
        let rows = tiles.iter().map(|t| t.y0 - y).max()? / side + 1;

        Some(Self {
            x,
            y,
            side,
            rows: rows as usize,
            columns: columns as usize,
        })
    }

    /// The id of the tile holding `p`, `None` off the grid.
    fn id(&self, p: Point) -> Option<usize> {
        let column = usize::try_from((p.x - self.x).div_euclid(self.side)).ok()?;
        let row = usize::try_from((p.y - self.y).div_euclid(self.side)).ok()?;

        (column < self.columns && row < self.rows).then_some(row * self.columns + column)
    }
}

/// The tile id of every position; a position off the grid, which tiles over
/// the tour never leave, counts as `others`.
fn grid_ids(positions: &[Point], grid: &Grid, others: usize) -> Vec<usize> {
    positions.iter().map(|&p| grid.id(p).unwrap_or(others)).collect()
}

#[cfg(test)]
mod tests {
    use rand::rngs::SmallRng;
    use rand::SeedableRng;

    use super::*;

    fn square_tour(side: i32) -> Vec<Point> {
        let corners = vec![
            Point::new(0, 0),
            Point::new(side, 0),
            Point::new(side, side),
            Point::new(0, side),
        ];
        mow_core::Route::new(corners).unwrap().positions().collect()
    }

    #[test]
    fn tiles_reach_every_position() {
        let tour = square_tour(30);
        let mut rng = SmallRng::seed_from_u64(3);

        let tiles = tiles(&tour, 8, &mut rng);
        assert!(tour
            .iter()
            .all(|&p| tiles.iter().filter(|t| t.contains(p)).count() == 1));
        assert!(tiles.iter().all(|t| t.width() == 8 && t.height() == 8));
    }

    #[test]
    fn tiles_at_sorts_by_column_then_row() {
        let tour = square_tour(10);
        let tiles = tiles_at(&tour, 8, 2);

        let corners: Vec<(i32, i32)> = tiles.iter().map(|t| (t.x0, t.y0)).collect();
        assert_eq!(corners, vec![(-2, -2), (-2, 6), (6, -2), (6, 6)]);
    }
}
