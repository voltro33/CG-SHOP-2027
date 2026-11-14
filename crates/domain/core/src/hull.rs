//! The hull bound: the area bound for the cutter's orthogonal hull.
//!
//! Any superset `C'` of the cutter sweeps at least what the cutter sweeps, so
//! a tour of length `L` sweeps at most `|C'| + L gain(C')` cells and the area
//! formula holds with `C'` in place of `C`. A unit step gains one cell per
//! run across the step, so the best `C'` has one run per row and per column:
//! the orthogonal hull, gaining `max(width, height)`. For the X cutter that is
//! its height (20 for the 20 high X) instead of `step_gain` (32): the trailing
//! arm re-sweeps what the leading arm swept, on any walk.

use crate::bounds::sweep_bound;
use crate::cutter::Cutter;
use crate::grid::BitGrid;
use crate::problem::Problem;

/// The area bound with the cutter replaced by its orthogonal hull. Never
/// below the area bound's gain, but its footprint term is larger, so take the
/// larger of the two.
pub fn hull_bound(problem: &Problem) -> u64 {
    let hull = orthogonal_hull(problem.cutter());

    sweep_bound(problem, hull.area() as u64, u64::from(hull.step_gain()))
}

/// The smallest superset of the cutter whose rows and columns are each one
/// run: gaps filled row by row, then column by column, until nothing changes.
pub fn orthogonal_hull(cutter: &Cutter) -> Cutter {
    let mut cells = cutter.cells().clone();

    loop {
        let filled = fill_columns(&fill_rows(&cells));
        if filled.len() == cells.len() {
            break;
        }
        cells = filled;
    }

    Cutter::new(cells).expect("a hull keeps the cutter's cells")
}

/// Each row filled from its first cell to its last.
fn fill_rows(cells: &BitGrid) -> BitGrid {
    let mut filled = cells.clone();

    let runs: Vec<_> = cells.runs().collect();
    for pair in runs.windows(2) {
        let (left, right) = (pair[0], pair[1]);
        if left.y == right.y {
            filled.insert_run(left.y, left.x_max, right.x_min);
        }
    }

    filled
}

/// Each column filled from its lowest cell to its highest.
fn fill_columns(cells: &BitGrid) -> BitGrid {
    fill_rows(&cells.transposed()).transposed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bounds::area_bound;
    use crate::geom::Point;
    use crate::problem::tests::rectangle;

    fn cutter(cells: &[(i32, i32)]) -> Cutter {
        let points: Vec<Point> = cells.iter().map(|&c| c.into()).collect();
        Cutter::new(BitGrid::from_cells(&points).unwrap().unwrap()).unwrap()
    }

    /// The X of side 5: both diagonals.
    fn x_five() -> Cutter {
        let cells: Vec<(i32, i32)> = (-2..=2).flat_map(|i| [(i, i), (i, -i)]).collect();
        cutter(&cells)
    }

    #[test]
    fn x_hull_is_its_box() {
        let hull = orthogonal_hull(&x_five());

        assert_eq!(x_five().area(), 9);
        assert_eq!(x_five().step_gain(), 9);
        assert_eq!(hull.area(), 25);
        assert_eq!(hull.step_gain(), 5);
    }

    #[test]
    fn convex_cutters_are_their_own_hull() {
        let plus = cutter(&[(0, -1), (-1, 0), (0, 0), (1, 0), (0, 1)]);

        assert_eq!(orthogonal_hull(&plus), plus);
    }

    #[test]
    fn hull_needs_more_than_one_pass() {
        // an L of two bars with a gap: rows fill nothing, columns fill the
        // gap, then the row of the corner fills
        let c = cutter(&[(0, 0), (0, 2), (1, 2), (2, 2), (2, 1)]);
        let hull = orthogonal_hull(&c);

        assert!(hull.cells().contains(Point::new(0, 1)));
        assert!(hull.cells().contains(Point::new(1, 1)));
        assert_eq!(hull.area(), 7);
    }

    /// Cells an X sweeps along a pseudo random closed walk of `2 * half`
    /// steps: out along random directions, then back the same way.
    fn swept_by_random_walk(seed: u64, half: usize) -> usize {
        let x = x_five();
        let mut state = seed;
        let mut p = Point::new(0, 0);
        let mut moves = Vec::new();
        let mut swept = std::collections::HashSet::new();

        for _ in 0..half {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            moves.push([(1, 0), (-1, 0), (0, 1), (0, -1)][(state >> 33) as usize % 4]);
        }
        let back: Vec<_> = moves.iter().rev().map(|&(dx, dy)| (-dx, -dy)).collect();

        for (dx, dy) in moves.into_iter().chain(back) {
            swept.extend(x.offsets().iter().map(|o| o.shifted(p.x, p.y)));
            p = p.shifted(dx, dy);
        }

        swept.len()
    }

    #[test]
    fn no_walk_sweeps_more_than_the_hull_allows() {
        let hull = orthogonal_hull(&x_five());

        for seed in 0..200 {
            let half = 1 + seed as usize % 40;
            let allowed = hull.area() + 2 * half * hull.step_gain() as usize;

            assert!(swept_by_random_walk(seed, half) <= allowed, "seed {seed}");
        }
    }

    #[test]
    fn hull_bound_equals_area_bound_for_a_square() {
        let p = rectangle(30, 9, 3, 1);

        assert_eq!(hull_bound(&p), area_bound(&p));
    }

    #[test]
    fn hull_bound_beats_area_bound_for_an_x() {
        let square = rectangle(40, 40, 5, 1);
        let p = Problem::new("x", square.region().clone(), x_five(), 1).unwrap();

        // area: (1600 - 9) / 9 = 176.8 -> 178; hull: (1600 - 25) / 5 = 315 -> 316
        assert_eq!(area_bound(&p), 178);
        assert_eq!(hull_bound(&p), 316);
    }
}
