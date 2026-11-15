//! Repair anchors: single stops for the cells a move's new walks still miss,
//! each placed to sweep as many of them as it can.

use std::cmp::Reverse;

use mow_core::{Bounds, Cutter, Point, Problem};

/// Anchors tried per missed cell besides the nearest: an even spread of
/// the cutter's offsets, so a large cutter costs a bounded count.
const ANCHOR_CANDIDATES: usize = 32;

/// Anchors sweeping every cell of `missing`, taken in order: for each cell
/// not yet swept, of the anchors that sweep it, the one sweeping the most
/// missed cells still left, the nearest among those to the point of `near`
/// nearest the cell.
///
/// The nearest anchor alone puts the cell on the cutter's far edge, the
/// footprint back over swept ground: with a 60 x 60 cutter that took one
/// anchor per 14 missed cells.
pub fn anchors_for(problem: &Problem, missing: &[Point], near: &[Point]) -> Vec<Point> {
    let cutter = problem.cutter();
    let offsets = cutter.offsets();
    let stride = (offsets.len() / ANCHOR_CANDIDATES).max(1);
    let strips = Strip::all(cutter);

    let Some(mut left) = Missed::new(missing) else {
        return Vec::new();
    };
    let mut picked = Vec::new();

    for &cell in missing {
        if !left.contains(cell) {
            continue;
        }

        let target = near.iter().copied().min_by_key(|p| p.l1(cell)).unwrap_or(cell);
        let nearest = offsets
            .iter()
            .map(|o| cell.shifted(-o.x, -o.y))
            .min_by_key(|a| a.l1(target))
            .unwrap_or(cell);
        let spread = offsets.iter().step_by(stride).map(|o| cell.shifted(-o.x, -o.y));

        let anchor = std::iter::once(nearest)
            .chain(spread)
            .max_by_key(|&a| (left.count_under(a, &strips), Reverse(a.l1(target))))
            .unwrap_or(cell);

        left.sweep(anchor, &strips);
        picked.push(anchor);
    }

    picked
}

/// The cutter cells of one row, `dy` above the center, from `low` to
/// `high` across: a row with a gap (the X cutter) is two strips.
#[derive(Debug, Clone, Copy)]
struct Strip {
    dy: i32,
    low: i32,
    high: i32,
}

impl Strip {
    /// The cutter as strips, row by row.
    fn all(cutter: &Cutter) -> Vec<Self> {
        let mut offsets = cutter.offsets().to_vec();
        offsets.sort_unstable_by_key(|o| (o.y, o.x));

        let mut strips: Vec<Self> = Vec::new();
        for o in offsets {
            match strips.last_mut() {
                Some(s) if s.dy == o.y && s.high + 1 == o.x => s.high = o.x,
                _ => strips.push(Self {
                    dy: o.y,
                    low: o.x,
                    high: o.x,
                }),
            }
        }

        strips
    }
}

/// The missed cells not swept yet, as a bitmap over their bounding box,
/// with a running count along each row so that the cells left under a
/// strip are two lookups.
struct Missed {
    bounds: Bounds,
    width: usize,
    left: Vec<bool>,
    /// Per row, `width + 1` entries: the cells left before each column.
    before: Vec<u32>,
}

impl Missed {
    /// `None` for no cells.
    fn new(cells: &[Point]) -> Option<Self> {
        let first = *cells.first()?;
        let start = Bounds {
            x_min: first.x,
            y_min: first.y,
            x_max: first.x,
            y_max: first.y,
        };
        let bounds = cells.iter().fold(start, |b, c| Bounds {
            x_min: b.x_min.min(c.x),
            y_min: b.y_min.min(c.y),
            x_max: b.x_max.max(c.x),
            y_max: b.y_max.max(c.y),
        });

        let width = bounds.width() as usize;
        let height = bounds.height() as usize;
        let mut missed = Self {
            bounds,
            width,
            left: vec![false; width * height],
            before: vec![0; (width + 1) * height],
        };
        for &c in cells {
            if let Some(i) = missed.index(c) {
                missed.left[i] = true;
            }
        }
        (0..height).for_each(|row| missed.recount(row));

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
    fn count_under(&self, anchor: Point, strips: &[Strip]) -> usize {
        strips
            .iter()
            .filter_map(|s| self.span(anchor, *s))
            .map(|(row, from, to)| {
                let line = &self.before[row * (self.width + 1)..];
                (line[to + 1] - line[from]) as usize
            })
            .sum()
    }

    fn sweep(&mut self, anchor: Point, strips: &[Strip]) {
        for strip in strips {
            let Some((row, from, to)) = self.span(anchor, *strip) else {
                continue;
            };

            self.left[row * self.width + from..=row * self.width + to].fill(false);
            self.recount(row);
        }
    }

    /// The row and the columns, clipped to the bounds, that `strip` of the
    /// cutter at `anchor` covers. `None` if it misses the bounds.
    fn span(&self, anchor: Point, strip: Strip) -> Option<(usize, usize, usize)> {
        let b = self.bounds;
        let y = anchor.y + strip.dy;
        let from = (anchor.x + strip.low).max(b.x_min);
        let to = (anchor.x + strip.high).min(b.x_max);
        if y < b.y_min || y > b.y_max || from > to {
            return None;
        }

        Some((
            (y - b.y_min) as usize,
            (from - b.x_min) as usize,
            (to - b.x_min) as usize,
        ))
    }

    fn recount(&mut self, row: usize) {
        let cells = &self.left[row * self.width..(row + 1) * self.width];
        let line = &mut self.before[row * (self.width + 1)..(row + 1) * (self.width + 1)];

        let mut running = 0;
        for (x, &left) in cells.iter().enumerate() {
            line[x] = running;
            running += u32::from(left);
        }
        line[self.width] = running;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::rectangle;
    use mow_core::BitGrid;
    use std::collections::HashSet;

    fn block(x: i32, y: i32) -> Vec<Point> {
        (0..3)
            .flat_map(|dy| (0..3).map(move |dx| Point::new(x + dx, y + dy)))
            .collect()
    }

    #[test]
    fn anchors_sweep_every_missing_cell_with_few_stops() {
        let anchors = anchors_for(&rectangle(20, 20, 3), &block(10, 10), &[Point::new(11, 11)]);

        assert_eq!(anchors, vec![Point::new(11, 11)]);
    }

    #[test]
    fn anchors_prefer_covering_more_over_standing_nearer() {
        let anchors = anchors_for(&rectangle(20, 20, 3), &block(10, 10), &[Point::new(0, 0)]);

        assert_eq!(anchors, vec![Point::new(11, 11)]);
    }

    #[test]
    fn anchors_sweep_every_missing_cell() {
        let p = rectangle(20, 20, 3);
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
    fn counts_under_a_gapped_cutter_match_a_cell_by_cell_count() {
        // an X: two strips on every row but the middle one
        let x: Vec<Point> = [
            (-2, -2),
            (2, -2),
            (-1, -1),
            (1, -1),
            (0, 0),
            (-1, 1),
            (1, 1),
            (-2, 2),
            (2, 2),
        ]
        .map(Point::from)
        .to_vec();
        let cutter = Cutter::new(BitGrid::from_cells(&x).unwrap().unwrap()).unwrap();
        let strips = Strip::all(&cutter);
        assert_eq!(strips.len(), 9);

        let missing: Vec<Point> = (0..40).map(|i| Point::new((i * 7) % 11, (i * 5) % 9)).collect();
        let mut left = Missed::new(&missing).unwrap();
        let mut reference: HashSet<Point> = missing.iter().copied().collect();

        for anchor in [(3, 3), (5, 1), (0, 8), (9, 4), (12, 12), (6, 6)].map(Point::from) {
            let expected = x
                .iter()
                .filter(|o| reference.contains(&anchor.shifted(o.x, o.y)))
                .count();
            assert_eq!(left.count_under(anchor, &strips), expected, "at {anchor:?}");

            left.sweep(anchor, &strips);
            x.iter().for_each(|o| {
                reference.remove(&anchor.shifted(o.x, o.y));
            });
            assert!(missing
                .iter()
                .all(|&c| left.contains(c) == reference.contains(&c)));
        }
    }
}
