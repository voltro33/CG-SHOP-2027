//! The leaderboard (`report.leaderboard_rows`): per instance with a stored
//! best, the baseline, the best, and how far both are from the lower bound.
//!
//! - `baseline`: the best objective of the baseline rounds 0 and 1
//! - `gap_pct`: `(best - lower_bound) / lower_bound` in percent
//! - `vs_baseline_pct`: how much shorter than the baseline, in percent
//! - `score_gain`: `(baseline / best) ** 2`
//! - `pessimistic_score`: `(lower_bound / best) ** 2`

use std::collections::{BTreeMap, HashMap};

use mow_store::JobRecord;

use crate::bests::Best;
use crate::catalog::Features;
use crate::csv::line;
use crate::family::{family, pct, pessimistic_score};
use crate::pyfmt::{repr, round};

/// The rounds whose results are the baseline (`jobs.BASELINE_ROUNDS`).
pub const BASELINE_ROUNDS: [i32; 2] = [0, 1];

const HEADER: [&str; 13] = [
    "instance",
    "k",
    "cells",
    "cutter",
    "family",
    "lower_bound",
    "baseline",
    "best",
    "best_config",
    "gap_pct",
    "vs_baseline_pct",
    "score_gain",
    "pessimistic_score",
];

/// One leaderboard row. `k`, `cells` and `cutter` are missing when the
/// catalog does not know the instance.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub instance: String,
    pub k: Option<usize>,
    pub cells: Option<usize>,
    pub cutter: Option<String>,
    pub family: &'static str,
    pub lower_bound: u64,
    pub baseline: Option<u64>,
    pub best: u64,
    pub best_config: String,
    pub gap_pct: Option<f64>,
    pub vs_baseline_pct: Option<f64>,
    pub score_gain: Option<f64>,
    pub pessimistic_score: Option<f64>,
}

/// The best objective each instance reached in the baseline rounds.
/// Like Python, a record counts whenever it has an objective.
pub fn baselines(records: &[JobRecord]) -> HashMap<&str, u64> {
    let mut baseline: HashMap<&str, u64> = HashMap::new();

    for record in records {
        let Some(objective) = record.objective else {
            continue;
        };
        if !BASELINE_ROUNDS.contains(&record.round) {
            continue;
        }

        let entry = baseline.entry(record.instance.as_str()).or_insert(objective);
        *entry = (*entry).min(objective);
    }

    baseline
}

/// One row per stored best, by instance name.
pub fn rows(catalog: &[Features], bests: &BTreeMap<String, Best>, records: &[JobRecord]) -> Vec<Row> {
    let info: HashMap<&str, &Features> = catalog.iter().map(|f| (f.instance.as_str(), f)).collect();
    let baseline = baselines(records);

    bests
        .iter()
        .map(|(uid, best)| {
            row(
                uid,
                best,
                info.get(uid.as_str()).copied(),
                baseline.get(uid.as_str()).copied(),
            )
        })
        .collect()
}

fn row(uid: &str, best: &Best, info: Option<&Features>, base: Option<u64>) -> Row {
    let bound = info.map_or(0, |f| f.lower_bound);
    let objective = best.objective;
    // a zero baseline is shown but, as in Python, compares to nothing
    let nonzero = base.filter(|&b| b > 0);

    Row {
        instance: uid.to_owned(),
        k: info.map(|f| f.k),
        cells: info.map(|f| f.cells),
        cutter: info.map(|f| f.cutter_kind.clone()),
        family: family(uid),
        lower_bound: bound,
        baseline: base,
        best: objective,
        best_config: best.label.clone(),
        gap_pct: (bound > 0).then(|| pct(objective as f64, bound as f64)),
        vs_baseline_pct: nonzero.map(|b| pct(objective as f64, b as f64)),
        score_gain: nonzero.map(|b| round((b as f64 / objective as f64).powi(2), 4)),
        pessimistic_score: pessimistic_score(bound, objective).map(|s| round(s, 4)),
    }
}

fn text<T: ToString>(value: Option<T>) -> String {
    value.map_or_else(String::new, |v| v.to_string())
}

fn float(value: Option<f64>) -> String {
    value.map_or_else(String::new, repr)
}

impl Row {
    fn fields(&self) -> Vec<String> {
        vec![
            self.instance.clone(),
            text(self.k),
            text(self.cells),
            self.cutter.clone().unwrap_or_default(),
            self.family.to_owned(),
            self.lower_bound.to_string(),
            text(self.baseline),
            self.best.to_string(),
            self.best_config.clone(),
            float(self.gap_pct),
            float(self.vs_baseline_pct),
            float(self.score_gain),
            float(self.pessimistic_score),
        ]
    }
}

/// `leaderboard.csv` as `report.write_leaderboard` writes it; empty
/// without rows, where Python writes nothing.
pub fn to_csv(rows: &[Row]) -> String {
    if rows.is_empty() {
        return String::new();
    }

    let header: Vec<String> = HEADER.iter().map(|&name| name.to_owned()).collect();
    let mut csv = line(&header, "\n");
    for row in rows {
        csv += &line(&row.fields(), "\n");
    }

    csv
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(instance: &str, round: i32, objective: Option<u64>) -> JobRecord {
        JobRecord {
            job_id: format!("{instance}|lanes-h|s0"),
            instance: instance.to_owned(),
            round,
            label: "lanes-h".to_owned(),
            objective,
            seconds: 1.0,
            error: None,
        }
    }

    fn best(objective: u64) -> Best {
        Best {
            objective,
            label: "a_1s2|lanes-h|s0|v2".to_owned(),
            path: "a.solution.json".into(),
        }
    }

    #[test]
    fn baseline_takes_rounds_zero_and_one_only() {
        let records = [
            record("a_1s2", 0, Some(120)),
            record("a_1s2", 1, Some(110)),
            record("a_1s2", 2, Some(90)),
            record("a_1s2", 1, None),
        ];
        let bests = BTreeMap::from([("a_1s2".to_owned(), best(100)), ("b_1x3".to_owned(), best(40))]);

        let rows = rows(&[], &bests, &records);

        assert_eq!(rows[0].baseline, Some(110));
        assert_eq!(rows[0].vs_baseline_pct, Some(-9.1));
        assert_eq!(rows[0].score_gain, Some(1.21));
        assert_eq!(rows[0].gap_pct, None);
        assert_eq!(rows[1].family, "X");
        assert_eq!(rows[1].baseline, None);

        let csv = to_csv(&rows);
        assert!(csv.contains("\na_1s2,,,,square,0,110,100,a_1s2|lanes-h|s0|v2,,-9.1,1.21,\n"));
    }
}
