//! A constraint model as plain data, and the port that solves it.
//!
//! The model is the part of OR-Tools' `CpModelProto` the window polish uses:
//! integer variables with a range, linear constraints with a range, `bool_or`,
//! enforcement literals, one objective to minimize and a solution hint. A
//! [`CpSolver`] adapter turns it into whatever its solver reads; the core
//! only builds it and reads the answer.
//!
//! Variables are numbered in the order they are made, as in the proto, so a
//! model built here and one built by Python's `cp_model` can be compared
//! constraint by constraint.

use std::time::Duration;

use crate::error::PortError;

/// A variable: its index in [`CpModel::domains`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Var(pub u32);

/// A boolean variable or its negation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Lit {
    pub var: Var,
    pub negated: bool,
}

/// `!lit` is its negation, as `~lit` in `cp_model`.
impl std::ops::Not for Lit {
    type Output = Self;

    fn not(self) -> Self {
        Self {
            negated: !self.negated,
            ..self
        }
    }
}

impl From<Var> for Lit {
    fn from(var: Var) -> Self {
        Self { var, negated: false }
    }
}

/// `lo <= x <= hi`. Unbounded sides are `i64::MIN` and `i64::MAX`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Domain {
    pub lo: i64,
    pub hi: i64,
}

/// `sum(coefficient * var)`, in the order the terms were given.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Linear {
    pub terms: Vec<(Var, i64)>,
}

impl Linear {
    pub fn new(terms: Vec<(Var, i64)>) -> Self {
        Self { terms }
    }

    /// Every variable once, with coefficient 1.
    pub fn sum(vars: impl IntoIterator<Item = Var>) -> Self {
        Self::new(vars.into_iter().map(|var| (var, 1)).collect())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstraintKind {
    /// `domain.lo <= expr <= domain.hi`.
    Linear { expr: Linear, domain: Domain },
    /// At least one literal is true.
    BoolOr(Vec<Lit>),
}

/// A constraint that only has to hold while every `enforcement` literal is true.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constraint {
    pub enforcement: Vec<Lit>,
    pub kind: ConstraintKind,
}

/// The model. Build it with the methods below; the fields stay readable for
/// adapters and tests.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CpModel {
    pub domains: Vec<Domain>,
    pub constraints: Vec<Constraint>,
    /// Minimized; `None` asks only for a feasible solution.
    pub objective: Option<Linear>,
    pub hint: Vec<(Var, i64)>,
}

impl CpModel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_int(&mut self, lo: i64, hi: i64) -> Var {
        let var = Var(self.domains.len() as u32);
        self.domains.push(Domain { lo, hi });

        var
    }

    pub fn new_bool(&mut self) -> Var {
        self.new_int(0, 1)
    }

    /// A variable fixed to `value`, as `cp_model.new_constant` makes one.
    pub fn new_constant(&mut self, value: i64) -> Var {
        self.new_int(value, value)
    }

    /// Adds `lo <= expr <= hi` and returns it, so enforcement can be added.
    pub fn add_linear(&mut self, expr: Linear, lo: i64, hi: i64) -> &mut Constraint {
        self.push(ConstraintKind::Linear {
            expr,
            domain: Domain { lo, hi },
        })
    }

    pub fn add_bool_or(&mut self, literals: Vec<Lit>) -> &mut Constraint {
        self.push(ConstraintKind::BoolOr(literals))
    }

    pub fn minimize(&mut self, objective: Linear) {
        self.objective = Some(objective);
    }

    pub fn add_hint(&mut self, var: Var, value: i64) {
        self.hint.push((var, value));
    }

    fn push(&mut self, kind: ConstraintKind) -> &mut Constraint {
        self.constraints.push(Constraint {
            enforcement: Vec::new(),
            kind,
        });

        self.constraints.last_mut().expect("just pushed")
    }
}

impl Constraint {
    /// `only_enforce_if` of `cp_model`.
    pub fn only_enforce_if(&mut self, literal: impl Into<Lit>) -> &mut Self {
        self.enforcement.push(literal.into());
        self
    }
}

/// How long and with how many threads to search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpParams {
    pub time_limit: Duration,
    pub workers: u32,
}

/// The solver's verdict, named as `CpSolver.status_name` names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpStatus {
    Optimal,
    Feasible,
    Infeasible,
    ModelInvalid,
    Unknown,
}

impl CpStatus {
    pub fn name(self) -> &'static str {
        match self {
            Self::Optimal => "OPTIMAL",
            Self::Feasible => "FEASIBLE",
            Self::Infeasible => "INFEASIBLE",
            Self::ModelInvalid => "MODEL_INVALID",
            Self::Unknown => "UNKNOWN",
        }
    }

    /// `OPTIMAL` or `FEASIBLE`: `values` hold a solution.
    pub fn found(self) -> bool {
        matches!(self, Self::Optimal | Self::Feasible)
    }
}

/// What the solver found. `values` has one entry per variable when
/// [`CpStatus::found`], else it is empty.
#[derive(Debug, Clone, PartialEq)]
pub struct CpResponse {
    pub status: CpStatus,
    pub objective: f64,
    pub values: Vec<i64>,
}

impl CpResponse {
    pub fn value(&self, var: Var) -> Option<i64> {
        self.values.get(var.0 as usize).copied()
    }
}

/// Solves a [`CpModel`]. The one in use is OR-Tools CP-SAT.
pub trait CpSolver: Send + Sync {
    fn solve(&self, model: &CpModel, params: &CpParams) -> Result<CpResponse, PortError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variables_are_numbered_in_order() {
        let mut model = CpModel::new();
        let a = model.new_int(0, 2);
        let b = model.new_bool();
        let one = model.new_constant(1);

        assert_eq!((a, b, one), (Var(0), Var(1), Var(2)));
        assert_eq!(model.domains[2], Domain { lo: 1, hi: 1 });
    }

    #[test]
    fn enforcement_attaches_to_the_last_constraint() {
        let mut model = CpModel::new();
        let x = model.new_int(0, 2);
        let on = model.new_bool();

        model
            .add_linear(Linear::sum([x]), 1, i64::MAX)
            .only_enforce_if(on);
        model.add_bool_or(vec![!Lit::from(on)]);

        assert_eq!(model.constraints[0].enforcement, vec![Lit::from(on)]);
        assert!(model.constraints[1].enforcement.is_empty());
    }
}
