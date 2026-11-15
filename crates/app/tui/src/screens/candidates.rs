//! The candidate tours kept for one instance, shortest first. Enter draws
//! one next to the best in the browser.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};
use ratatui::Frame;

use super::{moved, step_of, Nav, Task};
use crate::history::Candidate;

#[derive(Debug)]
pub struct Candidates {
    pub uid: String,
    pub best: Option<u64>,
    pub list: Vec<Candidate>,
    pub table: TableState,
}

impl Candidates {
    pub fn new(uid: String, best: Option<u64>, list: Vec<Candidate>) -> Self {
        Self {
            uid,
            best,
            list,
            table: TableState::default().with_selected(Some(0)),
        }
    }

    pub fn key(&mut self, key: KeyEvent) -> Nav {
        if let Some(step) = step_of(key, 20) {
            let at = moved(self.table.selected().unwrap_or(0), step, self.list.len());
            self.table.select(Some(at));
            return Nav::Stay;
        }

        match key.code {
            KeyCode::Enter => match self.table.selected().and_then(|i| self.list.get(i)) {
                Some(candidate) => Nav::Run(Task::Compare {
                    uid: self.uid.clone(),
                    candidate: candidate.clone(),
                }),
                None => Nav::Stay,
            },
            KeyCode::Esc | KeyCode::Backspace | KeyCode::Char('q') => Nav::Pop,
            _ => Nav::Stay,
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        if self.list.is_empty() {
            frame.render_widget(Paragraph::new("No candidates kept for this instance."), area);
            return;
        }

        let best = self.best;
        let rows: Vec<Row> = self
            .list
            .iter()
            .map(|c| {
                let over = best.map_or(String::new(), |b| {
                    format!("{:+.2}%", 100.0 * (c.objective as f64 / b as f64 - 1.0))
                });
                Row::new([
                    Cell::from(c.objective.to_string()),
                    Cell::from(over),
                    Cell::from(c.label.clone()),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [Constraint::Length(9), Constraint::Length(9), Constraint::Min(20)],
        )
        .header(Row::new(["longest", "vs best", "job"]).style(Style::new().add_modifier(Modifier::BOLD)))
        .block(Block::new().borders(Borders::NONE))
        .row_highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol("▶ ");
        frame.render_stateful_widget(table, area, &mut self.table);
    }
}
