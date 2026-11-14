//! Corner lists of closed tours: cyclic access, the ways between two
//! points, splicing and normalizing.

use mow_core::Point;

/// Corner `i` of a closed tour, any `i`, wrapping around.
pub fn at(corners: &[Point], i: usize) -> Point {
    corners[i % corners.len()]
}

/// Length of the open path through `corners` in order.
pub fn open_length(corners: &[Point]) -> u64 {
    corners.windows(2).map(|w| w[0].l1(w[1])).sum()
}

/// Length of the closed tour through `corners`.
pub fn closed_length(corners: &[Point]) -> u64 {
    let n = corners.len();
    (0..n).map(|i| corners[i].l1(corners[(i + 1) % n])).sum()
}

/// The corners `i, i + 1, ..., i + w`, wrapping around.
pub fn window(corners: &[Point], i: usize, w: usize, out: &mut Vec<Point>) {
    out.clear();
    out.extend((i..=i + w).map(|t| at(corners, t)));
}

/// The bends of the shortest ways from `a` to `b` with one bend each: none
/// if the two share a row or a column, else the two L shapes.
pub fn bends(a: Point, b: Point) -> Vec<Option<Point>> {
    match a.is_axis_parallel_to(b) {
        true => vec![None],
        false => vec![Some(Point::new(b.x, a.y)), Some(Point::new(a.x, b.y))],
    }
}

/// Z shaped shortest ways from `a` to `b` through the column or row of
/// `p`, when that column or row lies strictly between them.
pub fn z_ways(a: Point, b: Point, p: Point) -> Vec<[Point; 2]> {
    let mut ways = Vec::new();

    if strictly_between(p.x, a.x, b.x) && a.y != b.y {
        ways.push([Point::new(p.x, a.y), Point::new(p.x, b.y)]);
    }
    if strictly_between(p.y, a.y, b.y) && a.x != b.x {
        ways.push([Point::new(a.x, p.y), Point::new(b.x, p.y)]);
    }

    ways
}

fn strictly_between(v: i32, a: i32, b: i32) -> bool {
    v > a.min(b) && v < a.max(b)
}

/// Replaces the corners strictly between `i` and `i + w` (wrapping) by
/// `interior`. The tour may start at another corner afterwards.
pub fn splice(corners: &mut Vec<Point>, i: usize, w: usize, interior: &[Point]) {
    let n = corners.len();
    let i = i % n;

    if i + w >= n {
        corners.rotate_left(i);
        corners.splice(1..w, interior.iter().copied());
    } else {
        corners.splice(i + 1..i + w, interior.iter().copied());
    }
}

/// Drops repeated corners and corners a straight edge runs through. The
/// positions stay the same; at least one corner stays.
pub fn normalize(corners: &mut Vec<Point>) {
    let mut kept: Vec<Point> = Vec::with_capacity(corners.len());
    for &p in corners.iter() {
        if kept.last() == Some(&p) {
            continue;
        }
        while kept.len() >= 2 && straight(kept[kept.len() - 2], kept[kept.len() - 1], p) {
            kept.pop();
        }
        kept.push(p);
    }

    // the same around the closing edge
    loop {
        let n = kept.len();
        if n >= 2 && kept[0] == kept[n - 1] {
            kept.pop();
        } else if n >= 3 && straight(kept[n - 1], kept[0], kept[1]) {
            kept.remove(0);
        } else if n >= 3 && straight(kept[n - 2], kept[n - 1], kept[0]) {
            kept.pop();
        } else {
            break;
        }
    }

    *corners = kept;
}

/// True if `b` lies strictly inside the straight way from `a` to `c`.
fn straight(a: Point, b: Point, c: Point) -> bool {
    let on_column = a.x == b.x && b.x == c.x && (b.y - a.y).signum() * (c.y - b.y).signum() > 0;
    let on_row = a.y == b.y && b.y == c.y && (b.x - a.x).signum() * (c.x - b.x).signum() > 0;
    on_column || on_row
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pts(raw: &[(i32, i32)]) -> Vec<Point> {
        raw.iter().map(|&p| p.into()).collect()
    }

    #[test]
    fn bends_of_aligned_and_diagonal_pairs() {
        assert_eq!(bends(Point::new(0, 0), Point::new(0, 5)), vec![None]);
        assert_eq!(
            bends(Point::new(0, 0), Point::new(2, 3)),
            vec![Some(Point::new(2, 0)), Some(Point::new(0, 3))]
        );
    }

    #[test]
    fn z_ways_only_strictly_inside() {
        let (a, b) = (Point::new(0, 0), Point::new(4, 4));

        assert_eq!(
            z_ways(a, b, Point::new(2, 9)),
            vec![[Point::new(2, 0), Point::new(2, 4)]]
        );
        assert_eq!(z_ways(a, b, Point::new(4, 4)), Vec::<[Point; 2]>::new());
        assert_eq!(z_ways(a, b, Point::new(1, 3)).len(), 2);
    }

    #[test]
    fn splice_inside_and_across_the_end() {
        let mut inside = pts(&[(0, 0), (1, 0), (2, 0), (3, 0)]);
        splice(&mut inside, 0, 2, &pts(&[(9, 9)]));
        assert_eq!(inside, pts(&[(0, 0), (9, 9), (2, 0), (3, 0)]));

        let mut across = pts(&[(0, 0), (1, 0), (2, 0), (3, 0)]);
        splice(&mut across, 3, 2, &[]);
        assert_eq!(across, pts(&[(3, 0), (1, 0), (2, 0)]));
    }

    #[test]
    fn normalize_drops_repeats_and_straight_corners_but_keeps_spikes() {
        let mut route = pts(&[(0, 0), (2, 0), (2, 0), (4, 0), (4, 2), (0, 2)]);
        normalize(&mut route);
        assert_eq!(route, pts(&[(0, 0), (4, 0), (4, 2), (0, 2)]));

        let mut spike = pts(&[(0, 0), (3, 0), (0, 0), (0, 2)]);
        normalize(&mut spike);
        assert_eq!(spike, pts(&[(0, 0), (3, 0), (0, 0), (0, 2)]));
    }
}
