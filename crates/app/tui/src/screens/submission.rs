//! Submission: the zips packed so far, and packing a new one. A zip is only
//! written here; submitting it is up to its owner.

use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Row, Table};
use ratatui::Frame;

use super::confirm::Confirm;
use super::{draw_with_actions, moved, step_of, Ctx, Nav, Screen, Task};
use crate::clock::{ago, seconds_of};

/// A packed zip.
#[derive(Debug, Clone)]
pub struct Zip {
    pub path: PathBuf,
    pub bytes: u64,
    pub made: u64,
}

/// The zips in `data/rust`, newest first.
pub fn zips(data: &Path) -> Vec<Zip> {
    let Ok(entries) = std::fs::read_dir(data) else {
        return Vec::new();
    };

    let mut found: Vec<Zip> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "zip"))
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            Some(Zip {
                path: e.path(),
                bytes: meta.len(),
                made: seconds_of(meta.modified().ok()?),
            })
        })
        .collect();
    found.sort_by(|a, b| b.made.cmp(&a.made));

    found
}

#[derive(Debug)]
pub struct Submission {
    pub zips: Vec<Zip>,
    /// 0 is "Pack a new zip", then the zips.
    selected: usize,
}

impl Submission {
    pub fn new(ctx: &Ctx) -> Self {
        Self {
            zips: zips(&ctx.paths.data),
            selected: 0,
        }
    }

    pub fn key(&mut self, key: KeyEvent, ctx: &Ctx) -> Nav {
        if let Some(step) = step_of(key, 10) {
            self.selected = moved(self.selected, step, self.zips.len() + 1);
            return Nav::Stay;
        }

        match key.code {
            KeyCode::Enter if self.selected == 0 => Nav::Push(Screen::Confirm(Confirm::new(
                vec![
                    format!(
                        "Pack the {} stored bests (score {:.3}) into a new zip?",
                        ctx.summary.all.solved, ctx.summary.all.points
                    ),
                    "Each instance takes the shorter of the Rust and Python best that passes the official verifier."
                        .to_owned(),
                    "It runs in the background for a few minutes; follow it under Runs. Nothing is uploaded.".to_owned(),
                ],
                Task::Pack,
            ))),
            KeyCode::Enter => Nav::Run(Task::OpenFolder(ctx.paths.data.clone())),
            KeyCode::Esc | KeyCode::Backspace | KeyCode::Char('q') => Nav::Pop,
            _ => Nav::Stay,
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let [note, body] = Layout::vertical([Constraint::Length(2), Constraint::Min(0)]).areas(area);
        frame.render_widget(
            Paragraph::new(Line::styled(
                "Zips are written to data/rust/ and verified; uploading one is up to you.",
                Style::new().fg(Color::Gray),
            )),
            note,
        );

        let rows: Vec<Row> = self
            .zips
            .iter()
            .map(|z| {
                let name = z
                    .path
                    .file_name()
                    .map_or(String::new(), |n| n.to_string_lossy().into_owned());
                Row::new([name, format!("{:.1} MB", z.bytes as f64 / 1e6), ago(z.made)])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(40),
                Constraint::Length(10),
                Constraint::Min(10),
            ],
        )
        .header(Row::new(["zip", "size", "written"]).style(Style::new().add_modifier(Modifier::BOLD)));
        draw_with_actions(frame, body, &["Pack a new zip…"], table, self.selected);
    }
}
