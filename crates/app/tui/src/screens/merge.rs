//! Bests: folders whose tours could replace ours, what a merge would take
//! from one, and the merges so far, each undoable.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};
use ratatui::Frame;

use super::confirm::Confirm;
use super::{moved, step_of, Nav, Screen, Task};
use crate::bests::{Backup, Plan, Source};
use crate::clock::ago;

/// Which list has the keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Sources,
    Backups,
}

#[derive(Debug)]
pub struct Bests {
    pub sources: Vec<Source>,
    pub backups: Vec<Backup>,
    focus: Focus,
    source: usize,
    backup: usize,
}

impl Bests {
    pub fn new(sources: Vec<Source>, backups: Vec<Backup>) -> Self {
        Self {
            sources,
            backups,
            focus: Focus::Sources,
            source: 0,
            backup: 0,
        }
    }

    /// New lists after a merge or an undo, keeping the places.
    pub fn replace(&mut self, sources: Vec<Source>, backups: Vec<Backup>) {
        self.source = self.source.min(sources.len().saturating_sub(1));
        self.backup = self.backup.min(backups.len().saturating_sub(1));
        self.sources = sources;
        self.backups = backups;
    }

    pub fn key(&mut self, key: KeyEvent) -> Nav {
        if let Some(step) = step_of(key, 10) {
            match self.focus {
                Focus::Sources => self.source = moved(self.source, step, self.sources.len()),
                Focus::Backups => self.backup = moved(self.backup, step, self.backups.len()),
            }
            return Nav::Stay;
        }

        match key.code {
            KeyCode::Tab | KeyCode::BackTab => {
                self.focus = match self.focus {
                    Focus::Sources => Focus::Backups,
                    Focus::Backups => Focus::Sources,
                };
                Nav::Stay
            }
            KeyCode::Enter => self.enter(),
            KeyCode::Esc | KeyCode::Backspace | KeyCode::Char('q') => Nav::Pop,
            _ => Nav::Stay,
        }
    }

    fn enter(&self) -> Nav {
        match self.focus {
            Focus::Sources => match self.sources.get(self.source) {
                Some(source) => Nav::Run(Task::PlanMerge(source.clone())),
                None => Nav::Stay,
            },
            Focus::Backups => match self.backups.get(self.backup) {
                Some(b) if b.undone => Nav::Say("this merge is undone already".to_owned()),
                Some(b) => Nav::Push(Screen::Confirm(Confirm::new(
                    vec![
                        format!(
                            "Undo the merge from {} ({}, {} tours)?",
                            b.manifest.source,
                            ago(b.manifest.made),
                            b.manifest.changes.len()
                        ),
                        "Each instance gets back its tour from before, unless a newer tour replaced the merged one."
                            .to_owned(),
                    ],
                    Task::Undo(Box::new(b.clone())),
                ))),
                None => Nav::Stay,
            },
        }
    }

    pub fn hints(&self) -> &'static str {
        match self.focus {
            Focus::Sources => {
                "↑↓ choose · Enter see what merging it would take · Tab merges so far · Esc back"
            }
            Focus::Backups => "↑↓ choose · Enter undo that merge · Tab merge sources · Esc back",
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let [top, bottom] =
            Layout::vertical([Constraint::Percentage(55), Constraint::Percentage(45)]).areas(area);

        let rows: Vec<Row> = self.sources.iter().map(|s| Row::new([s.name.clone()])).collect();
        let sources = Table::new(rows, [Constraint::Min(20)])
            .block(block("Merge from", self.focus == Focus::Sources))
            .row_highlight_style(highlight(self.focus == Focus::Sources))
            .highlight_symbol("▶ ");
        let mut state = TableState::default().with_selected(Some(self.source));
        frame.render_stateful_widget(sources, top, &mut state);

        let rows: Vec<Row> = self.backups.iter().map(backup_row).collect();
        let backups = Table::new(
            rows,
            [
                Constraint::Length(13),
                Constraint::Length(8),
                Constraint::Length(10),
                Constraint::Length(8),
                Constraint::Min(20),
            ],
        )
        .header(
            Row::new(["made", "tours", "points", "", "from"])
                .style(Style::new().add_modifier(Modifier::BOLD)),
        )
        .block(block("Merges so far (undo)", self.focus == Focus::Backups))
        .row_highlight_style(highlight(self.focus == Focus::Backups))
        .highlight_symbol("▶ ");
        let mut state = TableState::default().with_selected(Some(self.backup));
        frame.render_stateful_widget(backups, bottom, &mut state);
    }
}

fn backup_row(b: &Backup) -> Row<'static> {
    Row::new([
        Cell::from(ago(b.manifest.made)),
        Cell::from(b.manifest.changes.len().to_string()),
        Cell::from(format!("{:+.3}", b.manifest.gain)),
        Cell::from(if b.undone { "undone" } else { "" }).style(Style::new().fg(Color::Yellow)),
        Cell::from(b.manifest.source.clone()),
    ])
}

fn block(title: &'static str, focused: bool) -> Block<'static> {
    let style = match focused {
        true => Style::new().fg(Color::Green).add_modifier(Modifier::BOLD),
        false => Style::new().fg(Color::Gray),
    };
    Block::new().title(title).title_style(style).borders(Borders::TOP)
}

fn highlight(focused: bool) -> Style {
    match focused {
        true => Style::new().add_modifier(Modifier::REVERSED),
        false => Style::new().add_modifier(Modifier::UNDERLINED),
    }
}

/// What merging one source would take.
#[derive(Debug)]
pub struct MergePlan {
    pub plan: Plan,
    table: TableState,
}

impl MergePlan {
    pub fn new(plan: Plan) -> Self {
        Self {
            plan,
            table: TableState::default().with_selected(Some(0)),
        }
    }

    pub fn key(&mut self, key: KeyEvent) -> Nav {
        if let Some(step) = step_of(key, 20) {
            let at = moved(self.table.selected().unwrap_or(0), step, self.plan.changes.len());
            self.table.select(Some(at));
            return Nav::Stay;
        }

        match key.code {
            KeyCode::Char('m') | KeyCode::Enter if !self.plan.changes.is_empty() => {
                Nav::Push(Screen::Confirm(Confirm::new(
                    vec![
                        format!(
                            "Merge {} shorter tours from {} into data/rust/best, {:+.3} points?",
                            self.plan.changes.len(),
                            self.plan.source.name,
                            self.plan.gain
                        ),
                        "The tours replaced are backed up first; Bests › Merges so far undoes it.".to_owned(),
                    ],
                    Task::Merge(Box::new(self.plan.clone())),
                )))
            }
            KeyCode::Esc | KeyCode::Backspace | KeyCode::Char('q') => Nav::Pop,
            _ => Nav::Stay,
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let [head, body] = Layout::vertical([Constraint::Length(2), Constraint::Min(0)]).areas(area);

        let summary = match self.plan.changes.len() {
            0 => format!("{}: nothing shorter than the stored bests", self.plan.source.name),
            n => format!(
                "{}: {n} shorter tours, {:+.3} points · Enter or m merges",
                self.plan.source.name, self.plan.gain
            ),
        };
        frame.render_widget(Paragraph::new(summary), head);

        let rows: Vec<Row> = self
            .plan
            .changes
            .iter()
            .map(|c| {
                let before = c.before.map_or("none".to_owned(), |b| b.to_string());
                let change = c.before.map_or(String::new(), |b| {
                    format!("{:+.2}%", 100.0 * (c.after as f64 / b as f64 - 1.0))
                });
                Row::new([c.uid.clone(), before, c.after.to_string(), change])
            })
            .collect();
        let table = Table::new(
            rows,
            [
                Constraint::Length(34),
                Constraint::Length(9),
                Constraint::Length(9),
                Constraint::Length(9),
            ],
        )
        .header(
            Row::new(["instance", "ours", "theirs", "change"])
                .style(Style::new().add_modifier(Modifier::BOLD)),
        )
        .row_highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol("▶ ");
        frame.render_stateful_widget(table, body, &mut self.table);
    }
}
