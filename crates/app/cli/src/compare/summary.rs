//! Wins, ties and losses, and the geometric mean ratio, per group of rows.

use super::rows::Row;

/// Rust against one Python column over some rows. Lower is better.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tally {
    pub wins: usize,
    pub ties: usize,
    pub losses: usize,
    /// Geometric mean of Rust / Python over the rows both sides solved.
    pub geomean: Option<f64>,
}

impl Tally {
    pub fn of(ratios: impl Iterator<Item = f64>) -> Self {
        let mut tally = Self::default();
        let mut log_sum = 0.0;

        for ratio in ratios {
            match ratio.partial_cmp(&1.0) {
                Some(std::cmp::Ordering::Less) => tally.wins += 1,
                Some(std::cmp::Ordering::Equal) => tally.ties += 1,
                _ => tally.losses += 1,
            }
            log_sum += ratio.ln();
        }

        let compared = tally.compared();
        if compared > 0 {
            tally.geomean = Some((log_sum / compared as f64).exp());
        }

        tally
    }

    pub fn compared(&self) -> usize {
        self.wins + self.ties + self.losses
    }
}

/// One line of the summary: a group of instances.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub name: String,
    pub instances: usize,
    pub solved: usize,
    pub vs_best: Tally,
    pub vs_mined: Tally,
}

impl Group {
    pub fn of<'a>(name: impl Into<String>, rows: impl Iterator<Item = &'a Row> + Clone) -> Self {
        Self {
            name: name.into(),
            instances: rows.clone().count(),
            solved: rows.clone().filter(|r| r.rust.is_some()).count(),
            vs_best: Tally::of(rows.clone().filter_map(Row::ratio_best)),
            vs_mined: Tally::of(rows.filter_map(Row::ratio_mined)),
        }
    }
}

/// All instances, then each k, then the bench set.
pub fn groups(rows: &[Row]) -> Vec<Group> {
    let mut groups = vec![Group::of("all", rows.iter())];

    let mut ks: Vec<usize> = rows.iter().map(|r| r.k).collect();
    ks.sort_unstable();
    ks.dedup();
    for k in ks {
        groups.push(Group::of(
            format!("k = {k}"),
            rows.iter().filter(move |r| r.k == k),
        ));
    }

    groups.push(Group::of("bench", rows.iter().filter(|r| r.bench)));
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_and_averages_ratios() {
        let tally = Tally::of([0.5, 1.0, 2.0].into_iter());

        assert_eq!((tally.wins, tally.ties, tally.losses), (1, 1, 1));
        assert!((tally.geomean.unwrap() - 1.0).abs() < 1e-12);
        assert_eq!(Tally::of(std::iter::empty()).geomean, None);
    }
}
