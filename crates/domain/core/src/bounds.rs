//! Lower bounds on the objective, as in `solver/bounds.py`. No tour can be
//! shorter, so `(ours - bound) / bound` is the most room left.

use crate::problem::Problem;

/// Each cutter starts with one footprint and every unit step adds at most
/// `step_gain` new cells:
///
/// ```text
/// k |C| + k L gain >= cells   =>   L >= (cells - k |C|) / (k gain)
/// ```
///
/// Closed walks on the grid have even length, so this rounds up to even.
pub fn area_bound(problem: &Problem) -> u64 {
    let cutter = problem.cutter();

    sweep_bound(problem, cutter.area() as u64, u64::from(cutter.step_gain()))
}

/// The area bound's formula for a cutter of `footprint` cells that newly
/// sweeps at most `gain` cells per unit step.
pub(crate) fn sweep_bound(problem: &Problem, footprint: u64, gain: u64) -> u64 {
    let k = problem.k() as u64;

    let remaining = (problem.num_cells() as u64).saturating_sub(k * footprint);
    if remaining == 0 || gain == 0 {
        return 0;
    }

    let per_cutter = remaining.div_ceil(k * gain);
    per_cutter + per_cutter % 2
}

/// A single cutter must reach the region's extreme cells and come back.
/// Only valid for `k == 1`.
pub fn extent_bound(problem: &Problem) -> u64 {
    if problem.k() != 1 {
        return 0;
    }

    let r = problem.region_bounds();
    let c = problem.cutter().bounds();

    let span_x = (i64::from(r.x_max - c.x_max) - i64::from(r.x_min - c.x_min)).max(0) as u64;
    let span_y = (i64::from(r.y_max - c.y_max) - i64::from(r.y_min - c.y_min)).max(0) as u64;

    2 * (span_x + span_y)
}

pub fn lower_bound(problem: &Problem) -> u64 {
    area_bound(problem).max(extent_bound(problem))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::problem::tests::rectangle;

    #[test]
    fn rectangle_bounds_match_the_python_formulas() {
        // 30 x 9 cells, square of side 3: (270 - 9) / 3 = 87, rounded up to even 88
        let p = rectangle(30, 9, 3, 1);

        assert_eq!(area_bound(&p), 88);
        assert_eq!(extent_bound(&p), 2 * (27 + 6));
        assert_eq!(lower_bound(&p), 88);
    }

    #[test]
    fn extent_bound_only_for_one_cutter() {
        let p = rectangle(30, 9, 3, 2);

        assert_eq!(extent_bound(&p), 0);
        // (270 - 18) / 6 = 42, already even
        assert_eq!(area_bound(&p), 42);
    }

    #[test]
    fn tiny_region_needs_no_travel() {
        assert_eq!(area_bound(&rectangle(2, 2, 3, 1)), 0);
    }
}
