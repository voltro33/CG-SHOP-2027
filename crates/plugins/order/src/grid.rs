//! Buckets of points on a square grid, for nearest point queries that do not
//! look at every point.
//!
//! The points of one bucket sit together in one array, with the live ones
//! first, so a point is removed by a swap and nothing is allocated after the
//! grid is built.

use mow_core::Point;

/// About this many points per bucket.
const POINTS_PER_BUCKET: u64 = 2;

pub(crate) struct EndGrid {
    x0: i64,
    y0: i64,
    cell: i64,
    cols: usize,
    rows: usize,

    // bucket b holds items[first[b] .. first[b] + live[b]]
    first: Vec<u32>,
    live: Vec<u32>,
    items: Vec<u32>,

    // where each point sits in `items`, and which bucket it is in
    slot: Vec<u32>,
    bucket: Vec<u32>,

    points: Vec<Point>,
}

impl EndGrid {
    /// A grid over `points`, all live. Point ids are their indices.
    pub(crate) fn new(points: Vec<Point>) -> Self {
        let (x0, y0, width, height) = bounding_box(&points);
        let (cell, cols, rows) = shape(width, height, points.len() as u64);

        let mut grid = Self {
            x0,
            y0,
            cell,
            cols,
            rows,
            first: vec![0; cols * rows + 1],
            live: vec![0; cols * rows],
            items: vec![0; points.len()],
            slot: vec![0; points.len()],
            bucket: vec![0; points.len()],
            points,
        };
        grid.fill();

        grid
    }

    /// Counting sort of the points into their buckets.
    fn fill(&mut self) {
        for id in 0..self.points.len() {
            let (cx, cy) = self.cell_of(self.points[id]);
            let b = cy * self.cols + cx;
            self.bucket[id] = b as u32;
            self.live[b] += 1;
        }

        for b in 0..self.live.len() {
            self.first[b + 1] = self.first[b] + self.live[b];
        }

        let mut next = self.first.clone();
        for id in 0..self.points.len() {
            let b = self.bucket[id] as usize;
            self.items[next[b] as usize] = id as u32;
            self.slot[id] = next[b];
            next[b] += 1;
        }
    }

    pub(crate) fn point(&self, id: u32) -> Point {
        self.points[id as usize]
    }

    /// The side of a bucket: a point in ring `r + 1` around a query is more
    /// than `r * cell` away from it.
    pub(crate) fn cell(&self) -> i64 {
        self.cell
    }

    /// Takes a point out of its bucket. Removing it twice does nothing.
    pub(crate) fn remove(&mut self, id: u32) {
        let b = self.bucket[id as usize] as usize;
        let start = self.first[b];
        let at = self.slot[id as usize];
        if at >= start + self.live[b] {
            return;
        }

        let last = start + self.live[b] - 1;
        let other = self.items[last as usize];
        self.items.swap(at as usize, last as usize);
        self.slot[other as usize] = at;
        self.slot[id as usize] = last;
        self.live[b] -= 1;
    }

    /// The bucket holding `p`, clamped to the grid.
    pub(crate) fn cell_of(&self, p: Point) -> (usize, usize) {
        let cx = ((i64::from(p.x) - self.x0).max(0) / self.cell) as usize;
        let cy = ((i64::from(p.y) - self.y0).max(0) / self.cell) as usize;

        (cx.min(self.cols - 1), cy.min(self.rows - 1))
    }

    /// The largest ring around `(cx, cy)` that still has buckets in it.
    pub(crate) fn last_ring(&self, (cx, cy): (usize, usize)) -> usize {
        cx.max(self.cols - 1 - cx).max(cy).max(self.rows - 1 - cy)
    }

    /// Calls `visit` with every live point in the buckets at Chebyshev
    /// distance exactly `r` from `(cx, cy)`.
    pub(crate) fn ring(&self, (cx, cy): (usize, usize), r: usize, mut visit: impl FnMut(u32)) {
        let (cx, cy, r) = (cx as i64, cy as i64, r as i64);
        let (cols, rows) = (self.cols as i64, self.rows as i64);

        let x_low = (cx - r).max(0);
        let x_high = (cx + r).min(cols - 1);

        for y in [cy - r, cy + r] {
            if (0..rows).contains(&y) {
                self.row(y, x_low, x_high, &mut visit);
            }
            if r == 0 {
                return;
            }
        }

        for y in (cy - r + 1).max(0)..=(cy + r - 1).min(rows - 1) {
            for x in [cx - r, cx + r] {
                if (0..cols).contains(&x) {
                    self.row(y, x, x, &mut visit);
                }
            }
        }
    }

    fn row(&self, y: i64, x_low: i64, x_high: i64, visit: &mut impl FnMut(u32)) {
        for x in x_low..=x_high {
            let b = (y * self.cols as i64 + x) as usize;
            let start = self.first[b] as usize;
            let end = start + self.live[b] as usize;
            self.items[start..end].iter().for_each(|&id| visit(id));
        }
    }
}

/// `(x0, y0, width, height)` of the smallest box around `points`.
fn bounding_box(points: &[Point]) -> (i64, i64, i64, i64) {
    let Some(first) = points.first() else {
        return (0, 0, 1, 1);
    };

    let (mut x_low, mut y_low) = (i64::from(first.x), i64::from(first.y));
    let (mut x_high, mut y_high) = (x_low, y_low);
    for p in points {
        x_low = x_low.min(i64::from(p.x));
        x_high = x_high.max(i64::from(p.x));
        y_low = y_low.min(i64::from(p.y));
        y_high = y_high.max(i64::from(p.y));
    }

    (x_low, y_low, x_high - x_low + 1, y_high - y_low + 1)
}

/// `(cell, cols, rows)`: square buckets with a few points each on average,
/// and never many more buckets than points.
fn shape(width: i64, height: i64, count: u64) -> (i64, usize, usize) {
    let buckets = (count / POINTS_PER_BUCKET).max(1);
    let area = width as f64 * height as f64;
    let mut cell = ((area / buckets as f64).sqrt().ceil() as i64).max(1);

    loop {
        let cols = (width + cell - 1) / cell;
        let rows = (height + cell - 1) / cell;
        if (cols as u64) * (rows as u64) <= 4 * buckets + 16 {
            return (cell, cols as usize, rows as usize);
        }
        cell *= 2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(count: i32) -> EndGrid {
        EndGrid::new((0..count).map(|x| Point::new(3 * x, 7)).collect())
    }

    fn ring_ids(grid: &EndGrid, at: Point, r: usize) -> Vec<u32> {
        let mut ids = Vec::new();
        grid.ring(grid.cell_of(at), r, |id| ids.push(id));
        ids.sort_unstable();
        ids
    }

    #[test]
    fn rings_cover_every_point_once() {
        let grid = line(50);
        let center = grid.cell_of(Point::new(60, 7));

        let mut all = Vec::new();
        for r in 0..=grid.last_ring(center) {
            grid.ring(center, r, |id| all.push(id));
        }
        all.sort_unstable();

        assert_eq!(all, (0..50).collect::<Vec<_>>());
    }

    #[test]
    fn removed_points_are_not_visited() {
        let mut grid = line(4);
        grid.remove(2);
        grid.remove(2);

        let mut all = Vec::new();
        for r in 0..=grid.last_ring((0, 0)) {
            grid.ring((0, 0), r, |id| all.push(id));
        }
        all.sort_unstable();

        assert_eq!(all, vec![0, 1, 3]);
        assert_eq!(grid.point(3), Point::new(9, 7));
    }

    #[test]
    fn ring_zero_is_the_own_bucket() {
        let grid = EndGrid::new((0..20).map(|i| Point::new(50 * i, 50 * i)).collect());
        let own = ring_ids(&grid, Point::new(0, 0), 0);

        assert!(own.contains(&0) && !own.contains(&19));
        assert!(grid.cell() >= 1);
    }

    #[test]
    fn empty_grids_are_harmless() {
        let grid = EndGrid::new(Vec::new());

        assert_eq!(grid.last_ring((0, 0)), 0);
        assert!(ring_ids(&grid, Point::new(5, 5), 0).is_empty());
    }
}
