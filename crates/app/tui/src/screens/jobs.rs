//! Every logged job of one instance, latest first.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Cell, Paragraph, Row, Table, TableState};
use ratatui::Frame;

use mow_store::JobRecord;

use super::{moved, step_of, Nav};

#[derive(Debug)]
pub struct Jobs {
    pub uid: String,
    /// Latest first.
    pub records: Vec<JobRecord>,
    pub table: TableState,
}

impl Jobs {
    pub fn new(uid: String, mut records: Vec<JobRecord>) -> Self {
        records.reverse();

        Self {
            uid,
            records,
            table: TableState::default().with_selected(Some(0)),
        }
    }

    pub fn key(&mut self, key: KeyEvent) -> Nav {
        if let Some(step) = step_of(key, 20) {
            let at = moved(self.table.selected().unwrap_or(0), step, self.records.len());
            self.table.select(Some(at));
            return Nav::Stay;
        }

        match key.code {
            KeyCode::Esc | KeyCode::Backspace | KeyCode::Char('q') => Nav::Pop,
            _ => Nav::Stay,
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        if self.records.is_empty() {
            frame.render_widget(Paragraph::new("No jobs logged for this instance."), area);
            return;
        }

        let rows: Vec<Row> = self.records.iter().map(row).collect();
        let table = Table::new(
            rows,
            [
                Constraint::Length(7),
                Constraint::Length(9),
                Constraint::Length(8),
                Constraint::Min(20),
            ],
        )
        .header(
            Row::new(["round", "longest", "seconds", "job"]).style(Style::new().add_modifier(Modifier::BOLD)),
        )
        .row_highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol("▶ ");
        frame.render_stateful_widget(table, area, &mut self.table);
    }
}

fn row(r: &JobRecord) -> Row<'static> {
    let (objective, style) = match (r.feasible_objective(), &r.error) {
        (Some(o), _) => (o.to_string(), Style::new()),
        (None, Some(e)) => (format!("failed: {e}"), Style::new().fg(Color::Red)),
        (None, None) => ("–".to_owned(), Style::new()),
    };

    Row::new([
        Cell::from(r.round.to_string()),
        Cell::from(objective).style(style),
        Cell::from(format!("{:.0}", r.seconds)),
        Cell::from(r.label.clone()),
    ])
}
