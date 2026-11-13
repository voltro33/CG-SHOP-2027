//! The comparison as markdown (summary on top, then one row per instance)
//! and as CSV.

use std::fmt::Write as _;

use super::rows::{Check, Row};
use super::summary::{groups, Tally};

/// CPU seconds each side spent mining.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Cpu {
    pub rust: f64,
    pub python_all: f64,
    pub python_mined: f64,
}

const HEADER: [&str; 12] = [
    "instance",
    "k",
    "cells",
    "lower_bound",
    "python_best",
    "python_mined",
    "rust",
    "rust_config",
    "rust_vs_python_best",
    "rust_vs_python_mined",
    "bench",
    "verified",
];

fn number(value: Option<u64>) -> String {
    value.map_or_else(String::new, |v| v.to_string())
}

fn ratio(value: Option<f64>) -> String {
    value.map_or_else(String::new, |v| format!("{v:.4}"))
}

fn check(row: &Row) -> String {
    match &row.check {
        Check::NotRun => String::new(),
        Check::Passed => "yes".to_owned(),
        Check::Failed(error) => format!("FAILED: {error}"),
    }
}

fn cells(row: &Row) -> [String; 12] {
    [
        row.uid.clone(),
        row.k.to_string(),
        row.cells.to_string(),
        row.lower_bound.to_string(),
        number(row.python_best),
        number(row.python_mined),
        number(row.rust),
        row.rust_config.clone().unwrap_or_default(),
        ratio(row.ratio_best()),
        ratio(row.ratio_mined()),
        if row.bench { "yes" } else { "" }.to_owned(),
        check(row),
    ]
}

pub fn csv(rows: &[Row]) -> String {
    let mut text = HEADER.join(",") + "\n";

    for row in rows {
        let quoted: Vec<String> = cells(row).iter().map(|c| quote(c)).collect();
        text += &(quoted.join(",") + "\n");
    }

    text
}

fn quote(cell: &str) -> String {
    match cell.contains([',', '"', '\n']) {
        true => format!("\"{}\"", cell.replace('"', "\"\"")),
        false => cell.to_owned(),
    }
}

fn tally(t: &Tally) -> String {
    let geomean = t.geomean.map_or("n/a".to_owned(), |g| format!("{g:.4}"));

    format!("{} / {} / {} | {geomean}", t.wins, t.ties, t.losses)
}

pub fn markdown(rows: &[Row], cpu: Cpu, failures: usize) -> String {
    let mut text = String::from("# Rust results against Python\n\n");
    text += "Lower objective is better. A ratio below 1 means Rust is shorter. \
             *Python best* is the stored best (CP-SAT polish included); *Python mined* \
             is the best mined result without polish (job log rounds 0 and up).\n\n";

    if failures > 0 {
        let _ = writeln!(text, "**{failures} RUST BESTS FAILED THE OFFICIAL VERIFIER.**\n");
    }

    text += "| group | instances | Rust solved | vs Python best: W / T / L \\| geomean | vs Python mined: W / T / L \\| geomean |\n";
    text += "|---|---|---|---|---|\n";
    for group in groups(rows) {
        let _ = writeln!(
            text,
            "| {} | {} | {} | {} | {} |",
            group.name,
            group.instances,
            group.solved,
            tally(&group.vs_best).replace('|', "\\|"),
            tally(&group.vs_mined).replace('|', "\\|")
        );
    }

    let _ = writeln!(
        text,
        "\nCPU time: Rust {:.1} h (its job log), Python {:.1} h in all, {:.1} h without polish.\n",
        cpu.rust / 3600.0,
        cpu.python_all / 3600.0,
        cpu.python_mined / 3600.0
    );

    text += &format!("| {} |\n", HEADER.join(" | "));
    text += &format!("|{}\n", "---|".repeat(HEADER.len()));
    for row in rows {
        let cells: Vec<String> = cells(row).iter().map(|c| c.replace('|', "\\|")).collect();
        let _ = writeln!(text, "| {} |", cells.join(" | "));
    }

    text
}
