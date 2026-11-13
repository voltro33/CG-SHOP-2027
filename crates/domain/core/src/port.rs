//! Ports: how the core reaches the outside world. Adapters (files, the
//! Python verifier, a database) implement these; the core never names them.

use crate::error::PortError;
use crate::problem::Problem;
use crate::solution::Solution;

/// Where instances come from.
pub trait InstanceSource: Send + Sync {
    fn uids(&self) -> Result<Vec<String>, PortError>;

    fn load(&self, uid: &str) -> Result<Problem, PortError>;
}

/// What a store did with an offered solution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Offer {
    /// Shorter than the stored best; it is the best now.
    Best,
    /// Kept as a candidate, close to the best.
    Candidate,
    /// Not kept.
    Rejected,
}

/// Where verified solutions are kept.
pub trait SolutionStore: Send + Sync {
    fn best(&self, uid: &str) -> Result<Option<Solution>, PortError>;

    /// Only verified solutions may be offered.
    fn offer(&self, solution: &Solution) -> Result<Offer, PortError>;
}

/// The verdict of a verifier: no errors means feasible.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Verdict {
    pub errors: Vec<String>,
}

impl Verdict {
    pub fn is_feasible(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Decides whether a solution is accepted. The official one is Python.
pub trait Verifier: Send + Sync {
    fn verify(&self, solution: &Solution) -> Result<Verdict, PortError>;
}
