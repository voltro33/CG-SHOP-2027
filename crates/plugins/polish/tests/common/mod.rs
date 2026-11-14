//! Shared by the golden tests: paths, fixture parsing, the canonical model
//! text and hash of `tools/golden/polish/canonical.py`, a model checker and
//! fake solvers.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use mow_core::{
    ConstraintKind, CpModel, CpParams, CpResponse, CpSolver, CpStatus, Linear, Lit, Point, PortError,
    Problem, Route,
};

pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

pub fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// The instance, or `None` when the instance files are not there.
pub fn problem(uid: &str) -> Option<Problem> {
    let path = root().join(format!(
        "data/instances/challenge_v0/instances/{uid}.instance.json"
    ));
    mow_io::load_problem(&path).ok()
}

pub fn numbers<T: std::str::FromStr>(text: &str) -> Vec<T>
where
    T::Err: std::fmt::Debug,
{
    text.split_whitespace().map(|n| n.parse().unwrap()).collect()
}

/// One line of corners per tour.
pub fn read_routes(path: &Path) -> Vec<Route> {
    let text = fs::read_to_string(path).unwrap();

    text.lines()
        .map(|line| {
            let n: Vec<i32> = numbers(line);
            let corners = n.chunks(2).map(|p| Point::new(p[0], p[1])).collect();
            Route::new(corners).unwrap()
        })
        .collect()
}

pub fn dense(routes: &[Route]) -> Vec<Vec<Point>> {
    routes.iter().map(|r| r.positions().collect()).collect()
}

pub fn fnv(text: &str) -> String {
    let mut value: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in text.as_bytes() {
        value = (value ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
    }

    format!("{value:016x}")
}

/// Points as `x y` lines.
pub fn cells_text(cells: &[Point]) -> String {
    cells.iter().map(|p| format!("{} {}\n", p.x, p.y)).collect()
}

fn literal(lit: Lit) -> i64 {
    let var = i64::from(lit.var.0);
    if lit.negated {
        -var - 1
    } else {
        var
    }
}

fn literals(lits: &[Lit]) -> String {
    let mut encoded: Vec<i64> = lits.iter().map(|&l| literal(l)).collect();
    encoded.sort_unstable();

    join(encoded.iter().map(i64::to_string))
}

fn terms(expr: &Linear) -> String {
    let mut merged = std::collections::BTreeMap::new();
    for &(var, coefficient) in &expr.terms {
        *merged.entry(var.0).or_insert(0_i64) += coefficient;
    }

    join(
        merged
            .iter()
            .filter(|(_, &c)| c != 0)
            .map(|(v, c)| format!("{v}:{c}")),
    )
}

fn join(parts: impl Iterator<Item = String>) -> String {
    parts.collect::<Vec<_>>().join(" ")
}

/// The canonical form of `canonical.py`.
pub fn canonical(model: &CpModel) -> String {
    let mut lines: Vec<String> = model
        .domains
        .iter()
        .map(|d| format!("v {} {}", d.lo, d.hi))
        .collect();

    for constraint in &model.constraints {
        let enforcement = literals(&constraint.enforcement);

        lines.push(match &constraint.kind {
            ConstraintKind::Linear { expr, domain } => {
                format!("L {enforcement} | {} | {} {}", terms(expr), domain.lo, domain.hi)
            }
            ConstraintKind::BoolOr(lits) => format!("B {enforcement} | {}", literals(lits)),
        });
    }

    let objective = model.objective.clone().unwrap_or_default();
    lines.push(format!("O {} | 0", terms(&objective)));
    lines.push(format!(
        "H {}",
        join(model.hint.iter().map(|(v, x)| format!("{}:{x}", v.0)))
    ));

    lines.join("\n") + "\n"
}

/// Every variable at its hint, or at its lowest value if not hinted.
pub fn hinted_values(model: &CpModel) -> Vec<i64> {
    let mut values: Vec<i64> = model.domains.iter().map(|d| d.lo).collect();
    for &(var, value) in &model.hint {
        values[var.0 as usize] = value;
    }

    values
}

fn evaluate(expr: &Linear, values: &[i64]) -> i64 {
    expr.terms.iter().map(|&(v, c)| c * values[v.0 as usize]).sum()
}

fn holds(lit: Lit, values: &[i64]) -> bool {
    (values[lit.var.0 as usize] != 0) != lit.negated
}

/// The first constraint `values` break, by index, or the first variable
/// out of its domain as `usize::MAX`.
pub fn first_broken(model: &CpModel, values: &[i64]) -> Option<usize> {
    let out_of_domain = model
        .domains
        .iter()
        .zip(values)
        .any(|(d, &v)| v < d.lo || v > d.hi);
    if out_of_domain {
        return Some(usize::MAX);
    }

    model.constraints.iter().position(|c| {
        let enforced = c.enforcement.iter().all(|&l| holds(l, values));
        let satisfied = match &c.kind {
            ConstraintKind::Linear { expr, domain } => {
                let value = evaluate(expr, values);
                domain.lo <= value && value <= domain.hi
            }
            ConstraintKind::BoolOr(lits) => lits.iter().any(|&l| holds(l, values)),
        };

        enforced && !satisfied
    })
}

/// A response that sets every variable to `values`.
pub fn response(model: &CpModel, values: Vec<i64>, status: CpStatus) -> CpResponse {
    let objective = model.objective.as_ref().map_or(0, |o| evaluate(o, &values));

    CpResponse {
        status,
        objective: objective as f64,
        values,
    }
}

/// Answers with the hint: the old walk, feasible and no shorter.
pub struct HintSolver;

impl CpSolver for HintSolver {
    fn solve(&self, model: &CpModel, _: &CpParams) -> Result<CpResponse, PortError> {
        Ok(response(model, hinted_values(model), CpStatus::Feasible))
    }
}

/// Answers with nothing found.
pub struct Nothing;

impl CpSolver for Nothing {
    fn solve(&self, _: &CpModel, _: &CpParams) -> Result<CpResponse, PortError> {
        Ok(CpResponse {
            status: CpStatus::Unknown,
            objective: 0.0,
            values: Vec::new(),
        })
    }
}
