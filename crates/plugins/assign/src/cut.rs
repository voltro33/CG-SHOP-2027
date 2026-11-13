//! Cutting an order of segments into consecutive groups, each priced with
//! its way home (port of `groups.group_order`).

use mow_core::Point;

/// Cuts an order of segments, given as `(entry, exit)` per segment in
/// order, into at most `k` consecutive groups, as first and last index, so
/// the longest group closed back to its first entry is as short as the
/// greedy finds. Nothing to cut gives no groups.
pub fn group_order(ends: &[(Point, Point)], k: usize) -> Vec<(usize, usize)> {
    let n = ends.len();
    if n == 0 {
        return Vec::new();
    }

    if k <= 1 || n == 1 {
        return vec![(0, n - 1)];
    }

    let driven = Driven::new(ends);

    let mut low = 0;
    let mut high = driven.closed_cost(0, n - 1);
    let mut best = driven.greedy(k, high).unwrap_or_else(|| vec![(0, n - 1)]);

    while low < high {
        let target = low + (high - low) / 2;

        match driven.greedy(k, target) {
            Some(groups) => {
                best = groups;
                high = target;
            }
            None => low = target + 1,
        }
    }

    best
}

/// Driven length from the first entry to every entry and exit, in order.
struct Driven<'a> {
    ends: &'a [(Point, Point)],
    at_entry: Vec<u64>,
    at_exit: Vec<u64>,
}

impl<'a> Driven<'a> {
    fn new(ends: &'a [(Point, Point)]) -> Self {
        let mut at_entry = Vec::with_capacity(ends.len());
        let mut at_exit = Vec::with_capacity(ends.len());
        let mut total = 0;
        let mut previous_exit = None;

        for &(entry, exit) in ends {
            total += previous_exit.map_or(0, |p: Point| p.l1(entry));
            at_entry.push(total);

            total += entry.l1(exit);
            at_exit.push(total);

            previous_exit = Some(exit);
        }

        Self {
            ends,
            at_entry,
            at_exit,
        }
    }

    fn closed_cost(&self, first: usize, last: usize) -> u64 {
        let inside = self.at_exit[last] - self.at_entry[first];
        let home = self.ends[first].0.l1(self.ends[last].1);

        inside + home
    }

    /// Groups as long as `target` allows, or `None` if `k` are not enough.
    /// Like Python, every later segment is tried as the group's last and the
    /// farthest that fits wins; a lone segment always forms a group.
    fn greedy(&self, k: usize, target: u64) -> Option<Vec<(usize, usize)>> {
        let n = self.ends.len();
        let mut groups = Vec::with_capacity(k);
        let mut first = 0;

        while first < n {
            if groups.len() == k {
                return None;
            }

            let last = (first..n)
                .rev()
                .find(|&last| self.closed_cost(first, last) <= target)
                .unwrap_or(first);

            groups.push((first, last));
            first = last + 1;
        }

        Some(groups)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lane(y: i32, a: i32, b: i32) -> (Point, Point) {
        (Point::new(a, y), Point::new(b, y))
    }

    #[test]
    fn two_far_apart_blocks_split_between_them() {
        let ends = [lane(0, 0, 10), lane(1, 10, 0), lane(100, 0, 10), lane(101, 10, 0)];

        assert_eq!(group_order(&ends, 2), vec![(0, 1), (2, 3)]);
        assert_eq!(group_order(&ends, 1), vec![(0, 3)]);
    }

    #[test]
    fn fewer_segments_than_cutters() {
        let ends = [lane(0, 0, 4)];

        assert_eq!(group_order(&ends, 3), vec![(0, 0)]);
        assert!(group_order(&[], 2).is_empty());
    }

    #[test]
    fn three_equal_lanes_get_one_group_each() {
        let ends = [lane(0, 0, 50), lane(10, 50, 0), lane(20, 0, 50)];

        assert_eq!(group_order(&ends, 3), vec![(0, 0), (1, 1), (2, 2)]);
    }
}
