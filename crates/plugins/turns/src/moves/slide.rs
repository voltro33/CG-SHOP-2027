//! Slide: a run of 1 to 3 edges whose two neighbours leave on the same side
//! (a U turn, a lane end connector with a jog, the far side of a lane
//! pair) moves towards them. Each step shortens both neighbours: two steps
//! saved, and the run keeps its shape.
//!
//! The slide grows by doubling while it is kept and halves when it is not,
//! so a long pull costs a few checks, each touching only the moved run.

use mow_core::Point;
use mow_coverage::CoverageError;

use super::Ctx;
use crate::path::at;

/// The most edges one slide moves.
const LONGEST_RUN: usize = 3;

/// Slides the runs starting at corner `i`, shortest first, as far as
/// coverage lets them. Returns the steps saved.
pub fn slide(corners: &mut [Point], i: usize, ctx: &mut Ctx) -> Result<u64, CoverageError> {
    let n = corners.len();

    for edges in 1..=LONGEST_RUN.min(n.saturating_sub(3)) {
        let saved = slide_run(corners, i, edges, ctx)?;
        if saved > 0 {
            return Ok(saved);
        }
    }

    Ok(0)
}

/// Slides corners `i ..= i + edges` together.
fn slide_run(corners: &mut [Point], i: usize, edges: usize, ctx: &mut Ctx) -> Result<u64, CoverageError> {
    let n = corners.len();
    let Some((dx, dy, room)) = inward(corners, i, edges) else {
        return Ok(0);
    };

    let (a, d) = (at(corners, i + n - 1), at(corners, i + edges + 1));
    let mut run: Vec<Point> = (i..=i + edges).map(|t| at(corners, t)).collect();
    let (mut done, mut step) = (0, 1);

    let mut old = Vec::with_capacity(edges + 3);
    let mut new = Vec::with_capacity(edges + 3);
    while step >= 1 && done < room {
        let step_now = step.min(room - done);
        old.clear();
        old.push(a);
        old.extend_from_slice(&run);
        old.push(d);
        new.clear();
        new.push(a);
        new.extend(run.iter().map(|p| p.shifted(dx * step_now, dy * step_now)));
        new.push(d);

        ctx.edit.clear();
        ctx.edit.remove_path(&old);
        ctx.edit.add_path(&new);
        ctx.tries += 1;

        if ctx.edit.try_on(ctx.counts)? {
            run.copy_from_slice(&new[1..=edges + 1]);
            done += step_now;
            step *= 2;
        } else {
            step /= 2;
        }
    }

    for (t, &p) in run.iter().enumerate() {
        corners[(i + t) % n] = p;
    }
    Ok(2 * done as u64)
}

/// The unit step towards both neighbours of the run `i ..= i + edges` and
/// how far the run can go before one neighbour is used up; `None` unless
/// both neighbours leave the run on the same side, along the same axis.
fn inward(corners: &[Point], i: usize, edges: usize) -> Option<(i32, i32, i32)> {
    let n = corners.len();
    let (a, p) = (at(corners, i + n - 1), at(corners, i));
    let (q, d) = (at(corners, i + edges), at(corners, i + edges + 1));

    let (from_a, from_d, vertical) = if a.x == p.x && d.x == q.x && a.y != p.y {
        (a.y - p.y, d.y - q.y, true)
    } else if a.y == p.y && d.y == q.y && a.x != p.x {
        (a.x - p.x, d.x - q.x, false)
    } else {
        return None;
    };
    if from_a.signum() != from_d.signum() {
        return None;
    }

    let room = from_a.abs().min(from_d.abs());
    let sign = from_a.signum();
    match vertical {
        true => Some((0, sign, room)),
        false => Some((sign, 0, room)),
    }
}
