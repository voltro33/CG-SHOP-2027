//! How `mow lns` shares its seconds among the instances.
//!
//! - [`Allocation::Even`]: every instance gets `--seconds`, as before.
//! - [`Allocation::Weighted`]: the same total, shared by the points each
//!   instance is expected to gain per second. Gaining a fraction `g` of its
//!   tour length lifts an instance's points `(lb / objective)²` by about
//!   `2 g × points`, so the weight is `points × gain rate`, the rate being
//!   the relative shortening per second over the last [`WINDOW`] passes in
//!   the job log.
//! - A stalled instance keeps a floor of [`FLOOR_SHARE`] of the median rate,
//!   and one without history the median itself; the seconds are clamped to
//!   [`SHORTEST`] to [`LONGEST`] times `--seconds` with the total kept.

use std::collections::HashMap;

use mow_store::JobRecord;

/// Passes of history a gain rate looks back on.
pub const WINDOW: usize = 2;

/// The rate a stalled instance keeps, as a share of the median rate.
pub const FLOOR_SHARE: f64 = 0.2;

/// The least and most seconds of an instance, as multiples of `--seconds`.
pub const SHORTEST: f64 = 0.2;
pub const LONGEST: f64 = 5.0;

/// Bisection steps when solving for the clamped split.
const STEPS: usize = 200;

/// How the seconds are shared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Allocation {
    Even,
    Weighted,
}

impl Allocation {
    pub fn parse(text: &str) -> Result<Self, String> {
        match text {
            "even" => Ok(Self::Even),
            "weighted" => Ok(Self::Weighted),
            _ => Err(format!("--allocate: {text:?} is not even or weighted")),
        }
    }
}

/// One finished LNS pass on an instance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pass {
    pub number: u32,
    pub objective: u64,
    pub seconds: f64,
}

/// The LNS passes in `records` per instance, in pass order. Failed jobs are
/// left out, and so are passes from `before` on (the one running now).
pub fn passes(records: &[JobRecord], before: Option<u32>) -> HashMap<String, Vec<Pass>> {
    let mut found: HashMap<String, Vec<Pass>> = HashMap::new();

    for record in records {
        let (Some(number), Some(objective)) = (pass_number(&record.label), record.feasible_objective())
        else {
            continue;
        };
        if before.is_some_and(|before| number >= before) {
            continue;
        }

        found.entry(record.instance.clone()).or_default().push(Pass {
            number,
            objective,
            seconds: record.seconds,
        });
    }

    for list in found.values_mut() {
        list.sort_by_key(|pass| pass.number);
    }
    found
}

/// The pass of an LNS label (`lns/temperature=T/pass=N`), 0 without one;
/// `None` for any other method.
pub fn pass_number(label: &str) -> Option<u32> {
    let mut parts = label.split('/');
    if parts.next() != Some(super::lns::METHOD) {
        return None;
    }

    let pass = parts.find_map(|part| part.strip_prefix("pass="));
    match pass {
        None => Some(0),
        Some(text) => text.parse::<f64>().ok().map(|n| n.max(0.0) as u32),
    }
}

/// The relative shortening of the best per second over the last [`WINDOW`]
/// passes; `None` with fewer than two passes.
///
/// A pass resumes from its saved walk and reports the best it saw from
/// there, which can be worse than the stored best; the best after a pass
/// is therefore the least objective of all passes up to it.
pub fn gain_rate(passes: &[Pass]) -> Option<f64> {
    if passes.len() < 2 {
        return None;
    }

    let bests: Vec<u64> = passes
        .iter()
        .scan(u64::MAX, |best, pass| {
            *best = (*best).min(pass.objective);
            Some(*best)
        })
        .collect();
    let first = passes.len().saturating_sub(WINDOW + 1);
    let (from, to) = (bests[first], bests[passes.len() - 1]);
    let seconds: f64 = passes[first + 1..].iter().map(|pass| pass.seconds).sum();

    match from > 0 && seconds > 0.0 {
        true => Some(from.saturating_sub(to) as f64 / from as f64 / seconds),
        false => None,
    }
}

/// `points × rate` per instance. A missing rate is the median of the known
/// ones, and no rate is below [`FLOOR_SHARE`] of that median.
pub fn weights(points: &[f64], rates: &[Option<f64>]) -> Vec<f64> {
    let known: Vec<f64> = rates.iter().flatten().copied().collect();
    let typical = median(&known).unwrap_or(1.0);
    let floor = FLOOR_SHARE * typical;

    points
        .iter()
        .zip(rates)
        .map(|(points, rate)| points * rate.unwrap_or(typical).max(floor))
        .collect()
}

/// Seconds per instance in proportion to `weights`, each clamped to
/// [`SHORTEST`] to [`LONGEST`] times `mean`, summing to `mean × n`. Even
/// when the weights say nothing (all zero or not finite).
pub fn split(weights: &[f64], mean: f64) -> Vec<f64> {
    let even = vec![mean; weights.len()];
    let usable = weights.iter().all(|w| w.is_finite() && *w >= 0.0);
    let heaviest = weights.iter().copied().fold(0.0, f64::max);
    if !usable || heaviest <= 0.0 || mean <= 0.0 {
        return even;
    }

    let (low, high) = (SHORTEST * mean, LONGEST * mean);
    let total = mean * weights.len() as f64;
    let clamped = |scale: f64| -> Vec<f64> { weights.iter().map(|w| (scale * w).clamp(low, high)).collect() };

    // the sum grows with the scale; at the top one every positive weight
    // is at `high`, so a total out of reach there means too many zeros
    let lightest = weights
        .iter()
        .copied()
        .filter(|w| *w > 0.0)
        .fold(heaviest, f64::min);
    let (mut below, mut above) = (0.0, high / lightest);
    if clamped(above).iter().sum::<f64>() < total {
        return even;
    }

    for _ in 0..STEPS {
        let middle = 0.5 * (below + above);
        match clamped(middle).iter().sum::<f64>() < total {
            true => below = middle,
            false => above = middle,
        }
    }

    clamped(above)
}

/// The least, median and most of `seconds`.
pub fn spread(seconds: &[f64]) -> Option<(f64, f64, f64)> {
    let least = seconds.iter().copied().reduce(f64::min)?;
    let most = seconds.iter().copied().reduce(f64::max)?;

    Some((least, median(seconds)?, most))
}

fn median(values: &[f64]) -> Option<f64> {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);

    let n = sorted.len();
    match n {
        0 => None,
        _ if n % 2 == 1 => Some(sorted[n / 2]),
        _ => Some(0.5 * (sorted[n / 2 - 1] + sorted[n / 2])),
    }
}

#[cfg(test)]
#[path = "allocate_tests.rs"]
mod tests;
