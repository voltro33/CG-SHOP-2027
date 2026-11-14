//! 2-opt on corners: edges `i -> i + 1` and `j -> j + 1` go, the corners
//! between are driven the other way, and two shortest ways reconnect
//! `i -> j` and `i + 1 -> j + 1`. A lane run is entered from its other end.
//!
//! The reversed run keeps its positions except its two ends: its old
//! positions end at `j`, its new ones at `i + 1`.

use mow_core::Point;
use mow_coverage::CoverageError;

use super::Ctx;
use crate::path::bends;

/// The first 2-opt with edge `i -> i + 1` that saves steps and keeps every
/// cell swept, applied. Returns the steps saved.
pub fn two_opt(corners: &mut Vec<Point>, i: usize, ctx: &mut Ctx) -> Result<u64, CoverageError> {
    let n = corners.len();
    if n < 4 || i + 1 >= n {
        return Ok(0);
    }

    let (a, b) = (corners[i], corners[i + 1]);
    for j in i + 2..n {
        let (c, d) = (corners[j], corners[(j + 1) % n]);
        if (j + 1) % n == i {
            continue;
        }

        let old = a.l1(b) + c.l1(d);
        let new = a.l1(c) + b.l1(d);
        if new >= old {
            continue;
        }

        if let Some((first, second)) = reconnect(a, b, c, d, ctx)? {
            apply(corners, i, j, first, second);
            return Ok(old - new);
        }
    }

    Ok(0)
}

/// The first pair of bends for `a -> c` and `b -> d` that keeps every cell
/// swept, applied to the counts.
fn reconnect(a: Point, b: Point, c: Point, d: Point, ctx: &mut Ctx) -> Result<Option<Bends>, CoverageError> {
    for first in bends(a, c) {
        for second in bends(b, d) {
            ctx.edit.clear();
            ctx.edit.remove_path(&[a, b]);
            ctx.edit.remove_point(c);
            ctx.edit.remove_path(&[c, d]);

            ctx.edit.add_path(&way(a, first, c));
            ctx.edit.add_point(b);
            ctx.edit.add_path(&way(b, second, d));

            ctx.tries += 1;
            if ctx.edit.try_on(ctx.counts)? {
                return Ok(Some((first, second)));
            }
        }
    }

    Ok(None)
}

/// The bends of the ways `i -> j` and `i + 1 -> j + 1`.
type Bends = (Option<Point>, Option<Point>);

fn way(from: Point, bend: Option<Point>, to: Point) -> Vec<Point> {
    std::iter::once(from).chain(bend).chain([to]).collect()
}

/// `0..=i`, the first bend, `j` down to `i + 1`, the second bend, the rest.
fn apply(corners: &mut Vec<Point>, i: usize, j: usize, first: Option<Point>, second: Option<Point>) {
    let mut result = Vec::with_capacity(corners.len() + 2);

    result.extend_from_slice(&corners[..=i]);
    result.extend(first);
    result.extend(corners[i + 1..=j].iter().rev());
    result.extend(second);
    result.extend_from_slice(&corners[j + 1..]);

    *corners = result;
}
