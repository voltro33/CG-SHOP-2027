//! Dropping repeated and straight-through corners, open and closed, in
//! exactly the order Python does it, but without its quadratic rescans.
//!
//! - open: `_normalize_open` in `improve.py`, one stack pass;
//! - closed: `normalize_cycle` in `tours.py`, which removes the *first*
//!   removable corner and rescans from the start. Which corner goes first can
//!   change the result, so the fast versions keep that order: a corner can
//!   only become removable when a neighbour goes, so only those are rechecked.
//!
//! The `*_after_edit` versions assume the route was normalized before one
//! local edit and only look around it, so a long route pays for the edit,
//! not for its length.

use std::collections::BTreeSet;

use mow_core::Point;

/// True if `b` lies strictly on the way from `a` to `c` along one axis.
pub fn passes_straight(a: Point, b: Point, c: Point) -> bool {
    let between = |p: i32, q: i32, r: i32| (p < q && q < r) || (p > q && q > r);

    (a.x == b.x && b.x == c.x && between(a.y, b.y, c.y))
        || (a.y == b.y && b.y == c.y && between(a.x, b.x, c.x))
}

/// Pushes `p` onto an open normalization stack.
fn push_open(stack: &mut Vec<Point>, p: Point) {
    if stack.last() == Some(&p) {
        return;
    }

    while stack.len() >= 2 && passes_straight(stack[stack.len() - 2], stack[stack.len() - 1], p) {
        stack.pop();
    }

    stack.push(p);
}

/// `_normalize_open`: the ends stay, repeats and straight-through corners go.
pub fn normalize_open(corners: &[Point]) -> Vec<Point> {
    let mut stack = Vec::with_capacity(corners.len());
    for &p in corners {
        push_open(&mut stack, p);
    }

    stack
}

/// `_normalize_open` of `route`, which was normalized before an edit that
/// changed it from index `at` on.
///
/// A normalized prefix loses at most its last corner to the pass (losing two
/// would need three of its corners in a straight line), so the pass restarts
/// two corners before the edit. It stops once it has pushed two consecutive
/// untouched corners: from there on nothing pops, so the rest stays.
pub fn normalize_open_after_edit(route: &mut Vec<Point>, at: usize) {
    let low = at.saturating_sub(2);
    let mut stack: Vec<Point> = Vec::with_capacity(8);
    let mut end = route.len();

    for j in low..route.len() {
        push_open(&mut stack, route[j]);

        let settled = j >= at + 2
            && stack.len() >= 2
            && stack[stack.len() - 1] == route[j]
            && stack[stack.len() - 2] == route[j - 1];
        if settled {
            end = j + 1;
            break;
        }
    }

    route.splice(low..end, stack);
}

/// True if corner `i` of a cycle of `n` corners would be dropped.
fn removable(prev: Point, cur: Point, next: Point, n: usize) -> bool {
    cur == next || (n > 2 && passes_straight(prev, cur, next))
}

/// `normalize_cycle`, in `O(n log n)`: corners linked in a ring, the
/// candidates kept by their original index, which removals never reorder.
pub fn normalize_cycle(corners: &[Point]) -> Vec<Point> {
    let n = corners.len();
    if n <= 1 {
        return corners.to_vec();
    }

    let mut prev: Vec<usize> = (0..n).map(|i| (i + n - 1) % n).collect();
    let mut next: Vec<usize> = (0..n).map(|i| (i + 1) % n).collect();
    let mut alive = vec![true; n];
    let mut left = n;

    let mut dirty: BTreeSet<usize> = (0..n).collect();
    while left > 1 {
        let Some(i) = dirty.pop_first() else { break };
        if !alive[i] || !removable(corners[prev[i]], corners[i], corners[next[i]], left) {
            continue;
        }

        let (p, q) = (prev[i], next[i]);
        next[p] = q;
        prev[q] = p;
        alive[i] = false;
        left -= 1;

        dirty.insert(p);
        dirty.insert(q);
    }

    (0..n).filter(|&i| alive[i]).map(|i| corners[i]).collect()
}

/// `normalize_cycle` of `route`, which was normalized before an edit; only
/// the corners in `dirty` (indices after the edit) can be removable now.
pub fn normalize_cycle_after_edit(route: &mut Vec<Point>, dirty: &[usize]) {
    let mut dirty: Vec<usize> = dirty.to_vec();

    while route.len() > 1 {
        dirty.sort_unstable();
        dirty.dedup();
        if dirty.is_empty() {
            return;
        }

        let i = dirty.remove(0);
        let n = route.len();
        if !removable(route[(i + n - 1) % n], route[i], route[(i + 1) % n], n) {
            continue;
        }

        route.remove(i);
        let n = route.len();
        for d in dirty.iter_mut() {
            if *d > i {
                *d -= 1;
            }
        }

        // the two neighbours of the removed corner now face each other
        dirty.push((i + n - 1) % n);
        dirty.push(i % n);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pts(raw: &[(i32, i32)]) -> Vec<Point> {
        raw.iter().map(|&p| p.into()).collect()
    }

    /// `normalize_cycle` exactly as Python writes it, quadratic.
    fn slow_cycle(corners: &[Point]) -> Vec<Point> {
        let mut route = corners.to_vec();
        'outer: while route.len() > 1 {
            let n = route.len();
            for i in 0..n {
                if removable(route[(i + n - 1) % n], route[i], route[(i + 1) % n], n) {
                    route.remove(i);
                    continue 'outer;
                }
            }
            break;
        }
        route
    }

    /// A small xorshift, so the tests need no random crate.
    fn rng(state: &mut u64, n: i32) -> i32 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        (*state % n as u64) as i32
    }

    /// A random axis parallel corner list with many repeats and straights.
    fn random_cycle(state: &mut u64, len: usize) -> Vec<Point> {
        let mut p = Point::new(0, 0);
        let mut out = vec![p];
        for _ in 1..len {
            if rng(state, 2) == 0 {
                p.x = rng(state, 4);
            } else {
                p.y = rng(state, 4);
            }
            out.push(p);
        }
        // close it along one axis
        out.push(Point::new(0, p.y));
        out
    }

    #[test]
    fn cycle_matches_the_quadratic_reference() {
        let mut state = 7;
        for len in 1..40 {
            for _ in 0..50 {
                let corners = random_cycle(&mut state, len);
                assert_eq!(normalize_cycle(&corners), slow_cycle(&corners), "{corners:?}");
            }
        }
    }

    #[test]
    fn local_cycle_matches_after_random_edits() {
        let mut state = 11;
        for _ in 0..2000 {
            let base = normalize_cycle(&random_cycle(&mut state, 12));
            if base.len() < 3 {
                continue;
            }

            let i = rng(&mut state, base.len() as i32) as usize;
            let mut edited = base.clone();
            edited[i] = Point::new(edited[(i + 1) % base.len()].x, edited[i].y);
            let n = edited.len();
            let dirty = [(i + n - 1) % n, i, (i + 1) % n];

            let want = slow_cycle(&edited);
            normalize_cycle_after_edit(&mut edited, &dirty);
            assert_eq!(edited, want);
        }
    }

    #[test]
    fn local_open_matches_after_random_edits() {
        let mut state = 5;
        for _ in 0..2000 {
            let base = normalize_open(&random_cycle(&mut state, 15));
            if base.len() < 3 {
                continue;
            }

            let i = 1 + rng(&mut state, base.len() as i32 - 2) as usize;
            let mut edited = base.clone();
            if rng(&mut state, 2) == 0 {
                edited.remove(i);
            } else {
                edited[i] = Point::new(rng(&mut state, 4), edited[i].y);
            }

            let want = normalize_open(&edited);
            normalize_open_after_edit(&mut edited, i);
            assert_eq!(edited, want);
        }
    }

    #[test]
    fn open_keeps_reversals_and_ends() {
        let corners = pts(&[(0, 0), (1, 0), (1, 0), (3, 0), (2, 0), (2, 2), (2, 4)]);

        assert_eq!(normalize_open(&corners), pts(&[(0, 0), (3, 0), (2, 0), (2, 4)]));
    }
}
