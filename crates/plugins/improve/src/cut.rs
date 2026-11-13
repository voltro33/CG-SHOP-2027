//! One corner cut (`_try_cut` in `improve.py`): replace `a -> b -> c` by the
//! shortest way `a -> c`, or drop a spike `a -> b -> a`, if every region
//! cell around the positions given up stays swept.

use mow_core::Point;
use mow_coverage::{CoverageError, VisitCounts};

/// An accepted cut, to apply to the route's corners.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cut {
    /// Corner `at` becomes `bend`, or goes if there is none.
    Replace { at: usize, bend: Option<Point> },
    /// Corner `at` and the one after it (cyclically) go.
    DropSpike { at: usize },
}

impl Cut {
    /// Applies the cut. Returns the indices, after the cut, whose corner or
    /// neighbours changed.
    pub fn apply(self, route: &mut Vec<Point>) -> [usize; 3] {
        match self {
            Self::Replace { at, bend: Some(p) } => {
                route[at] = p;
                around(route.len(), at, 1)
            }
            Self::Replace { at, bend: None } => {
                route.remove(at);
                around(route.len(), at, 0)
            }
            Self::DropSpike { at } if at + 1 < route.len() => {
                route.drain(at..at + 2);
                around(route.len(), at, 0)
            }
            Self::DropSpike { .. } => {
                // the last corner and the first
                route.pop();
                route.remove(0);
                around(route.len(), route.len(), 0)
            }
        }
    }
}

/// `at - 1`, `at` and `at + extra`, cyclically, over `n` corners.
fn around(n: usize, at: usize, extra: usize) -> [usize; 3] {
    if n == 0 {
        return [0; 3];
    }

    [(at + n - 1) % n, at % n, (at + extra) % n]
}

/// The positions of the path through `corners`, all but the first `skip`
/// and the last `drop` (`densify(corners)[skip:len - drop]`).
fn path(corners: &[Point], skip: usize, drop: usize) -> impl Iterator<Item = Point> + '_ {
    let total = 1 + corners.windows(2).map(|w| w[0].l1(w[1]) as usize).sum::<usize>();

    let steps = corners.windows(2).flat_map(|w| {
        let (a, b) = (w[0], w[1]);
        let (dx, dy) = ((b.x - a.x).signum(), (b.y - a.y).signum());
        (1..=a.l1(b) as i32).map(move |t| a.shifted(dx * t, dy * t))
    });

    std::iter::once(corners[0])
        .chain(steps)
        .take(total.saturating_sub(drop))
        .skip(skip)
}

/// Tries to cut corner `i` of `route` (closed or not), as `_try_cut`. On
/// success the counts hold the new route and the cut is returned; otherwise
/// the counts are as they were.
pub fn try_cut(route: &[Point], i: usize, counts: &mut VisitCounts) -> Result<Option<Cut>, CoverageError> {
    let n = route.len();
    let (a, b, c) = (route[(i + n - 1) % n], route[i], route[(i + 1) % n]);

    if a == c {
        let spike = [a, b, a];
        let accepted = attempt(counts, &spike, 1, 0, None)?;
        return Ok(accepted.then_some(Cut::DropSpike { at: i }));
    }

    let aligned = a.x == c.x || a.y == c.y;
    let gain = a.l1(b) + b.l1(c) - a.l1(c);
    if gain == 0 && !aligned {
        // same length, same number of corners: no progress
        return Ok(None);
    }

    let bends: &[Option<Point>] = if aligned {
        &[None]
    } else {
        &[Some(Point::new(c.x, a.y)), Some(Point::new(a.x, c.y))]
    };

    for &bend in bends {
        let new_way: Vec<Point> = std::iter::once(a).chain(bend).chain([c]).collect();
        if attempt(counts, &[a, b, c], 1, 1, Some(&new_way))? {
            return Ok(Some(Cut::Replace { at: i, bend }));
        }
    }

    Ok(None)
}

/// Gives up the positions of `old` (without `skip` first and `drop` last),
/// takes the inner positions of `new`, and keeps the change if the cells
/// around the positions given up stay swept; reverts it otherwise.
fn attempt(
    counts: &mut VisitCounts,
    old: &[Point],
    skip: usize,
    drop: usize,
    new: Option<&[Point]>,
) -> Result<bool, CoverageError> {
    counts.remove_positions(path(old, skip, drop))?;
    if let Some(new) = new {
        counts.add_positions(path(new, 1, 1))?;
    }

    if counts.all_swept_around(path(old, skip, drop)) {
        return Ok(true);
    }

    if let Some(new) = new {
        counts.remove_positions(path(new, 1, 1))?;
    }
    counts.add_positions(path(old, skip, drop))?;

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pts(raw: &[(i32, i32)]) -> Vec<Point> {
        raw.iter().map(|&p| p.into()).collect()
    }

    #[test]
    fn path_skips_and_drops_like_a_slice() {
        let corners = pts(&[(0, 0), (2, 0), (2, 1)]);

        assert_eq!(path(&corners, 0, 0).count(), 4);
        assert_eq!(path(&corners, 1, 1).collect::<Vec<_>>(), pts(&[(1, 0), (2, 0)]));
        assert_eq!(path(&pts(&[(0, 0), (0, 0)]), 1, 0).count(), 0);
    }

    #[test]
    fn apply_reports_the_changed_neighbourhood() {
        let mut route = pts(&[(0, 0), (2, 0), (2, 2), (0, 2)]);
        let dirty = Cut::DropSpike { at: 3 }.apply(&mut route);

        assert_eq!(route, pts(&[(2, 0), (2, 2)]));
        assert_eq!(dirty, [1, 0, 0]);
    }
}
