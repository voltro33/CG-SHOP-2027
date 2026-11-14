//! The report as `mine_report.py` prints it, and as its `--json` file.

use std::fmt::Write as _;

use super::{CapShare, ConfigStats, MineReport, RoundStats, Stratum};
use crate::pyfmt::{percent, Json};

/// All four tables, as printed.
pub fn text(report: &MineReport) -> String {
    let mut out = String::new();

    rounds(&mut out, &report.rounds);
    configs(&mut out, &report.configs);
    strata(&mut out, &report.strata);
    caps(&mut out, &report.caps);

    out
}

fn rounds(out: &mut String, rounds: &[RoundStats]) {
    out.push_str("rounds, in the order jobs finished\n");
    let _ = writeln!(
        out,
        "  {:>5} {:>6} {:>7} {:>7} {:>7} {:>10}",
        "round", "jobs", "cpu h", "better", "points", "pts/cpu h"
    );

    for s in rounds {
        let _ = writeln!(
            out,
            "  {:>5} {:>6} {:>7.1} {:>7} {:>7.2} {:>10.3}",
            s.round,
            s.jobs,
            s.cpu_hours,
            s.improvements,
            s.points,
            s.points_per_cpu_hour()
        );
    }
}

fn configs(out: &mut String, configs: &[ConfigStats]) {
    let never: Vec<&ConfigStats> = configs.iter().filter(|s| s.wins == 0).collect();
    let never_hours = never.iter().fold(0.0, |sum, s| sum + s.cpu_hours);
    let total_hours = configs.iter().fold(0.0, |sum, s| sum + s.cpu_hours);

    let _ = writeln!(
        out,
        "\nconfigs: {}, of which {} never won, using {never_hours:.1} of {total_hours:.1} cpu hours",
        configs.len(),
        never.len()
    );
    let _ = writeln!(
        out,
        "  {:58} {:>5} {:>6} {:>5} {:>5} {:>9}",
        "config", "jobs", "cpu h", "wins", "sole", "at stake"
    );

    for s in configs {
        let _ = writeln!(
            out,
            "  {:58} {:>5} {:>6.1} {:>5} {:>5} {:>9.3}",
            s.config, s.jobs, s.cpu_hours, s.wins, s.sole_wins, s.points_at_stake
        );
    }
}

fn strata(out: &mut String, strata: &[Stratum]) {
    out.push_str("\nmost wins per family and size\n");

    for s in strata {
        let listed: Vec<String> = s.ranked.iter().map(|(c, n)| format!("{c} {n}")).collect();
        let _ = writeln!(out, "  {:15} {:9} {}", s.family, s.band, listed.join(", "));
    }
}

fn caps(out: &mut String, caps: &[CapShare]) {
    out.push_str("\ntime caps per size band\n");

    for c in caps {
        let _ = writeln!(
            out,
            "  {:9} {:>6} jobs, {} capped, median run uses {} of its cap",
            c.band,
            c.jobs,
            percent(c.capped_share, 6, 1),
            percent(c.median_used_share, 6, 1)
        );
    }
}

/// What `mine_report.py --json` writes.
pub fn to_json(report: &MineReport) -> Json {
    let rounds = report.rounds.iter().map(|s| {
        Json::object(vec![
            ("round", Json::int(s.round)),
            ("jobs", Json::int(s.jobs)),
            ("cpu_hours", Json::Float(s.cpu_hours)),
            ("improvements", Json::int(s.improvements)),
            ("points", Json::Float(s.points)),
        ])
    });

    let configs = report.configs.iter().map(|s| {
        Json::object(vec![
            ("config", Json::str(&s.config)),
            ("jobs", Json::int(s.jobs)),
            ("cpu_hours", Json::Float(s.cpu_hours)),
            ("wins", Json::int(s.wins)),
            ("sole_wins", Json::int(s.sole_wins)),
            ("points_at_stake", Json::Float(s.points_at_stake)),
        ])
    });

    let strata = report.strata.iter().map(|s| {
        let ranked = s
            .ranked
            .iter()
            .map(|(config, count)| Json::List(vec![Json::str(config), Json::int(*count)]));
        (format!("{}|{}", s.family, s.band), Json::List(ranked.collect()))
    });

    let caps = report.caps.iter().map(|c| {
        let values = Json::object(vec![
            ("jobs", Json::int(c.jobs)),
            ("capped_share", Json::Float(c.capped_share)),
            ("median_used_share", Json::Float(c.median_used_share)),
        ]);
        (c.band.to_owned(), values)
    });

    Json::object(vec![
        ("rounds", Json::List(rounds.collect())),
        ("configs", Json::List(configs.collect())),
        ("strata", Json::Object(strata.collect())),
        ("time_caps", Json::Object(caps.collect())),
    ])
}
