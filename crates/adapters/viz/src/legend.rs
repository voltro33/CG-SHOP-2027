//! The text above the plot: what is shown, how long the routes are, and
//! how far the longest is from the lower bound.

use mow_core::{lower_bound, Problem, Route};

use crate::palette;

/// One line of legend text, in one color. Not escaped yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub text: String,
    pub color: &'static str,
}

impl Line {
    fn plain(text: String) -> Self {
        Self {
            text,
            color: palette::TEXT,
        }
    }
}

/// What the legend reports, gathered by the scene.
pub struct Facts<'a> {
    pub problem: &'a Problem,
    pub title: Option<&'a str>,
    pub segments: Option<usize>,
    pub routes: &'a [Route],
    pub uncovered: usize,
}

pub fn lines(facts: &Facts<'_>) -> Vec<Line> {
    let problem = facts.problem;
    let bound = lower_bound(problem);

    let heading = match facts.title {
        Some(title) => format!("{}: {title}", problem.uid()),
        None => problem.uid().to_string(),
    };
    let size = format!(
        "k = {}, {} cells, lower bound {bound}",
        problem.k(),
        problem.num_cells()
    );
    let mut lines = vec![Line::plain(heading), Line::plain(size)];

    if let Some(count) = facts.segments {
        lines.push(Line::plain(format!("{count} segments")));
    }
    if !facts.routes.is_empty() {
        lines.push(Line::plain(score(facts.routes, bound)));
        lines.extend(facts.routes.iter().enumerate().map(route_line));
    }
    if facts.uncovered > 0 {
        lines.push(Line {
            text: format!("{} uncovered cells", facts.uncovered),
            color: palette::UNCOVERED,
        });
    }

    lines
}

fn score(routes: &[Route], bound: u64) -> String {
    let objective = routes.iter().map(Route::length).max().unwrap_or(0);

    if bound == 0 {
        return format!("objective {objective}");
    }
    let gap = 100.0 * (objective as f64 - bound as f64) / bound as f64;

    format!("objective {objective}, gap {gap:.2}%")
}

fn route_line((index, route): (usize, &Route)) -> Line {
    Line {
        text: format!(
            "cutter {}: length {}, {} corners",
            index + 1,
            route.length(),
            route.corners().len()
        ),
        color: palette::route_color(index),
    }
}
