//! For every segment, the few other segments with an end closest to either of
//! its own ends. Port of `ordering.nearest_ends`, on a bucket grid instead of
//! all pairs.

use mow_core::Segment;

use crate::grid::EndGrid;

/// The neighbour lists, flat: segment `s` has `ids[s * k .. s * k + len[s]]`,
/// nearest first, ties by lower index.
pub(crate) struct Neighbours {
    k: usize,
    ids: Vec<u32>,
    len: Vec<u8>,
}

impl Neighbours {
    /// Up to `count` neighbours per segment, never the segment itself.
    pub(crate) fn build(segments: &[Segment], count: usize) -> Self {
        let n = segments.len();
        let k = count.min(n.saturating_sub(1)).min(u8::MAX as usize);

        let ends = segments.iter().flat_map(|s| [s.start(), s.end()]).collect();
        let grid = EndGrid::new(ends);

        let mut lists = Self {
            k,
            ids: vec![0; n * k],
            len: vec![0; n],
        };
        if k == 0 {
            return lists;
        }

        let mut finder = Finder {
            grid: &grid,
            k,
            found: Vec::new(),
            stamp: vec![u32::MAX; n],
            round: 0,
        };
        for s in 0..n as u32 {
            let len = finder.nearest(s, &mut lists.ids[s as usize * k..(s as usize + 1) * k]);
            lists.len[s as usize] = len as u8;
        }

        lists
    }

    /// The neighbours of segment `s`, nearest first. Out of range ids, such as
    /// the dummy stop of an open order, have none.
    pub(crate) fn of(&self, s: u32) -> &[u32] {
        let s = s as usize;
        match self.len.get(s) {
            Some(&len) => &self.ids[s * self.k..s * self.k + len as usize],
            None => &[],
        }
    }
}

/// Scratch space for the queries, reused for every segment.
struct Finder<'a> {
    grid: &'a EndGrid,
    k: usize,

    // (distance, segment) candidates of the current query
    found: Vec<(u64, u32)>,

    // stamp[s] == round marks s as seen in the current count
    stamp: Vec<u32>,
    round: u32,
}

impl Finder<'_> {
    /// Writes the nearest segments of `s` to `out`, returns how many.
    fn nearest(&mut self, s: u32, out: &mut [u32]) -> usize {
        self.found.clear();
        self.search(s, 2 * s);
        self.search(s, 2 * s + 1);

        // each segment once, at its smallest distance to either end of s
        self.found.sort_unstable();
        self.round = self.round.wrapping_add(1);

        let mut len = 0;
        for i in 0..self.found.len() {
            let owner = self.found[i].1;
            if self.stamp[owner as usize] != self.round && len < out.len() {
                self.stamp[owner as usize] = self.round;
                out[len] = owner;
                len += 1;
            }
        }

        len
    }

    /// Adds the ends around `end` to `found`, ring by ring, until the `k`
    /// nearest other segments seen from this end are certain.
    fn search(&mut self, s: u32, end: u32) {
        let from = self.grid.point(end);
        let center = self.grid.cell_of(from);
        let keep = self.found.len();

        for r in 0..=self.grid.last_ring(center) {
            let found = &mut self.found;
            let grid = self.grid;
            grid.ring(center, r, |other| {
                if other / 2 != s {
                    found.push((from.l1(grid.point(other)), other / 2));
                }
            });

            let reach = r as u64 * self.grid.cell() as u64;
            if self.kth_distance(keep).is_some_and(|d| d <= reach) {
                return;
            }
        }
    }

    /// The distance of the `k`th distinct segment among `found[keep..]`.
    fn kth_distance(&mut self, keep: usize) -> Option<u64> {
        if self.found.len() - keep < self.k {
            return None;
        }

        self.found[keep..].sort_unstable();
        self.round = self.round.wrapping_add(1);

        let mut distinct = 0;
        for &(distance, owner) in &self.found[keep..] {
            if self.stamp[owner as usize] != self.round {
                self.stamp[owner as usize] = self.round;
                distinct += 1;
                if distinct == self.k {
                    return Some(distance);
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(y: i32, from: i32, to: i32) -> Segment {
        Segment::horizontal(y, from, to).unwrap()
    }

    /// All pairs, the way Python does it.
    fn brute(segments: &[Segment], count: usize) -> Vec<Vec<u32>> {
        let ends = |s: &Segment| [s.start(), s.end()];
        (0..segments.len())
            .map(|s| {
                let mut others: Vec<(u64, u32)> = (0..segments.len())
                    .filter(|&t| t != s)
                    .map(|t| {
                        let d = ends(&segments[s])
                            .iter()
                            .flat_map(|a| ends(&segments[t]).map(|b| a.l1(b)))
                            .min()
                            .unwrap();
                        (d, t as u32)
                    })
                    .collect();
                others.sort_unstable();
                others.into_iter().take(count).map(|(_, t)| t).collect()
            })
            .collect()
    }

    #[test]
    fn matches_all_pairs() {
        // a scattered, repeatable set of segments
        let segments: Vec<Segment> = (0..300)
            .map(|i: i32| {
                let y = (i * 37) % 101;
                let a = (i * 53) % 97 * 3;
                h(y, a, a + (i % 7) * 4)
            })
            .collect();

        let lists = Neighbours::build(&segments, 8);
        let expected = brute(&segments, 8);

        for (s, want) in expected.iter().enumerate() {
            assert_eq!(lists.of(s as u32), want.as_slice(), "segment {s}");
        }
    }

    #[test]
    fn small_sets_and_the_dummy() {
        let segments = vec![h(0, 0, 1), h(5, 0, 1)];
        let lists = Neighbours::build(&segments, 8);

        assert_eq!(lists.of(0), &[1]);
        assert_eq!(lists.of(1), &[0]);
        assert!(lists.of(2).is_empty());
        assert!(Neighbours::build(&segments[..1], 8).of(0).is_empty());
    }
}
