//! Transfer, for k > 1: a block of corners leaves the longest tour and goes
//! between two corners of another tour, either way round, when both tours
//! end up shorter than the longest was. This is the balancing the other
//! moves can't do: they only ever shorten one tour.
//!
//! The positions change as in or-opt, only the gap and the new place lie in
//! two tours; the counts hold every tour, so the check is the same.

use mow_core::Point;
use mow_coverage::CoverageError;

use super::Ctx;
use crate::path::{at, bends, closed_length};

/// The longest block moved.
const LONGEST_BLOCK: usize = 6;

/// A block of `len` corners of the giving tour from `start`, to go after
/// corner `after` of the taking tour.
#[derive(Debug, Clone, Copy)]
struct Placement {
    start: usize,
    len: usize,
    after: usize,
    reversed: bool,
}

/// The first transfer from `from` (the longest tour) to `to` that lowers
/// the longer of the two below `from`'s length, applied. Returns how much
/// shorter the longer of the two got.
pub fn transfer(from: &mut Vec<Point>, to: &mut Vec<Point>, ctx: &mut Ctx) -> Result<u64, CoverageError> {
    let (n, m) = (from.len(), to.len());
    let (from_length, to_length) = (closed_length(from), closed_length(to));

    for start in 0..n {
        for len in 1..=LONGEST_BLOCK.min(n.saturating_sub(3)) {
            for after in 0..m {
                for reversed in [false, true] {
                    let placement = Placement {
                        start,
                        len,
                        after,
                        reversed,
                    };
                    let (given, taken) = lengths(from, to, placement, from_length, to_length);
                    if given.max(taken) >= from_length {
                        continue;
                    }

                    if let Some(bends) = try_placement(from, to, placement, ctx)? {
                        apply(from, to, placement, bends);
                        return Ok(from_length - given.max(taken));
                    }
                }
            }
        }
    }

    Ok(0)
}

/// The corners around a placement: before and after the block, its first
/// and last corner, and the two corners of the taking tour it goes between.
fn ends(from: &[Point], to: &[Point], m: Placement) -> [Point; 6] {
    let n = from.len();
    let (s, e) = (m.start, m.start + m.len - 1);

    [
        at(from, s + n - 1),
        at(from, e + 1),
        at(from, s),
        at(from, e),
        at(to, m.after),
        at(to, m.after + 1),
    ]
}

/// The lengths of both tours after the transfer.
fn lengths(from: &[Point], to: &[Point], m: Placement, from_length: u64, to_length: u64) -> (u64, u64) {
    let [prev, next, first, last, p, q] = ends(from, to, m);
    let (enter, leave) = if m.reversed { (last, first) } else { (first, last) };
    let inner: u64 = (m.start..m.start + m.len - 1)
        .map(|t| at(from, t).l1(at(from, t + 1)))
        .sum();

    let given = from_length + prev.l1(next) - prev.l1(first) - inner - last.l1(next);
    let taken = to_length + p.l1(enter) + inner + leave.l1(q) - p.l1(q);
    (given, taken)
}

/// The bends of the way that closes the gap, the way into the block and
/// the way out of it.
type Bends = [Option<Point>; 3];

/// Tries every choice of bends; the first kept one is applied to the counts.
fn try_placement(
    from: &[Point],
    to: &[Point],
    m: Placement,
    ctx: &mut Ctx,
) -> Result<Option<Bends>, CoverageError> {
    let [prev, next, first, last, p, q] = ends(from, to, m);
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

/// Cuts the block out of `from` (closing the gap with its bend) and puts it
/// into `to` after corner `after`, between its two bends.
fn apply(from: &mut Vec<Point>, to: &mut Vec<Point>, m: Placement, [closing, into, out]: Bends) {
    from.rotate_left(m.start);
    let mut block: Vec<Point> = from.drain(..m.len).collect();
    from.extend(closing);
    if m.reversed {
        block.reverse();
    }

    let mut inserted = Vec::with_capacity(block.len() + 2);
    inserted.extend(into);
    inserted.extend(block);
    inserted.extend(out);
    to.splice(m.after + 1..m.after + 1, inserted);
}
