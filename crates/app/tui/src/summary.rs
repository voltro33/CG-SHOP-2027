//! The dashboard's numbers: the score and the gaps, overall and by group.

use mow_report::family::median;

use crate::data::{Instance, Lawn};

/// The score and gaps of some instances.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub name: String,
    pub count: usize,
    /// Instances with a stored best.
    pub solved: usize,
    pub points: f64,
    /// Median gap to the bound in percent, over the solved instances.
    pub median_gap: Option<f64>,
}

impl Group {
    fn of<'a>(name: impl Into<String>, members: impl Iterator<Item = &'a Instance>) -> Self {
        let members: Vec<&Instance> = members.collect();
        let gaps: Vec<f64> = members.iter().filter_map(|i| i.gap()).collect();

        Self {
            name: name.into(),
            count: members.len(),
            solved: gaps.len(),
            points: members.iter().map(|i| i.points()).sum(),
            median_gap: median(&gaps),
        }
    }
}

/// Every group the dashboard shows.
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    pub all: Group,
    pub by_k: Vec<Group>,
    pub by_family: Vec<Group>,
}

impl Summary {
    pub fn of(lawn: &Lawn) -> Self {
        let all = Group::of("all", lawn.instances.iter());

        let mut ks: Vec<usize> = lawn.instances.iter().map(|i| i.k).collect();
        ks.sort_unstable();
        ks.dedup();
        let by_k = ks
            .into_iter()
            .map(|k| Group::of(format!("k = {k}"), lawn.instances.iter().filter(|i| i.k == k)))
            .collect();

        let mut families: Vec<&str> = lawn.instances.iter().map(|i| i.family).collect();
        families.sort_unstable();
        families.dedup();
        let by_family = families
            .into_iter()
            .map(|f| Group::of(f, lawn.instances.iter().filter(|i| i.family == f)))
            .collect();

        Self { all, by_k, by_family }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::tests::instance;

    #[test]
    fn groups_split_by_k_and_family() {
        let lawn = Lawn {
            instances: vec![
                instance("a", 1, "square", Some(100)),
                instance("b", 1, "X", Some(200)),
                instance("c", 3, "X", Some(125)),
                instance("d", 3, "X", None),
            ],
            latest: None,
        };

        let s = Summary::of(&lawn);

        assert_eq!(s.all.count, 4);
        assert_eq!(s.all.solved, 3);
        assert_eq!(s.all.median_gap, Some(25.0));
        assert_eq!(s.by_k.len(), 2);
        assert_eq!(s.by_k[1].name, "k = 3");
        assert_eq!(s.by_k[1].solved, 1);
        assert_eq!(s.by_family[0].name, "X");
        assert_eq!(s.by_family[0].count, 3);
        assert!((s.by_family[0].points - (0.25 + 0.64)).abs() < 1e-12);
    }
}
