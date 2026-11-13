//! Progress lines on stderr: one per finished job, with the totals so far.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use mow_store::JobRecord;

use super::MineSummary;

/// How a finished job went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Best,
    Done,
    Failed,
}

pub struct Progress {
    total: usize,
    done: AtomicUsize,
    bests: AtomicUsize,
    failed: AtomicUsize,
    started: Instant,
}

impl Progress {
    pub fn new(total: usize) -> Self {
        Self {
            total,
            done: AtomicUsize::new(0),
            bests: AtomicUsize::new(0),
            failed: AtomicUsize::new(0),
            started: Instant::now(),
        }
    }

    /// Counts the job and prints its line.
    pub fn finished(&self, record: &JobRecord, outcome: Outcome) {
        let done = self.done.fetch_add(1, Ordering::Relaxed) + 1;
        match outcome {
            Outcome::Best => self.bests.fetch_add(1, Ordering::Relaxed),
            Outcome::Failed => self.failed.fetch_add(1, Ordering::Relaxed),
            Outcome::Done => 0,
        };

        let minutes = self.started.elapsed().as_secs_f64() / 60.0;
        let rate = done as f64 / minutes.max(1e-9);
        let totals = format!(
            "{done}/{} | {rate:.1} jobs/min, {} bests, {} failed",
            self.total,
            self.bests.load(Ordering::Relaxed),
            self.failed.load(Ordering::Relaxed)
        );

        match (&record.error, record.objective) {
            (Some(error), _) => eprintln!(
                "FAIL {:<34} {:<44} {} | {totals}",
                record.instance,
                record.label,
                clip(error, 120)
            ),
            (None, objective) => eprintln!(
                "{} {:<34} {:<44} {:>8} {:>7.1}s | {totals}",
                if outcome == Outcome::Best { "BEST" } else { "    " },
                record.instance,
                record.label,
                objective.unwrap_or(0),
                record.seconds
            ),
        }
    }

    pub fn summary(&self, total: usize, skipped: usize, unstarted: usize, seconds: f64) -> MineSummary {
        MineSummary {
            total,
            skipped,
            done: self.done.load(Ordering::Relaxed),
            failed: self.failed.load(Ordering::Relaxed),
            bests: self.bests.load(Ordering::Relaxed),
            unstarted,
            seconds,
        }
    }
}

fn clip(text: &str, chars: usize) -> String {
    text.chars().take(chars).collect()
}
