//! The dashboard: the score, by k and by cutter family, with the median gap
//! to the lower bound as a bar per group.

use std::time::SystemTime;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};
use ratatui::Frame;

use super::{Ctx, Nav, Task};
use crate::summary::Group;

/// Median gaps drawn at full bar length.
const FULL_BAR_GAP: f64 = 80.0;
const BAR_WIDTH: usize = 24;

#[derive(Debug, Default)]
pub struct Dashboard;

impl Dashboard {
    pub fn key(&mut self, key: KeyEvent) -> Nav {
        match key.code {
            KeyCode::Char('r') => Nav::Run(Task::Reload),
            KeyCode::Esc | KeyCode::Backspace | KeyCode::Char('q') => Nav::Pop,
            _ => Nav::Stay,
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect, ctx: &Ctx) {
        let [head, by_k, by_family] = Layout::vertical([
            Constraint::Length(5),
            Constraint::Length(ctx.summary.by_k.len() as u16 + 4),
            Constraint::Min(0),
        ])
        .areas(area);

        frame.render_widget(headline(ctx), head);
        frame.render_widget(table("By number of cutters", &ctx.summary.by_k), by_k);
        frame.render_widget(table("By cutter family", &ctx.summary.by_family), by_family);
    }
}

fn headline(ctx: &Ctx) -> Paragraph<'static> {
    let all = &ctx.summary.all;
    let gap = all.median_gap.map_or("–".to_owned(), |g| format!("{g:.1}%"));
    let latest = match &ctx.lawn.latest {
        Some((uid, when)) => format!("{uid}, {}", ago(*when)),
        None => "none".to_owned(),
    };

    let lines = vec![
        Line::from(vec![
            Span::raw("Score "),
            Span::styled(
                format!("{:.3}", all.points),
                Style::new().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                " of {}   (sum of (bound / best)², the pessimistic score)",
                all.count
            )),
        ]),
        Line::from(format!(
            "{} of {} instances have a stored best · median gap {gap}",
            all.solved, all.count
        )),
        Line::from(format!("Latest new best: {latest}")),
    ];

    Paragraph::new(lines).block(Block::new().borders(Borders::BOTTOM))
}

fn table(title: &'static str, groups: &[Group]) -> Table<'static> {
    let header =
        Row::new(["group", "n", "points", "median gap", ""]).style(Style::new().add_modifier(Modifier::BOLD));
    let rows: Vec<Row> = groups.iter().map(row).collect();

    Table::new(
        rows,
        [
            Constraint::Length(16),
            Constraint::Length(5),
            Constraint::Length(9),
            Constraint::Length(11),
            Constraint::Length(BAR_WIDTH as u16),
        ],
    )
    .header(header)
    .block(Block::new().title(title).borders(Borders::TOP))
}

fn row(g: &Group) -> Row<'static> {
    let gap = g.median_gap.map_or("–".to_owned(), |v| format!("{v:.1}%"));
    let bar = g.median_gap.map_or(String::new(), bar);

    Row::new([
        Cell::from(g.name.clone()),
        Cell::from(g.count.to_string()),
        Cell::from(format!("{:.3}", g.points)),
        Cell::from(gap),
        Cell::from(bar).style(Style::new().fg(Color::Yellow)),
    ])
}

/// A bar as long as `gap` is against [`FULL_BAR_GAP`].
fn bar(gap: f64) -> String {
    let filled = ((gap / FULL_BAR_GAP).clamp(0.0, 1.0) * BAR_WIDTH as f64).round() as usize;
    "█".repeat(filled)
}

/// `"5 min ago"` and the like.
fn ago(when: SystemTime) -> String {
    let seconds = SystemTime::now().duration_since(when).map_or(0, |d| d.as_secs());

    match seconds {
        0..=59 => format!("{seconds} s ago"),
        60..=3599 => format!("{} min ago", seconds / 60),
        3600..=86_399 => format!("{:.1} h ago", seconds as f64 / 3600.0),
        _ => format!("{:.1} days ago", seconds as f64 / 86_400.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bars_grow_with_the_gap_and_stop_at_full() {
        assert_eq!(bar(0.0), "");
        assert_eq!(bar(40.0).chars().count(), BAR_WIDTH / 2);
        assert_eq!(bar(500.0).chars().count(), BAR_WIDTH);
    }
}
