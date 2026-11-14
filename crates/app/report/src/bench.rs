//! The fixed benchmark set (`bench.py`): picking it, and scoring a run on
//! it against the bests stored before.
//!
//! - self score: `sum (best known / ours) ** 2`, one point per instance
//!   means as good as before, more means better
//! - pessimistic score: `sum (lower bound / ours) ** 2`
//!
//! Running a config on the set is `mow mine --bench` or `mow solve`.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

use crate::catalog::Features;
use crate::csv::Table;
use crate::family::{family, median, pessimistic_score};
use crate::pyfmt::{round, Json};

/// Small enough that a whole run fits in a few minutes.
pub const MAX_CELLS: usize = 400_000;
pub const PER_STRATUM: usize = 2;

/// The worst X gaps at full size, where the most room is.
pub const WORST_X_LARGE: usize = 3;

/// Two instances per cutter family and k, at one third and two thirds of
/// the size order, plus the worst X gaps above the size cap from
/// `leaderboard.csv`. Deterministic for fixed inputs.
pub fn pick_bench(catalog: &[Features], leaderboard: &Table) -> Vec<String> {
    let mut strata: BTreeMap<(&str, String), Vec<(usize, &str)>> = BTreeMap::new();
    for row in catalog
        .iter()
        .filter(|r| r.is_challenge() && r.cells <= MAX_CELLS)
    {
        let key = (family(&row.instance), row.k.to_string());
        strata.entry(key).or_default().push((row.cells, &row.instance));
    }

    let mut chosen: Vec<String> = Vec::new();
    for members in strata.values_mut() {
        members.sort();

        let count = members.len();
        let mut picks: Vec<&str> = Vec::new();
        for share in 1..=PER_STRATUM {
            let uid = members[(count * share) / (PER_STRATUM + 1)].1;
            if !picks.contains(&uid) {
                picks.push(uid);
            }
        }
        chosen.extend(picks.into_iter().map(str::to_owned));
    }

    chosen.extend(worst_large_x(leaderboard));
    chosen
}

fn worst_large_x(leaderboard: &Table) -> Vec<String> {
    let mut large_x: Vec<(f64, &str)> = leaderboard
        .rows()
        .filter(|row| family(row.get("instance")) == "X")
        .filter(|row| row.parse::<usize>("cells").is_ok_and(|cells| cells > MAX_CELLS))
        .filter_map(|row| Some((-row.parse::<f64>("gap_pct").ok()?, row.get("instance"))))
        .collect();
    large_x.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(b.1)));

    large_x
        .into_iter()
        .take(WORST_X_LARGE)
        .map(|(_, uid)| uid.to_owned())
        .collect()
}

/// One instance of a scored run.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoreRow {
    pub instance: String,
    pub before: Option<u64>,
    pub ours: Option<u64>,
    pub ratio: f64,
}

/// The scores of one run.
#[derive(Debug, Clone, PartialEq)]
pub struct Score {
    pub instances: usize,
    pub feasible: usize,
    pub self_score: f64,
    pub pessimistic_score: f64,
    pub pessimistic_before: f64,
    pub median_gap_by_family: Vec<(&'static str, f64)>,
    pub rows: Vec<ScoreRow>,
}

/// Self score, pessimistic score and gap medians of a run over `uids`. An
/// instance without a result scores zero.
pub fn score_run(
    uids: &[String],
    bounds: &HashMap<String, u64>,
    before: &HashMap<String, u64>,
    ours: &HashMap<String, u64>,
) -> Score {
    let (mut self_score, mut pessimistic, mut pessimistic_before) = (0.0, 0.0, 0.0);
    let mut gaps: BTreeMap<&'static str, Vec<f64>> = BTreeMap::new();
    let mut rows = Vec::new();

    for uid in uids {
        let bound = bounds.get(uid).copied().unwrap_or(0);
        let was = before.get(uid).copied().filter(|&b| b > 0);
        let objective = ours.get(uid).copied();

        if let Some(was) = was {
            pessimistic_before += pessimistic_score(bound, was).unwrap_or(0.0);
        }

        let Some(objective) = objective else {
            rows.push(score_row(uid, was, None, 0.0));
            continue;
        };

        let ratio = was.map_or(1.0, |b| (b as f64 / objective as f64).powi(2));
        self_score += ratio;
        pessimistic += pessimistic_score(bound, objective).unwrap_or(0.0);

        if bound > 0 {
            let gap = 100.0 * (objective as f64 - bound as f64) / bound as f64;
            gaps.entry(family(uid)).or_default().push(gap);
        }
        rows.push(score_row(uid, was, Some(objective), ratio));
    }

    Score {
        instances: uids.len(),
        feasible: rows.iter().filter(|r| r.ours.is_some()).count(),
        self_score: round(self_score, 2),
        pessimistic_score: round(pessimistic, 2),
        pessimistic_before: round(pessimistic_before, 2),
        median_gap_by_family: gaps
            .iter()
            .filter_map(|(&name, values)| Some((name, round(median(values)?, 1))))
            .collect(),
        rows,
    }
}

fn score_row(uid: &str, before: Option<u64>, ours: Option<u64>, ratio: f64) -> ScoreRow {
    ScoreRow {
        instance: uid.to_owned(),
        before,
        ours,
        ratio,
    }
}

/// The run as `bench.py` prints it: one line per instance, then the scores.
pub fn report(score: &Score, config: &str) -> String {
    let mut out = format!("\nconfig: {config}\n");
    let _ = writeln!(
        out,
        "{:36} {:>8} {:>8} {:>7}",
        "instance", "before", "ours", "ratio"
    );

    let shown = |value: Option<u64>, missing: &str| value.map_or(missing.to_owned(), |v| v.to_string());
    for row in &score.rows {
        let _ = writeln!(
            out,
            "{:36} {:>8} {:>8} {:>7.3}",
            row.instance,
            shown(row.before, "None"),
            shown(row.ours, "FAIL"),
            row.ratio
        );
    }

    let medians = score
        .median_gap_by_family
        .iter()
        .map(|&(name, gap)| (name.to_owned(), Json::Float(gap)))
        .collect();
    let summary = Json::object(vec![
        ("instances", Json::int(score.instances)),
        ("feasible", Json::int(score.feasible)),
        ("self_score", Json::Float(score.self_score)),
        ("pessimistic_score", Json::Float(score.pessimistic_score)),
        ("pessimistic_before", Json::Float(score.pessimistic_before)),
        ("median_gap_by_family", Json::Object(medians)),
    ]);
    out + &summary.dump() + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn features(uid: &str, cells: usize, k: usize) -> Features {
        Features {
            source: "instances".to_owned(),
            instance: uid.to_owned(),
            cells,
            width: 1,
            height: 1,
            aspect: 1.0,
            fill: 1.0,
            holes: 0,
            k,
            cutter_cells: 1,
            cutter_width: 1,
            cutter_height: 1,
            cutter_kind: "square".to_owned(),
            step_gain: 1,
            lower_bound: 100,
            hull_bound: 100,
            witness_bound: 0,
        }
    }

    #[test]
    fn pick_bench_is_stratified_and_deterministic() {
        let mut catalog = Vec::new();
        for index in 0..6 {
            catalog.push(features(&format!("a{index}_10k_1s2"), 1000 * (index + 1), 1));
            catalog.push(features(&format!("b{index}_10k_2x3"), 1000 * (index + 1), 2));
        }

        // too big for the strata, but a candidate for the worst X list
        catalog.push(features("big_900k_3x5", 900_000, 3));
        let leaderboard = Table::parse("instance,cells,gap_pct\nbig_900k_3x5,900000,200.0\n");

        let first = pick_bench(&catalog, &leaderboard);
        assert_eq!(first, pick_bench(&catalog, &leaderboard));
        assert_eq!(first.len(), 5);
        assert_eq!(first[4], "big_900k_3x5");
        assert_eq!(first.iter().filter(|uid| uid.contains("_1s2")).count(), 2);
    }

    #[test]
    fn score_run_counts_failures_as_zero() {
        let uids = ["a_1s2".to_owned(), "b_1s2".to_owned()];
        let bounds = HashMap::from([("a_1s2".to_owned(), 50), ("b_1s2".to_owned(), 50)]);
        let before = HashMap::from([("a_1s2".to_owned(), 100), ("b_1s2".to_owned(), 100)]);
        let ours = HashMap::from([("a_1s2".to_owned(), 50)]);

        let score = score_run(&uids, &bounds, &before, &ours);

        // twice as short scores four, the failure scores nothing
        assert_eq!(score.self_score, 4.0);
        assert_eq!(score.feasible, 1);
        assert_eq!(score.pessimistic_score, 1.0);
        assert_eq!(score.pessimistic_before, 0.5);
        assert_eq!(score.median_gap_by_family, [("square", 0.0)]);
        let printed = report(&score, "x");
        assert!(
            printed.contains(
                "     100     FAIL   0.000
"
            ),
            "{printed}"
        );
        assert!(printed.contains("\"self_score\": 4.0,"), "{printed}");
    }
}
