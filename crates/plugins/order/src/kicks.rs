//! Random changes the descent would not make on its own, because they do not
//! pay off right away. Half the time a block reversal between spatial
//! neighbours, half the time a local double bridge.

use rand::rngs::SmallRng;
use rand::Rng;

use crate::moves::Touched;
use crate::neighbours::Neighbours;
use crate::tour::Tour;

/// How far apart, in positions, the cuts of one double bridge may be.
const DOUBLE_BRIDGE_WINDOW: usize = 30;

/// One kick, keeping the tour cost up to date. Returns the stops to re-check.
pub(crate) fn kick(tour: &mut Tour, neighbours: &Neighbours, segments: usize, rng: &mut SmallRng) -> Touched {
    if rng.gen_bool(0.5) {
        reverse_between_neighbours(tour, neighbours, segments, rng)
    } else {
        double_bridge(tour, rng)
    }
}

/// Reverses the block between a random segment and one of its neighbours.
fn reverse_between_neighbours(
    tour: &mut Tour,
    neighbours: &Neighbours,
    segments: usize,
    rng: &mut SmallRng,
) -> Touched {
    let segment = rng.gen_range(0..segments as u32);
    let near = neighbours.of(segment);
    if near.is_empty() {
        return Touched::default();
    }

    let other = near[rng.gen_range(0..near.len())];
    let i = tour.pos_of[segment as usize] as usize;
    let j = tour.pos_of[other as usize] as usize;
    let (a, b) = (i.min(j), i.max(j));
    if b - a < 2 {
        return Touched::default();
    }

    let old = tour.link(a) + tour.link(b);
    tour.reverse(a + 1, b - a);
    tour.cost += tour.link(a) + tour.link(b) - old;

    let after = tour.next(b);
    Touched::at(tour, &[a, a + 1, b, after])
}

/// Swaps two neighbouring blocks, `..B C..` becomes `..C B..`, both kept in
/// their driving direction. No single 2-opt or Or-opt move undoes this when
/// the blocks are longer than one segment.
fn double_bridge(tour: &mut Tour, rng: &mut SmallRng) -> Touched {
    let m = tour.len();
    let window = DOUBLE_BRIDGE_WINDOW.min(m - 1);
    if window < 3 {
        return Touched::default();
    }

    // three cuts in start+1 ..= start+window, so the blocks never wrap
    let start = rng.gen_range(0..m - window);
    let mut cuts = distinct_three(rng, window).map(|offset| start + 1 + offset);
    cuts.sort_unstable();
    let [first, middle, end] = cuts;

    let old = tour.link(first - 1) + tour.link(middle - 1) + tour.link(end - 1);
    rotate(tour, first, middle, end);

    let new_middle = first + (end - middle);
    let new = tour.link(first - 1) + tour.link(new_middle - 1) + tour.link(end - 1);
    tour.cost += new - old;

    Touched::at(
        tour,
        &[first - 1, first, new_middle - 1, new_middle, end - 1, end % m],
    )
}

/// Three distinct numbers below `window`, which is at least 3.
fn distinct_three(rng: &mut SmallRng, window: usize) -> [usize; 3] {
    let a = rng.gen_range(0..window);

    let mut b = rng.gen_range(0..window);
    while b == a {
        b = rng.gen_range(0..window);
    }

    let mut c = rng.gen_range(0..window);
    while c == a || c == b {
        c = rng.gen_range(0..window);
    }

    [a, b, c]
}

/// Puts positions `middle..end` before `first..middle`.
fn rotate(tour: &mut Tour, first: usize, middle: usize, end: usize) {
    tour.seg_at[first..end].rotate_left(middle - first);
    tour.rev_at[first..end].rotate_left(middle - first);

    for p in first..end {
        tour.write(p, tour.seg_at[p], tour.rev_at[p]);
    }
}

#[cfg(test)]
mod tests {
    use mow_core::{Closure, Order, Segment};
    use rand::SeedableRng;

    use super::*;

    fn lanes(count: i32) -> Vec<Segment> {
        (0..count)
            .map(|y| Segment::horizontal(2 * y, 0, 10).unwrap())
            .collect()
    }

    #[test]
    fn kicks_keep_the_cost_and_maps_right() {
        let segments = lanes(40);
        let neighbours = Neighbours::build(&segments, 8);
        let mut rng = SmallRng::seed_from_u64(3);

        for closure in [Closure::Open, Closure::Closed] {
            let mut tour = Tour::new(&segments, &Order::identity(40), closure);

            for _ in 0..200 {
                kick(&mut tour, &neighbours, 40, &mut rng);
                assert_eq!(tour.cost, tour.full_cost());
            }

            for p in 0..tour.len() {
                assert_eq!(tour.pos_of[tour.seg_at[p] as usize] as usize, p);
            }
        }
    }

    #[test]
    fn tiny_tours_are_left_alone() {
        let segments = lanes(3);
        let mut tour = Tour::new(&segments, &Order::identity(3), Closure::Closed);
        let mut rng = SmallRng::seed_from_u64(1);

        assert!(double_bridge(&mut tour, &mut rng).ids().is_empty());
        assert_eq!(tour.cost, tour.full_cost());
    }
}
