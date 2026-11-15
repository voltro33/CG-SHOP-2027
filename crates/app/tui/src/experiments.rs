//! A/B experiments: two kept binaries run `lns` side by side on the same
//! instances, from copies of the same bests and search state, with the same
//! seed. Each lives in `data/rust/experiments/<id>/`:
//!
//! - `experiment.json`, the [`Setup`] and each instance's start length;
//! - `base/` and `fast/`, each arm's `best/` and `state/`;
//! - `base.exe`, `fast.exe`, the binaries as they were when it started.
//!
//! Its output is in the run named in the setup.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mow_report::bests::read_bests;
use mow_report::family::pessimistic_score;
use serde::{Deserialize, Serialize};

use crate::binaries::Binary;
use crate::clock::{now, stamp};
use crate::data::{Lawn, Paths};
use crate::recipes::{exe, exec, numbers, Set};
use crate::runs::{Spec, Step};

pub const ARMS: [&str; 2] = ["base", "fast"];

const SETUP: &str = "experiment.json";

/// What an experiment compares and on what.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Setup {
    pub id: String,
    /// The binary labels of the two arms.
    pub base: String,
    pub fast: String,
    pub set: String,
    pub seconds: u32,
    pub threads: usize,
    pub seed: u64,
    pub made: u64,
    /// The run that runs it.
    pub run: String,
    /// Each instance's best when it started.
    pub start: BTreeMap<String, u64>,
}

/// The choices of a new experiment.
#[derive(Debug, Clone)]
pub struct Choice {
    pub base: Binary,
    pub fast: Binary,
    pub set: Set,
    pub seconds: u32,
    /// Per arm.
    pub threads: usize,
    pub seed: u64,
}

pub fn experiments_dir(paths: &Paths) -> PathBuf {
    paths.data.join("experiments")
}

/// Lays out a new experiment and returns the run that runs it.
pub fn create(paths: &Paths, lawn: &Lawn, choice: &Choice) -> Result<Spec, String> {
    let uids = choice.set.uids(paths, lawn)?;
    let start: BTreeMap<String, u64> = uids
        .iter()
        .filter_map(|uid| Some((uid.clone(), lawn.find(uid)?.best.as_ref()?.objective)))
        .collect();
    if start.is_empty() {
        return Err(format!("{} holds no instance with a best", choice.set.name()));
    }

    let id = format!("{}-{}-vs-{}", stamp(now()), choice.base.label, choice.fast.label);
    let dir = experiments_dir(paths).join(&id);
    if dir.exists() {
        return Err(format!("{} exists already", dir.display()));
    }
    for arm in ARMS {
        copy_arm(paths, &dir.join(arm), start.keys())?;
    }

    let list = dir.join("instances.txt");
    let names: Vec<&str> = start.keys().map(String::as_str).collect();
    std::fs::write(&list, names.join("\n") + "\n").map_err(|e| e.to_string())?;

    let run = crate::recipes::run_id("experiment");
    let setup = Setup {
        id: id.clone(),
        base: choice.base.label.clone(),
        fast: choice.fast.label.clone(),
        set: choice.set.name(),
        seconds: choice.seconds,
        threads: choice.threads,
        seed: choice.seed,
        made: now(),
        run: run.clone(),
        start,
    };
    let json = serde_json::to_string_pretty(&setup).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(SETUP), json).map_err(|e| e.to_string())?;

    let copies = [(&choice.base, "base"), (&choice.fast, "fast")].map(|(binary, arm)| Step::Copy {
        name: format!("{arm} binary"),
        from: binary.path.clone(),
        to: dir.join(exe(arm)),
    });
    let arms = ARMS.map(|arm| {
        let mut args = vec![
            "lns".to_owned(),
            "--bench".to_owned(),
            list.to_string_lossy().into_owned(),
            "--out".to_owned(),
            dir.join(arm).to_string_lossy().into_owned(),
        ];
        args.extend(numbers(&[
            ("--seconds", choice.seconds as usize),
            ("--threads", choice.threads),
            ("--verifiers", 1),
            ("--seed", choice.seed as usize),
        ]));
        exec(arm, &dir.join(exe(arm)), args, &paths.root, paths)
    });

    Ok(Spec {
        id: run,
        kind: "experiment".to_owned(),
        title: format!(
            "{} vs {} on {}, {} s each",
            choice.base.label, choice.fast.label, setup.set, choice.seconds
        ),
        started: now(),
        score_before: lawn.score(),
        data: paths.data.clone(),
        instances: paths.instances.clone(),
        files: Vec::new(),
        phases: vec![copies.to_vec(), arms.to_vec()],
    })
}

/// Copies the stored best and search state of each uid into an arm.
fn copy_arm<'a>(paths: &Paths, arm: &Path, uids: impl Iterator<Item = &'a String>) -> Result<(), String> {
    let (best, state) = (arm.join("best"), arm.join("state"));
    for dir in [&best, &state] {
        std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }

    for uid in uids {
        let name = format!("{uid}.solution.json");
        std::fs::copy(paths.best_dir().join(&name), best.join(&name)).map_err(|e| format!("{uid}: {e}"))?;

        let saved = paths.data.join("state").join(format!("{uid}.json"));
        if saved.is_file() {
            std::fs::copy(&saved, state.join(format!("{uid}.json"))).map_err(|e| format!("{uid}: {e}"))?;
        }
    }

    Ok(())
}

/// Every experiment, newest first.
pub fn list(paths: &Paths) -> Vec<(PathBuf, Setup)> {
    let Ok(entries) = std::fs::read_dir(experiments_dir(paths)) else {
        return Vec::new();
    };

    let mut found: Vec<(PathBuf, Setup)> = entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            let dir = e.path();
            let text = std::fs::read_to_string(dir.join(SETUP)).ok()?;
            Some((dir, serde_json::from_str(&text).ok()?))
        })
        .collect();
    found.sort_by(|a, b| b.1.made.cmp(&a.1.made));

    found
}

/// One instance in the report.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub uid: String,
    pub start: u64,
    /// Each arm's best now.
    pub base: u64,
    pub fast: u64,
    /// Moves each arm made, from its log.
    pub moves: (Option<u64>, Option<u64>),
}

/// The verdict so far.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub lines: Vec<Line>,
    /// Points each arm added over the start.
    pub points: (f64, f64),
    /// Instances each arm shortened.
    pub better: (usize, usize),
    /// Instances where each arm ended shorter than the other.
    pub ahead: (usize, usize),
    /// Median of fast moves over base moves.
    pub moves_ratio: Option<f64>,
}

/// Scores the arms against the start. `logs` are the run's `base.log` and
/// `fast.log`, for the move counts.
pub fn report(dir: &Path, setup: &Setup, lawn: &Lawn, logs: (&Path, &Path)) -> Result<Report, String> {
    let arms = [
        read_bests(&dir.join("base").join("best"))?,
        read_bests(&dir.join("fast").join("best"))?,
    ];
    let moves = (moves_in(logs.0), moves_in(logs.1));

    let mut lines = Vec::new();
    let (mut points, mut better, mut ahead) = ((0.0, 0.0), (0, 0), (0, 0));
    for (uid, &start) in &setup.start {
        let end = |i: usize| arms[i].get(uid).map_or(start, |b| b.objective.min(start));
        let (base, fast) = (end(0), end(1));
        let bound = lawn.find(uid).map_or(0, |i| i.lower_bound);
        let gained = |o: u64| {
            pessimistic_score(bound, o).unwrap_or(0.0) - pessimistic_score(bound, start).unwrap_or(0.0)
        };

        points = (points.0 + gained(base), points.1 + gained(fast));
        better = (
            better.0 + usize::from(base < start),
            better.1 + usize::from(fast < start),
        );
        ahead = (
            ahead.0 + usize::from(base < fast),
            ahead.1 + usize::from(fast < base),
        );
        lines.push(Line {
            uid: uid.clone(),
            start,
            base,
            fast,
            moves: (moves.0.get(uid).copied(), moves.1.get(uid).copied()),
        });
    }

    let mut ratios: Vec<f64> = lines
        .iter()
        .filter_map(|l| match l.moves {
            (Some(b), Some(f)) if b > 0 => Some(f as f64 / b as f64),
            _ => None,
        })
        .collect();
    ratios.sort_by(f64::total_cmp);

    Ok(Report {
        lines,
        points,
        better,
        ahead,
        moves_ratio: ratios.get(ratios.len() / 2).copied(),
    })
}

/// Moves per instance from lines `lns <uid>: a -> b in N moves, ...`.
fn moves_in(log: &Path) -> BTreeMap<String, u64> {
    let text = std::fs::read_to_string(log).unwrap_or_default();

    text.lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("lns ")?;
            let (uid, rest) = rest.split_once(": ")?;
            let count = rest
                .split(" in ")
                .nth(1)?
                .split_whitespace()
                .next()?
                .parse()
                .ok()?;
            Some((uid.to_owned(), count))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runs::tests::scratch;

    fn put(dir: &Path, uid: &str, side: i32) {
        std::fs::create_dir_all(dir).unwrap();
        let json = format!(
            "{{\"instance_uid\":\"{uid}\",\"tours\":[{{\"x\":[0,{side},{side},0],\"y\":[0,0,{side},{side}]}}],\"meta\":{{}}}}"
        );
        std::fs::write(dir.join(format!("{uid}.solution.json")), json).unwrap();
    }

    #[test]
    fn the_report_scores_each_arm_against_the_start() {
        let dir = scratch("experiment");
        put(&dir.join("base").join("best"), "a", 10); // 40, as it started
        put(&dir.join("fast").join("best"), "a", 9); // 36
        put(&dir.join("base").join("best"), "b", 9);
        put(&dir.join("fast").join("best"), "b", 10);
        std::fs::write(
            dir.join("base.log"),
            "lns a: 40 -> 40 in 100 moves, 0 bests\nlns b: 40 -> 36 in 300 moves, 1 bests\n",
        )
        .unwrap();
        std::fs::write(dir.join("fast.log"), "lns a: 40 -> 36 in 250 moves, 1 bests\n").unwrap();

        let setup = Setup {
            id: "x".to_owned(),
            base: "old".to_owned(),
            fast: "new".to_owned(),
            set: "bench".to_owned(),
            seconds: 1,
            threads: 1,
            seed: 1,
            made: 0,
            run: "r".to_owned(),
            start: [("a".to_owned(), 40), ("b".to_owned(), 40)].into(),
        };
        let mut lawn = Lawn::default();
        for uid in ["a", "b"] {
            let mut i = crate::data::tests::instance(uid, 1, "square", Some(40));
            i.lower_bound = 20;
            lawn.instances.push(i);
        }

        let r = report(
            &dir,
            &setup,
            &lawn,
            (&dir.join("base.log"), &dir.join("fast.log")),
        )
        .unwrap();

        assert_eq!(r.better, (1, 1));
        assert_eq!(r.ahead, (1, 1));
        assert!((r.points.0 - r.points.1).abs() < 1e-12);
        assert_eq!(r.lines[0].moves, (Some(100), Some(250)));
        assert_eq!(r.moves_ratio, Some(2.5));

        std::fs::remove_dir_all(dir).unwrap();
    }
}
