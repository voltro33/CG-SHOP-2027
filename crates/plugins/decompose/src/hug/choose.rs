//! One residue per strip, chosen jointly: a shortest path over the strips
//! left to right, where keeping the residue across a border earns that
//! border's saving and changing it costs nothing extra.

use super::costs::Costs;

/// The residue of every strip that makes the summed cost smallest. Ties keep
/// the residue of the strip before, then prefer `preferred`, then the lowest.
pub(super) fn residues(costs: &Costs, preferred: usize) -> Vec<usize> {
    let (count, strips) = (costs.residues, costs.strips);
    if count == 0 || strips == 0 {
        return Vec::new();
    }

    let mut total: Vec<f64> = (0..count).map(|r| costs.cost(r, 0)).collect();
    let mut from = vec![0usize; count * strips];

    for s in 1..strips {
        let best = cheapest(&total, preferred);

        let next: Vec<f64> = (0..count)
            .map(|r| {
                let stay = total[r] + costs.saving(r, s - 1);
                let (came, before) = match stay <= total[best] {
                    true => (r, stay),
                    false => (best, total[best]),
                };

                from[s * count + r] = came;
                before + costs.cost(r, s)
            })
            .collect();
        total = next;
    }

    backtrack(&from, count, cheapest(&total, preferred))
}

/// The index of the smallest total, `preferred` winning ties.
fn cheapest(total: &[f64], preferred: usize) -> usize {
    let mut best = preferred.min(total.len() - 1);

    for (r, &value) in total.iter().enumerate() {
        if value < total[best] {
            best = r;
        }
    }

    best
}

fn backtrack(from: &[usize], count: usize, last: usize) -> Vec<usize> {
    let strips = from.len() / count;
    let mut chosen = vec![last; strips];

    for s in (1..strips).rev() {
        chosen[s - 1] = from[s * count + chosen[s]];
    }

    chosen
}

#[cfg(test)]
mod tests {
    use super::*;

    fn costs(cost: Vec<Vec<f64>>, saving: Vec<Vec<f64>>) -> Costs {
        let strips = cost[0].len();

        Costs {
            residues: cost.len(),
            strips,
            cost: cost.concat(),
            saving: saving.concat(),
        }
    }

    #[test]
    fn each_strip_takes_its_cheapest_residue_when_nothing_joins() {
        let c = costs(
            vec![vec![1.0, 5.0, 1.0], vec![4.0, 2.0, 4.0]],
            vec![vec![0.0; 3], vec![0.0; 3]],
        );

        assert_eq!(residues(&c, 0), vec![0, 1, 0]);
    }

    #[test]
    fn savings_keep_one_residue_across_borders() {
        // residue 1 is a little dearer in the middle strip, but joining saves more
        let c = costs(
            vec![vec![1.0, 2.0, 1.0], vec![4.0, 2.5, 4.0]],
            vec![vec![-1.0, -1.0, 0.0], vec![0.0; 3]],
        );

        assert_eq!(residues(&c, 1), vec![0, 0, 0]);
    }

    #[test]
    fn ties_go_to_the_preferred_residue() {
        let c = costs(
            vec![vec![1.0, 1.0], vec![1.0, 1.0]],
            vec![vec![0.0; 2], vec![0.0; 2]],
        );

        assert_eq!(residues(&c, 1), vec![1, 1]);
        assert_eq!(residues(&c, 0), vec![0, 0]);
    }
}
