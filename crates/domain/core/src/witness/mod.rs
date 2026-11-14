//! The witness bound: cells far apart that each need their own visit.
//!
//! Every witness cell `w` must be swept, so some tour visits an anchor in
//! `w - C`. A tour visiting anchors `a_1..a_m` in that order is a cycle at
//! least the sum of the anchor-set distances along it, so any cycle bound over
//! the witnesses with those distances bounds the tour. It counts thin arms and
//! long reaches, which the area bound misses.
//!
//! - `k = 1`: Held and Karp's 1-tree bound.
//! - any `k`: of `k + 1` witnesses two share a tour, which goes from one to
//!   the other and back (twice the smallest gap); and the `k` tours together
//!   are at least the cheapest forest of `k` trees, so the longest is at least
//!   a `k`-th of it.

mod pick;
mod reach;
mod tree;

pub use reach::Reach;

use crate::problem::Problem;
use pick::witnesses;
use tree::{dispersion, forest, held_karp, Distances};

/// Witnesses per instance: enough to trace the border, few enough for the
/// `O(m^2)` trees.
const WITNESSES: usize = 256;

/// Subgradient steps of the 1-tree bound.
const ITERATIONS: usize = 300;

/// Witnesses searched for the `k + 1` farthest apart.
const DISPERSION_LIMIT: usize = 64;

/// The longest tour is at least this long. Even, as every closed walk is.
pub fn witness_bound(problem: &Problem) -> u64 {
    let reach = Reach::new(problem.cutter());
    let points = witnesses(problem, &reach, WITNESSES);
    let d = Distances::new(points.len(), |i, j| reach.between(points[i], points[j]));

    let k = problem.k();
    let tours = if k == 1 {
        held_karp(&d, ITERATIONS)
    } else {
        forest(&d, k) as f64 / k as f64
    };
    let pair = dispersion(&d, k, DISPERSION_LIMIT) as f64;

    even_ceil(tours.max(pair))
}

/// The smallest even integer at least `length`, forgiving rounding noise.
fn even_ceil(length: f64) -> u64 {
    let whole = (length - 1e-6).ceil().max(0.0) as u64;

    whole + whole % 2
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bounds::area_bound;
    use crate::cutter::Cutter;
    use crate::geom::{Bounds, Point};
    use crate::grid::BitGrid;
    use crate::problem::tests::rectangle;

    /// An L of two bars of `arm` cells, `width` wide, mowed by a square.
    fn ell(arm: i32, width: i32, side: i32, k: usize) -> Problem {
        let mut region = BitGrid::empty(Bounds {
            x_min: 0,
            y_min: 0,
            x_max: arm - 1,
            y_max: arm - 1,
        })
        .unwrap();
        for y in 0..arm {
            let x_max = if y < width { arm - 1 } else { width - 1 };
            region.insert_run(y, 0, x_max);
        }

        let cutter = rectangle(1, 1, side, 1).cutter().clone();
        Problem::new("ell", region, cutter, k).unwrap()
    }

    #[test]
    fn a_bar_needs_a_round_trip() {
        // 30 x 3 cells, square of side 3: anchors run x = 1..=28, so 54
        let p = rectangle(30, 3, 3, 1);

        assert_eq!(witness_bound(&p), 54);
        assert!(witness_bound(&p) > area_bound(&p));
    }

    #[test]
    fn an_ell_needs_both_arms() {
        // arms 40 long and 3 wide: out to each tip and back, 2 * (37 + 37)
        let p = ell(40, 3, 3, 1);

        assert_eq!(witness_bound(&p), 148);
    }

    #[test]
    fn more_cutters_split_the_reach() {
        let one = witness_bound(&rectangle(60, 3, 3, 1));
        let two = witness_bound(&rectangle(60, 3, 3, 2));
        let three = witness_bound(&rectangle(60, 3, 3, 3));

        // three witnesses at 0, 29, 59: two of them share a tour, 2 * 27
        assert_eq!(one, 114);
        assert_eq!(two, 54);
        assert!(three < two);
    }

    #[test]
    fn a_blob_inside_one_footprint_needs_nothing() {
        assert_eq!(witness_bound(&rectangle(3, 3, 3, 1)), 0);
    }

    #[test]
    fn x_cutter_reach_counts_its_arms() {
        let cells: Vec<Point> = (-1..=1)
            .flat_map(|i| [Point::new(i, i), Point::new(i, -i)])
            .collect();
        let x = Cutter::new(BitGrid::from_cells(&cells).unwrap().unwrap()).unwrap();
        let p = Problem::new("x", rectangle(20, 1, 1, 1).region().clone(), x, 1).unwrap();

        // the ends are cells 0 and 19 of one row; an X anchor reaches one to
        // either side, so the anchors are 17 apart
        assert_eq!(witness_bound(&p), 34);
    }

    #[test]
    fn even_ceil_rounds_up_to_even() {
        assert_eq!(even_ceil(0.0), 0);
        assert_eq!(even_ceil(53.0000001), 54);
        assert_eq!(even_ceil(54.2), 56);
        assert_eq!(even_ceil(52.5), 54);
    }
}
