//! Tour bounds over a complete graph of witnesses.
//!
//! Every bound here holds for any edge weights, metric or not: a tour that
//! visits one anchor per witness, in the order it first meets them, is a
//! cycle whose edges are at least the anchor-set distances.

/// Symmetric distances between `n` witnesses.
#[derive(Debug, Clone)]
pub struct Distances {
    n: usize,
    weights: Vec<u64>,
}

impl Distances {
    pub fn new(n: usize, distance: impl Fn(usize, usize) -> u64) -> Self {
        let mut weights = vec![0; n * n];
        for i in 0..n {
            for j in i + 1..n {
                let d = distance(i, j);
                weights[i * n + j] = d;
                weights[j * n + i] = d;
            }
        }

        Self { n, weights }
    }

    const fn len(&self) -> usize {
        self.n
    }

    fn get(&self, i: usize, j: usize) -> u64 {
        self.weights[i * self.n + j]
    }
}

/// Steps without a better bound before the step size halves.
const STALL: usize = 15;

/// Held and Karp's 1-tree bound with node penalties, raised by subgradient
/// steps: a cycle through every witness is at least this long. Any penalties
/// give a valid bound; the steps only make it larger.
pub fn held_karp(d: &Distances, iterations: usize) -> f64 {
    if d.len() < 3 {
        return 0.0;
    }

    let mut penalty = vec![0.0; d.len()];
    let mut best = (f64::MIN, penalty.clone());
    let mut scale = 1.0;
    let mut stalled = 0;

    for _ in 0..iterations {
        let (weight, degrees) = one_tree(d, &penalty);
        if weight > best.0 {
            best = (weight, penalty.clone());
            stalled = 0;
        } else {
            stalled += 1;
        }

        // stuck: smaller steps from the best penalties so far
        if stalled == STALL {
            scale /= 2.0;
            stalled = 0;
            penalty.clone_from(&best.1);
            continue;
        }

        let steps: Vec<f64> = degrees.iter().map(|&deg| deg as f64 - 2.0).collect();
        let norm: f64 = steps.iter().map(|s| s * s).sum();
        if norm == 0.0 {
            break;
        }

        // aim a tenth above the best bound, as in Polyak's step
        let size = scale * 0.1 * best.0.max(1.0) / norm;
        for (p, s) in penalty.iter_mut().zip(&steps) {
            *p += size * s;
        }
    }

    best.0
}

/// The penalized 1-tree: a spanning tree of witnesses `1..n` plus the two
/// cheapest edges of witness 0. Returns its weight less twice the penalties,
/// and each witness's degree.
fn one_tree(d: &Distances, penalty: &[f64]) -> (f64, Vec<usize>) {
    let n = d.len();
    let cost = |i: usize, j: usize| d.get(i, j) as f64 + penalty[i] + penalty[j];

    let mut degrees = vec![0; n];
    let mut weight = 0.0;

    // Prim over 1..n
    let mut best: Vec<(f64, usize)> = (0..n).map(|j| (cost(1, j), 1)).collect();
    let mut inside = vec![false; n];
    inside[0] = true;
    inside[1] = true;
    for _ in 2..n {
        let next = (2..n)
            .filter(|&j| !inside[j])
            .min_by(|&a, &b| best[a].0.total_cmp(&best[b].0))
            .expect("a witness is left");

        let (edge, from) = best[next];
        weight += edge;
        degrees[next] += 1;
        degrees[from] += 1;
        inside[next] = true;

        for j in (2..n).filter(|&j| !inside[j]) {
            if cost(next, j) < best[j].0 {
                best[j] = (cost(next, j), next);
            }
        }
    }

    // the two cheapest edges of witness 0
    let mut edges: Vec<(f64, usize)> = (1..n).map(|j| (cost(0, j), j)).collect();
    edges.sort_by(|a, b| a.0.total_cmp(&b.0));
    for &(edge, j) in &edges[..2] {
        weight += edge;
        degrees[j] += 1;
    }
    degrees[0] = 2;

    (weight - 2.0 * penalty.iter().sum::<f64>(), degrees)
}

/// The cheapest forest of at most `parts` trees spanning every witness: the
/// spanning tree less its `parts - 1` heaviest edges. `parts` closed tours
/// together are at least this long, each having a tree through its witnesses.
pub fn forest(d: &Distances, parts: usize) -> u64 {
    let mut edges = spanning_tree_edges(d);
    edges.sort_unstable();

    let kept = edges.len().saturating_sub(parts.saturating_sub(1));
    edges[..kept].iter().sum()
}

/// The edge weights of a minimum spanning tree (Prim).
fn spanning_tree_edges(d: &Distances) -> Vec<u64> {
    let n = d.len();
    if n < 2 {
        return Vec::new();
    }

    let mut best: Vec<u64> = (0..n).map(|j| d.get(0, j)).collect();
    let mut inside = vec![false; n];
    inside[0] = true;

    let mut edges = Vec::with_capacity(n - 1);
    for _ in 1..n {
        let next = (0..n)
            .filter(|&j| !inside[j])
            .min_by_key(|&j| best[j])
            .expect("a witness is left");

        edges.push(best[next]);
        inside[next] = true;
        for j in (0..n).filter(|&j| !inside[j]) {
            best[j] = best[j].min(d.get(next, j));
        }
    }

    edges
}

/// Among the first `limit` witnesses, the `parts + 1` farthest apart: two of
/// them share a tour, which goes there and back. Twice the smallest gap.
pub fn dispersion(d: &Distances, parts: usize, limit: usize) -> u64 {
    let n = d.len().min(limit);
    let mut best = 0;
    let mut picked = Vec::with_capacity(parts + 1);

    spread(d, n, parts + 1, 0, u64::MAX, &mut picked, &mut best);

    2 * best
}

/// Depth first over increasing index sets, pruning once the smallest gap
/// cannot beat `best`.
fn spread(
    d: &Distances,
    n: usize,
    size: usize,
    from: usize,
    gap: u64,
    picked: &mut Vec<usize>,
    best: &mut u64,
) {
    if picked.len() == size {
        *best = (*best).max(gap);
        return;
    }

    for i in from..n {
        let with = picked
            .iter()
            .map(|&j| d.get(i, j))
            .min()
            .unwrap_or(u64::MAX)
            .min(gap);
        if with <= *best {
            continue;
        }

        picked.push(i);
        spread(d, n, size, i + 1, with, picked, best);
        picked.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Witnesses on a line at the given positions.
    fn line(at: &[i64]) -> Distances {
        Distances::new(at.len(), |i, j| at[i].abs_diff(at[j]))
    }

    #[test]
    fn held_karp_finds_the_tour_on_a_line() {
        let d = line(&[0, 10, 3, 7, 5]);

        // the best cycle goes out and back: 20; a plain 1-tree gives less
        let (plain, _) = one_tree(&d, &[0.0; 5]);
        assert!(plain < 20.0);
        let bound = held_karp(&d, 200);
        assert!(bound <= 20.0 + 1e-6);
        assert!(bound >= 19.5);
    }

    #[test]
    fn held_karp_never_exceeds_a_square_tour() {
        // corners of a 10 x 10 square: the tour is 40
        let corners: [(i64, i64); 4] = [(0, 0), (10, 0), (10, 10), (0, 10)];
        let d = Distances::new(4, |i, j| {
            let (a, b) = (corners[i], corners[j]);
            a.0.abs_diff(b.0) + a.1.abs_diff(b.1)
        });

        let bound = held_karp(&d, 200);
        assert!(bound <= 40.0 + 1e-6);
        assert!(bound >= 39.0);
    }

    #[test]
    fn forest_drops_the_heaviest_edges() {
        let d = line(&[0, 1, 2, 12, 13]);

        assert_eq!(forest(&d, 1), 13);
        assert_eq!(forest(&d, 2), 3);
        assert_eq!(forest(&d, 3), 2);
    }

    #[test]
    fn dispersion_picks_spread_out_witnesses() {
        let d = line(&[0, 10, 5, 1, 9]);

        assert_eq!(dispersion(&d, 1, 5), 20);
        assert_eq!(dispersion(&d, 2, 5), 10);
        assert_eq!(dispersion(&d, 2, 2), 0);
    }
}
