//! Moving single segments out of the longest closed group into the
//! cheapest slot of another group (port of `groups.rebalance`, `_best_move`
//! and `_apply_move`).

use std::time::Instant;

use mow_core::{Point, Segment, Visit};

/// How many single segment moves one rebalance may make.
const MAX_MOVES: usize = 500;

/// One group: the visits of a closed tour, in order.
pub(crate) type Group = Vec<Visit>;

/// Moves single segments out of the longest group, while that shortens the
/// longest group, until `deadline` or [`MAX_MOVES`].
pub(crate) fn rebalance(segments: &[Segment], mut groups: Vec<Group>, deadline: Instant) -> Vec<Group> {
    if groups.len() < 2 {
        return groups;
    }

    for _ in 0..MAX_MOVES {
        if Instant::now() >= deadline {
            break;
        }

        let Some(found) = best_move(segments, &groups) else {
            break;
        };

        apply(&mut groups, found);
    }

    groups
}

/// A visit's `(entry, exit)`.
pub(crate) fn ends(segments: &[Segment], visit: Visit) -> (Point, Point) {
    segments[visit.segment as usize].ends(visit.reversed)
}

/// The length of a group driven as a closed tour.
pub(crate) fn closed_length(segments: &[Segment], group: &[Visit]) -> u64 {
    let n = group.len();

    (0..n)
        .map(|i| {
            let (entry, exit) = ends(segments, group[i]);
            let (next_entry, _) = ends(segments, group[(i + 1) % n]);
            entry.l1(exit) + exit.l1(next_entry)
        })
        .sum()
}

/// Take visit `position` of group `source`, put it after visit `slot` of
/// group `target`, driven the other way if `flipped`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Move {
    source: usize,
    position: usize,
    target: usize,
    slot: usize,
    flipped: bool,
}

fn apply(groups: &mut [Group], found: Move) {
    let mut visit = groups[found.source].remove(found.position);
    visit.reversed ^= found.flipped;

    groups[found.target].insert(found.slot + 1, visit);
}

/// What taking one visit out of a closed group saves, and what driving it
/// costs, per visit.
struct Removal {
    saving: Vec<i64>,
    across: Vec<i64>,
}

fn removal(segments: &[Segment], group: &[Visit]) -> Removal {
    let n = group.len();
    let at = |i: usize| ends(segments, group[i % n]);
    let d = |a: Point, b: Point| a.l1(b) as i64;

    let mut saving = Vec::with_capacity(n);
    let mut across = Vec::with_capacity(n);

    for i in 0..n {
        let (_, before) = at(i + n - 1);
        let (entry, exit) = at(i);
        let (after, _) = at(i + 1);

        let drive = d(entry, exit);
        saving.push(d(before, entry) + drive + d(exit, after) - d(before, after));
        across.push(drive);
    }

    Removal { saving, across }
}

/// The move that shortens the longest group the most, or `None`. Ties go to
/// the first target group, then the first visit, then the first slot, as
/// Python's argmins do.
fn best_move(segments: &[Segment], groups: &[Group]) -> Option<Move> {
    let lengths: Vec<i64> = groups.iter().map(|g| closed_length(segments, g) as i64).collect();
    let longest = first_max(&lengths);
    let current = lengths[longest];

    let source = &groups[longest];
    if source.len() < 2 {
        return None;
    }

    let removal = removal(segments, source);
    let mut best = None;
    let mut best_peak = current;

    for (target, group) in groups.iter().enumerate() {
        // an empty group has no slot to insert into
        if target == longest || group.is_empty() {
            continue;
        }

        let slots = slots(segments, group);
        let floor = lengths
            .iter()
            .enumerate()
            .filter(|&(i, _)| i != longest && i != target)
            .map(|(_, &l)| l)
            .max()
            .unwrap_or(0);

        for (position, &visit) in source.iter().enumerate() {
            let (a, b) = ends(segments, visit);
            let (slot, added, flipped) = cheapest_slot(&slots, a, b);

            let shrunk = current - removal.saving[position];
            let grown = lengths[target] + added + removal.across[position];
            let peak = shrunk.max(grown).max(floor);

            if peak < best_peak {
                best_peak = peak;
                best = Some(Move {
                    source: longest,
                    position,
                    target,
                    slot,
                    flipped,
                });
            }
        }
    }

    best
}

/// A slot sits between the exit of a visit and the entry of the next one:
/// `(from, to, link)` per visit of the group.
fn slots(segments: &[Segment], group: &[Visit]) -> Vec<(Point, Point, i64)> {
    let n = group.len();

    (0..n)
        .map(|j| {
            let (_, from) = ends(segments, group[j]);
            let (to, _) = ends(segments, group[(j + 1) % n]);
            (from, to, from.l1(to) as i64)
        })
        .collect()
}

/// The slot where a segment from `a` to `b` adds the least link length,
/// driven as is or flipped: `(slot, added link, flipped)`.
fn cheapest_slot(slots: &[(Point, Point, i64)], a: Point, b: Point) -> (usize, i64, bool) {
    let mut best = (0, i64::MAX, false);

    for (j, &(from, to, link)) in slots.iter().enumerate() {
        let straight = (a.l1(from) + to.l1(b)) as i64;
        let flipped = (b.l1(from) + to.l1(a)) as i64;
        let cheaper = straight.min(flipped) - link;

        if cheaper < best.1 {
            best = (j, cheaper, flipped < straight);
        }
    }

    best
}

fn first_max(values: &[i64]) -> usize {
    let mut best = 0;
    for (i, &v) in values.iter().enumerate() {
        if v > values[best] {
            best = i;
        }
    }

    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn visits(range: std::ops::Range<u32>) -> Group {
        range
            .map(|segment| Visit {
                segment,
                reversed: false,
            })
            .collect()
    }

    #[test]
    fn closed_length_counts_the_way_home() {
        let segments = vec![
            Segment::horizontal(0, 0, 10).unwrap(),
            Segment::horizontal(2, 0, 10).unwrap(),
        ];
        let group = vec![
            Visit {
                segment: 0,
                reversed: false,
            },
            Visit {
                segment: 1,
                reversed: true,
            },
        ];

        assert_eq!(closed_length(&segments, &group), 24);
    }

    #[test]
    fn a_lopsided_split_evens_out() {
        let segments: Vec<Segment> = (0..6)
            .map(|i| Segment::horizontal(2 * i, 0, 20).unwrap())
            .collect();
        let groups = vec![visits(0..5), visits(5..6)];

        let before = groups.iter().map(|g| closed_length(&segments, g)).max().unwrap();
        let after = rebalance(
            &segments,
            groups,
            Instant::now() + std::time::Duration::from_secs(5),
        );
        let longest = after.iter().map(|g| closed_length(&segments, g)).max().unwrap();

        assert!(longest < before, "{longest} !< {before}");
        assert_eq!(after.iter().map(Vec::len).sum::<usize>(), 6);
    }

    #[test]
    fn one_group_stays() {
        let segments = vec![Segment::horizontal(0, 0, 3).unwrap()];
        let groups = vec![visits(0..1)];

        assert_eq!(rebalance(&segments, groups.clone(), Instant::now()), groups);
    }
}
