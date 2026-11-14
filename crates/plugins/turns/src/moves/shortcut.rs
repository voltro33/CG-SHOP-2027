//! Shortcut: the way from corner `i` to corner `i + w` becomes a shortest
//! one, an L or a Z through the column or row of a corner it skips. With
//! `w = 2` this is the corner cut; longer windows drop detours, spikes and
//! short lanes whose cells something else sweeps.

use mow_core::Point;
use mow_coverage::CoverageError;

use super::Ctx;
use crate::path::{at, bends, open_length, splice, window, z_ways};

/// Tries windows of 2 to `ctx.options.window` corners from `i`, shortest
/// first. Returns the steps saved, 0 if nothing was kept.
pub fn shortcut(corners: &mut Vec<Point>, i: usize, ctx: &mut Ctx) -> Result<u64, CoverageError> {
    let n = corners.len();
    let widest = ctx.options.window.min(n.saturating_sub(1));

    for w in 2..=widest {
        let (a, b) = (at(corners, i), at(corners, i + w));
        let mut old = std::mem::take(&mut ctx.buf);
        window(corners, i, w, &mut old);

        let old_length = open_length(&old);
        let kept = match old_length > ctx.options.longest_shortcut {
            true => None,
            false => first_way(&old, a, b, ctx)?,
        };
        ctx.buf = old;

        if let Some(interior) = kept {
            splice(corners, i, w, &interior);
            return Ok(old_length - a.l1(b));
        }
        if old_length > ctx.options.longest_shortcut {
            break;
        }
    }

    Ok(0)
}

/// The first shortest way from `a` to `b` that keeps every cell swept, as
/// its corners between `a` and `b`, applied to the counts.
fn first_way(old: &[Point], a: Point, b: Point, ctx: &mut Ctx) -> Result<Option<Vec<Point>>, CoverageError> {
    if open_length(old) <= a.l1(b) {
        return Ok(None);
    }

    for interior in ways(old, a, b) {
        let mut new = Vec::with_capacity(interior.len() + 2);
        new.push(a);
        new.extend_from_slice(&interior);
        new.push(b);

        ctx.edit.clear();
        ctx.edit.remove_path(old);
        ctx.edit.add_path(&new);
        ctx.tries += 1;
        if ctx.edit.try_on(ctx.counts)? {
            return Ok(Some(interior));
        }
    }

    Ok(None)
}

/// The L ways first, then the Z ways through the corners given up.
fn ways(old: &[Point], a: Point, b: Point) -> Vec<Vec<Point>> {
    let mut ways: Vec<Vec<Point>> = bends(a, b)
        .into_iter()
        .map(|bend| bend.into_iter().collect())
        .collect();

    for &p in &old[1..old.len() - 1] {
        for z in z_ways(a, b, p) {
            if !ways.iter().any(|w| w[..] == z[..]) {
                ways.push(z.to_vec());
            }
        }
    }

    ways
}
