//! Or-opt on corners: a block of 1 to 3 corners (a detour tip, a lane, a
//! lane and its turn) leaves its place and goes between two other corners,
//! either way round. Shortest ways close the gap and connect the block.

use mow_core::Point;
use mow_coverage::CoverageError;

use super::Ctx;
use crate::path::{at, bends};

/// The longest block moved.
const LONGEST_BLOCK: usize = 3;

/// One placement: the block of `len` corners from `start` goes after
/// corner `after`, reversed or not.
#[derive(Debug, Clone, Copy)]
struct Placement {
    start: usize,
    len: usize,
    after: usize,
    reversed: bool,
}

/// The first move of a block starting at `start` that saves steps and keeps
/// every cell swept, applied. Returns the steps saved.
pub fn or_opt(corners: &mut Vec<Point>, start: usize, ctx: &mut Ctx) -> Result<u64, CoverageError> {
    let n = corners.len();

    for len in 1..=LONGEST_BLOCK.min(n.saturating_sub(3)) {
        for offset in len..n - 1 {
            for reversed in [false, true] {
                let placement = Placement {
                    start,
                    len,
                    after: (start + offset) % n,
                    reversed,
                };
                let saved = gain(corners, placement);
                if saved <= 0 {
                    continue;
                }

                if let Some(bends) = try_placement(corners, placement, ctx)? {
                    apply(corners, placement, bends);
                    return Ok(saved as u64);
                }
            }
        }
    }

    Ok(0)
}

/// The corners around a placement: before and after the block, its first
/// and last corner, and the two corners it goes between.
fn ends(corners: &[Point], m: Placement) -> [Point; 6] {
    let n = corners.len();
    let (s, e) = (m.start, m.start + m.len - 1);

    [
        at(corners, s + n - 1),
        at(corners, e + 1),
        at(corners, s),
        at(corners, e),
        at(corners, m.after),
        at(corners, m.after + 1),
    ]
}

fn gain(corners: &[Point], m: Placement) -> i64 {
    let [prev, next, first, last, p, q] = ends(corners, m);
    let (enter, leave) = if m.reversed { (last, first) } else { (first, last) };

    let old = prev.l1(first) + last.l1(next) + p.l1(q);
    let new = prev.l1(next) + p.l1(enter) + leave.l1(q);
    old as i64 - new as i64
}

/// The bends of the way that closes the gap, the way into the block and
/// the way out of it.
type Bends = [Option<Point>; 3];

/// Tries every choice of bends for the three new ways; the first kept one
/// is applied to the counts.
fn try_placement(corners: &[Point], m: Placement, ctx: &mut Ctx) -> Result<Option<Bends>, CoverageError> {
    let [prev, next, first, last, p, q] = ends(corners, m);
    let (enter, leave) = if m.reversed { (last, first) } else { (first, last) };

    for closing in bends(prev, next) {
        for into in bends(p, enter) {
            for out in bends(leave, q) {
                ctx.edit.clear();
                ctx.edit.remove_path(&[prev, first]);
                ctx.edit.remove_path(&[last, next]);
                ctx.edit.remove_path(&[p, q]);
                ctx.edit.add_path(&way(prev, closing, next));
                ctx.edit.add_path(&way(p, into, enter));
                ctx.edit.add_path(&way(leave, out, q));
                if m.reversed {
                    ctx.edit.remove_point(last);
                    ctx.edit.add_point(first);
                }

                ctx.tries += 1;
                if ctx.edit.try_on(ctx.counts)? {
                    return Ok(Some([closing, into, out]));
                }
            }
        }
    }

    Ok(None)
}

fn way(from: Point, bend: Option<Point>, to: Point) -> Vec<Point> {
    std::iter::once(from).chain(bend).chain([to]).collect()
}

/// Rebuilds the tour from the corner after the block: on to `after`, the
/// block between its two bends, the rest up to the corner before the
/// block, and the bend that closes the gap.
fn apply(corners: &mut Vec<Point>, m: Placement, [closing, into, out]: Bends) {
    let n = corners.len();
    corners.rotate_left(m.start);

    let mut block: Vec<Point> = corners[..m.len].to_vec();
    if m.reversed {
        block.reverse();
    }
    let after = (m.after + n - m.start) % n;

    let mut result = Vec::with_capacity(n + 3);
    result.extend_from_slice(&corners[m.len..=after]);
    result.extend(into);
    result.extend(block);
    result.extend(out);
    result.extend_from_slice(&corners[after + 1..]);
    result.extend(closing);

    *corners = result;
}
