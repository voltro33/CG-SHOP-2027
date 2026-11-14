//! Choosing the witnesses: region cells far apart in anchor distance.
//!
//! Extreme cells sit on the region's border, so the candidates are the
//! border cells, and farthest point sampling spreads the witnesses over them.

use super::reach::Reach;
use crate::geom::Point;
use crate::problem::Problem;

/// More border cells than this are thinned to every n-th one.
const MAX_CANDIDATES: usize = 200_000;

/// Up to `count` border cells, each the farthest from those before it.
/// Stops early once every candidate is within reach of a witness.
pub fn witnesses(problem: &Problem, reach: &Reach, count: usize) -> Vec<Point> {
    let candidates = border_cells(problem);
    let Some(&first) = candidates.first() else {
        return Vec::new();
    };

    // start from the cell farthest from an arbitrary one: an extreme cell
    let from_first: Vec<u64> = candidates.iter().map(|&c| reach.between(first, c)).collect();
    let start = candidates[argmax(&from_first)];

    let mut chosen = vec![start];
    let mut nearest: Vec<u64> = candidates.iter().map(|&c| reach.between(start, c)).collect();

    while chosen.len() < count {
        let next = argmax(&nearest);
        if nearest[next] == 0 {
            break;
        }

        let witness = candidates[next];
        chosen.push(witness);
        for (gap, &c) in nearest.iter_mut().zip(&candidates) {
            *gap = (*gap).min(reach.between(witness, c));
        }
    }

    chosen
}

/// Region cells with a side on the outside, thinned to at most
/// [`MAX_CANDIDATES`].
fn border_cells(problem: &Problem) -> Vec<Point> {
    let region = problem.region();
    let sides = [(1, 0), (-1, 0), (0, 1), (0, -1)];

    let border: Vec<Point> = region
        .cells()
        .filter(|c| sides.iter().any(|&(dx, dy)| !region.contains(c.shifted(dx, dy))))
        .collect();

    let stride = border.len().div_ceil(MAX_CANDIDATES).max(1);
    border.into_iter().step_by(stride).collect()
}

/// The index of the largest value, the first on ties.
fn argmax(values: &[u64]) -> usize {
    let mut best = 0;

    for (index, &value) in values.iter().enumerate() {
        if value > values[best] {
            best = index;
        }
    }

    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::problem::tests::rectangle;

    #[test]
    fn a_bar_gets_its_two_ends_first() {
        let p = rectangle(30, 1, 1, 1);
        let reach = Reach::new(p.cutter());
        let chosen = witnesses(&p, &reach, 3);

        assert_eq!(chosen[0], Point::new(29, 0));
        assert_eq!(chosen[1], Point::new(0, 0));
        assert_eq!(chosen[2].y, 0);
    }

    #[test]
    fn stops_when_one_cutter_covers_everything() {
        let p = rectangle(3, 3, 3, 1);
        let reach = Reach::new(p.cutter());

        assert_eq!(witnesses(&p, &reach, 10).len(), 1);
    }
}
