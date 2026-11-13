//! What one run produced: a solution, the errors found, the time it took.

use mow_core::Solution;

/// The result of one run. A run never panics or returns an error to its
/// caller: whatever went wrong is a string in `errors`, so one bad config
/// never stops a batch.
#[derive(Debug, Clone, PartialEq)]
pub struct Attempt {
    /// The config label, as the job log writes it.
    pub label: String,
    /// The routes built, if the run got that far, even when they fail the
    /// coverage check.
    pub solution: Option<Solution>,
    pub errors: Vec<String>,
    pub seconds: f64,
}

impl Attempt {
    /// A solution without errors. The official verifier still has the
    /// last word; the coverage check here is only a fast pre-check.
    pub fn feasible(&self) -> bool {
        self.solution.is_some() && self.errors.is_empty()
    }

    /// The longest route, for feasible attempts only.
    pub fn objective(&self) -> Option<u64> {
        self.solution
            .as_ref()
            .filter(|_| self.errors.is_empty())
            .map(Solution::objective)
    }
}

#[cfg(test)]
mod tests {
    use mow_core::{Point, Route};

    use super::*;

    fn attempt(errors: Vec<String>) -> Attempt {
        let route = Route::new(vec![Point::new(0, 0), Point::new(3, 0)]).unwrap();

        Attempt {
            label: "segments-h".into(),
            solution: Some(Solution::new("x", vec![route])),
            errors,
            seconds: 0.0,
        }
    }

    #[test]
    fn errors_hide_the_objective() {
        assert_eq!(attempt(Vec::new()).objective(), Some(6));
        assert!(attempt(Vec::new()).feasible());

        assert_eq!(attempt(vec!["uncovered".into()]).objective(), None);
        assert!(!attempt(vec!["uncovered".into()]).feasible());
    }
}
