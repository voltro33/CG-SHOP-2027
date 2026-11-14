//! Adaptive operator weights (Ropke and Pisinger 2006): an operator is drawn
//! by roulette over its weight; every [`SEGMENT`] moves each weight moves
//! towards the mean score its operator earned in that segment.

use serde::{Deserialize, Serialize};

/// Moves per weight update.
pub const SEGMENT: u32 = 100;

/// How far a weight moves to its segment's mean score.
const REACTION: f64 = 0.1;

/// Scores: a new best, a better current, a worse one accepted.
const SCORE_BEST: f64 = 33.0;
const SCORE_BETTER: f64 = 9.0;
const SCORE_ACCEPTED: f64 = 13.0;

/// No operator is ever starved completely.
const MIN_WEIGHT: f64 = 0.02;
const START_WEIGHT: f64 = 1.0;

/// What one move came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Best,
    Better,
    Accepted,
    Rejected,
}

/// One operator's weight and what it did in this run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Operator {
    pub name: String,
    pub weight: f64,
    #[serde(skip)]
    pub tally: Tally,
    #[serde(skip)]
    score: f64,
    #[serde(skip)]
    uses: u32,
}

/// Moves an operator took part in, by verdict.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tally {
    pub tries: u64,
    pub accepted: u64,
    pub better: u64,
    pub best: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Wheel {
    pub operators: Vec<Operator>,
    #[serde(skip)]
    moves: u32,
}

impl Wheel {
    /// Every operator at the same start weight.
    pub fn new<S: Into<String>>(names: impl IntoIterator<Item = S>) -> Self {
        let operators = names
            .into_iter()
            .map(|name| Operator {
                name: name.into(),
                weight: START_WEIGHT,
                tally: Tally::default(),
                score: 0.0,
                uses: 0,
            })
            .collect();

        Self { operators, moves: 0 }
    }

    /// The weights of `saved` where the names match, the rest as they are.
    pub fn with_weights(mut self, saved: &Wheel) -> Self {
        for op in &mut self.operators {
            if let Some(old) = saved.operators.iter().find(|o| o.name == op.name) {
                op.weight = old.weight.max(MIN_WEIGHT);
            }
        }

        self
    }

    /// The mean weight per name over `wheels`, for an instance seen first.
    pub fn pooled(names: &[String], wheels: &[Wheel]) -> Self {
        let mut pooled = Self::new(names.iter().cloned());

        for op in &mut pooled.operators {
            let weights: Vec<f64> = wheels
                .iter()
                .filter_map(|w| w.operators.iter().find(|o| o.name == op.name))
                .map(|o| o.weight)
                .collect();
            if !weights.is_empty() {
                op.weight = weights.iter().sum::<f64>() / weights.len() as f64;
            }
        }

        pooled
    }

    /// Draws an operator among those `allowed`, by weight. `u` is uniform in `[0, 1)`.
    pub fn draw(&self, u: f64, allowed: impl Fn(usize) -> bool) -> usize {
        let total: f64 = (0..self.operators.len())
            .filter(|&i| allowed(i))
            .map(|i| self.operators[i].weight)
            .sum();

        let mut left = u * total;
        let mut last = 0;
        for (i, op) in self.operators.iter().enumerate().filter(|&(i, _)| allowed(i)) {
            last = i;
            left -= op.weight;
            if left < 0.0 {
                return i;
            }
        }

        last
    }

    /// Books one move of operator `i`.
    pub fn record(&mut self, i: usize, verdict: Verdict) {
        let op = &mut self.operators[i];
        op.tally.tries += 1;
        op.uses += 1;

        let score = match verdict {
            Verdict::Best => SCORE_BEST,
            Verdict::Better => SCORE_BETTER,
            Verdict::Accepted => SCORE_ACCEPTED,
            Verdict::Rejected => 0.0,
        };
        op.score += score;
        op.tally.accepted += u64::from(verdict != Verdict::Rejected);
        op.tally.better += u64::from(matches!(verdict, Verdict::Better | Verdict::Best));
        op.tally.best += u64::from(verdict == Verdict::Best);
    }

    /// Counts one move; every [`SEGMENT`] moves the weights adapt.
    pub fn tick(&mut self) {
        self.moves += 1;
        if self.moves % SEGMENT != 0 {
            return;
        }

        for op in &mut self.operators {
            if op.uses > 0 {
                let mean = op.score / f64::from(op.uses);
                op.weight = ((1.0 - REACTION) * op.weight + REACTION * mean).max(MIN_WEIGHT);
            }
            op.score = 0.0;
            op.uses = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_follow_the_weights_and_the_mask() {
        let mut wheel = Wheel::new(["a", "b", "c"]);
        wheel.operators[0].weight = 10.0;
        wheel.operators[1].weight = 30.0;
        wheel.operators[2].weight = 10.0;

        assert_eq!(wheel.draw(0.05, |_| true), 0);
        assert_eq!(wheel.draw(0.5, |_| true), 1);
        assert_eq!(wheel.draw(0.99, |_| true), 2);
        assert_eq!(wheel.draw(0.5, |i| i != 1), 2);
    }

    #[test]
    fn winners_gain_weight_after_a_segment() {
        let mut wheel = Wheel::new(["win", "lose"]);

        for _ in 0..SEGMENT / 2 {
            wheel.record(0, Verdict::Best);
            wheel.tick();
            wheel.record(1, Verdict::Rejected);
            wheel.tick();
        }

        assert!(wheel.operators[0].weight > START_WEIGHT);
        assert!(wheel.operators[1].weight < START_WEIGHT);
        assert_eq!(wheel.operators[0].tally.best, u64::from(SEGMENT / 2));
    }

    #[test]
    fn pooling_averages_by_name_and_saved_weights_carry_over() {
        let names = vec!["a".to_owned(), "b".to_owned()];
        let mut one = Wheel::new(["a"]);
        one.operators[0].weight = 4.0;
        let mut two = Wheel::new(["a", "b"]);
        two.operators[0].weight = 8.0;

        let pooled = Wheel::pooled(&names, &[one, two.clone()]);
        assert_eq!(pooled.operators[0].weight, 6.0);
        assert_eq!(pooled.operators[1].weight, START_WEIGHT);

        let carried = Wheel::new(["b", "a"]).with_weights(&two);
        assert_eq!(carried.operators[1].weight, 8.0);
    }
}
