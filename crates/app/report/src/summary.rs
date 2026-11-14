//! The headline numbers of the leaderboard (`report.summary`), as the JSON
//! `python -m solver.report` prints.

use crate::family::{family, median, pct, pessimistic_score};
use crate::leaderboard::Row;
use crate::pyfmt::{round, Json};

/// How many instances the lists of wins and gaps name.
const LISTED: usize = 5;

/// Median gap to the bound in percent per cutter family, widest gap first;
/// families tie in the order they first appear.
pub fn median_gap_by_family(rows: &[Row]) -> Vec<(&'static str, f64)> {
    let mut gaps: Vec<(&'static str, Vec<f64>)> = Vec::new();

    for row in rows {
        let Some(gap) = row.gap_pct else { continue };

        let name = family(&row.instance);
        match gaps.iter_mut().find(|(n, _)| *n == name) {
            Some((_, values)) => values.push(gap),
            None => gaps.push((name, vec![gap])),
        }
    }

    let mut medians: Vec<(&'static str, f64)> = gaps
        .iter()
        .filter_map(|(name, values)| Some((*name, round(median(values)?, 1))))
        .collect();
    medians.sort_by(|a, b| b.1.total_cmp(&a.1));

    medians
}

/// The summary of `rows` over `total_instances` challenge instances.
pub fn summary(rows: &[Row], total_instances: usize) -> Json {
    let with_baseline: Vec<&Row> = rows
        .iter()
        .filter(|r| r.baseline.is_some_and(|b| b > 0))
        .collect();

    let baseline_sum: u64 = with_baseline.iter().filter_map(|r| r.baseline).sum();
    let best_sum: u64 = with_baseline.iter().map(|r| r.best).sum();

    let gaps: Vec<f64> = rows.iter().filter_map(|r| r.gap_pct).collect();
    let score_gain = with_baseline
        .iter()
        .filter_map(|r| r.score_gain)
        .fold(0.0, |a, b| a + b);

    // recomputed from bound and best so rounding in the csv does not add up
    let pessimistic: f64 = rows
        .iter()
        .filter_map(|r| pessimistic_score(r.lower_bound, r.best))
        .fold(0.0, |a, b| a + b);

    Json::object(vec![
        ("feasible", Json::int(rows.len())),
        ("instances", Json::int(total_instances)),
        ("baseline_sum", Json::int(baseline_sum)),
        ("best_sum", Json::int(best_sum)),
        (
            "sum_change_pct",
            Json::float((baseline_sum > 0).then(|| pct(best_sum as f64, baseline_sum as f64))),
        ),
        ("median_gap_pct", Json::float(median(&gaps).map(|m| round(m, 1)))),
        ("score_gain_total", Json::Float(round(score_gain, 2))),
        ("pessimistic_score", Json::Float(round(pessimistic, 1))),
        ("median_gap_by_family", by_family(rows)),
        ("biggest_wins", biggest_wins(&with_baseline)),
        ("worst_gaps", worst_gaps(rows)),
    ])
}

fn by_family(rows: &[Row]) -> Json {
    let pairs = median_gap_by_family(rows)
        .into_iter()
        .map(|(name, gap)| (name.to_owned(), Json::Float(gap)))
        .collect();

    Json::Object(pairs)
}

fn pair(row: &Row, value: Option<f64>) -> Json {
    Json::List(vec![Json::str(&row.instance), Json::float(value)])
}

/// The largest cuts against the baseline; the sort is stable, as Python's.
fn biggest_wins(with_baseline: &[&Row]) -> Json {
    let mut wins = with_baseline.to_vec();
    wins.sort_by(|a, b| key(a.vs_baseline_pct).total_cmp(&key(b.vs_baseline_pct)));

    Json::List(
        wins.iter()
            .take(LISTED)
            .map(|r| pair(r, r.vs_baseline_pct))
            .collect(),
    )
}

fn worst_gaps(rows: &[Row]) -> Json {
    let mut worst: Vec<&Row> = rows.iter().collect();
    worst.sort_by(|a, b| key(b.gap_pct).total_cmp(&key(a.gap_pct)));

    Json::List(worst.iter().take(LISTED).map(|r| pair(r, r.gap_pct)).collect())
}

/// `r["gap_pct"] or 0`.
fn key(value: Option<f64>) -> f64 {
    value.unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(uid: &str, bound: u64, best: u64) -> Row {
        Row {
            instance: uid.to_owned(),
            k: None,
            cells: None,
            cutter: None,
            family: family(uid),
            lower_bound: bound,
            baseline: None,
            best,
            best_config: String::new(),
            gap_pct: Some(round(100.0 * (best as f64 - bound as f64) / bound as f64, 1)),
            vs_baseline_pct: None,
            score_gain: None,
            pessimistic_score: None,
        }
    }

    #[test]
    fn summary_sums_pessimistic_scores() {
        let rows = [row("a_1s2", 50, 100), row("b_1s2", 90, 100), row("c_2x3", 10, 40)];

        // 0.25 plus 0.81 plus 0.0625
        let text = summary(&rows, 3).dump();
        assert!(text.contains("\"pessimistic_score\": 1.1,"), "{text}");
    }

    #[test]
    fn median_gap_by_family_sorts_widest_first() {
        let rows = [
            row("a_1s2", 100, 110),
            row("b_1s2", 100, 130),
            row("c_2x3", 100, 300),
        ];

        assert_eq!(median_gap_by_family(&rows), [("X", 200.0), ("square", 20.0)]);
    }
}
