//! How far a run is, read from the lines `mow` prints: one per finished job
//! ending `| 12/250 | 2.9 jobs/min, 7 bests, 0 failed`, and before the first
//! of those, `250 instances, 250 jobs, 0 already done, 250 to run`.

use std::path::PathBuf;

/// Jobs done of the total, summed over a run's logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    pub bests: usize,
    pub failed: usize,
}

impl Progress {
    /// The sum over `logs`; `None` when none of them reports jobs yet.
    pub fn of_logs(logs: &[PathBuf]) -> Option<Self> {
        let found: Vec<Self> = logs
            .iter()
            .filter_map(|path| std::fs::read_to_string(path).ok())
            .filter_map(|text| Self::of_text(&text))
            .collect();

        (!found.is_empty()).then(|| {
            found.iter().fold(Self::default(), |a, p| Self {
                done: a.done + p.done,
                total: a.total + p.total,
                bests: a.bests + p.bests,
                failed: a.failed + p.failed,
            })
        })
    }

    /// From one log's text: its last job line, else its plan line.
    pub fn of_text(text: &str) -> Option<Self> {
        text.lines()
            .rev()
            .find_map(job_line)
            .or_else(|| text.lines().find_map(plan_line))
    }

    /// Done over total, 0 to 1.
    pub fn fraction(&self) -> f64 {
        match self.total {
            0 => 0.0,
            total => (self.done as f64 / total as f64).min(1.0),
        }
    }
}

/// `... | 12/250 | 2.9 jobs/min, 7 bests, 0 failed`
fn job_line(line: &str) -> Option<Progress> {
    let mut parts = line.rsplit(" | ");
    let totals = parts.next()?;
    let (done, total) = parts.next()?.trim().split_once('/')?;

    let mut words = totals.split(", ");
    words.next().filter(|w| w.ends_with("jobs/min"))?;
    let bests = words.next()?.strip_suffix(" bests")?;
    let failed = words.next()?.strip_suffix(" failed")?;

    Some(Progress {
        done: done.parse().ok()?,
        total: total.parse().ok()?,
        bests: bests.parse().ok()?,
        failed: failed.parse().ok()?,
    })
}

/// `250 instances, 250 jobs, 0 already done, 250 to run on 12 threads ...`
fn plan_line(line: &str) -> Option<Progress> {
    let to_run = line.split(", ").find_map(|w| w.split_once(" to run"))?.0;

    Some(Progress {
        total: to_run.trim().parse().ok()?,
        ..Progress::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_job_line_counts() {
        let text = "\
250 instances, 250 jobs, 0 already done, 250 to run on 12 threads (kick scale 1, version r1)
BEST a_1x3          lns/x   3176   300.0s | 1/250 | 2.9 jobs/min, 1 bests, 0 failed
     b_1x3          lns/x   3180   300.0s | 2/250 | 3.1 jobs/min, 1 bests, 0 failed
lns b_1x3: 3180 -> 3180 in 120 moves, 0 bests
";
        let p = Progress::of_text(text).unwrap();

        assert_eq!((p.done, p.total, p.bests, p.failed), (2, 250, 1, 0));
        assert!((p.fraction() - 0.008).abs() < 1e-12);
    }

    #[test]
    fn before_the_first_job_the_plan_gives_the_total() {
        let p = Progress::of_text("31 instances, 31 jobs, 0 already done, 31 to run on 7 threads\n").unwrap();

        assert_eq!((p.done, p.total), (0, 31));
        assert_eq!(Progress::of_text("starting\n"), None);
    }

    #[test]
    fn failures_with_pipes_in_the_error_still_parse() {
        let line = "FAIL a_1x3   lns/x   broken | pipe | 3/9 | 1.0 jobs/min, 0 bests, 1 failed";

        assert_eq!(job_line(line).map(|p| (p.done, p.failed)), Some((3, 1)));
    }
}
