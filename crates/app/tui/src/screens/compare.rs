//! Compare: choose A and B, each a folder of solutions or one solution
//! file, in the file browser, then start `mow compare A B` as a background
//! run. Every solution is checked before it is compared; the page opens in
//! the browser when the run is done.

use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::picker::{shown, solutions_in, Picker, Slot};
use super::{moved, step_of, Ctx, Nav, Screen, Task};

const ROWS: usize = 4;
const SWAP: usize = 2;
const START: usize = 3;

#[derive(Debug)]
pub struct Compare {
    pub a: Option<PathBuf>,
    pub b: Option<PathBuf>,
    selected: usize,
}

impl Compare {
    /// A starts on the stored bests; B is to be chosen.
    pub fn new(ctx: &Ctx) -> Self {
        let best = ctx.paths.best_dir();

        Self {
            a: best.is_dir().then_some(best),
            b: None,
            selected: 1,
        }
    }

    pub fn set(&mut self, slot: Slot, path: PathBuf) {
        match slot {
            Slot::A => self.a = Some(path),
            Slot::B => self.b = Some(path),
        }
        self.selected = match (&self.a, &self.b) {
            (Some(_), Some(_)) => START,
            (None, _) => 0,
            _ => 1,
        };
    }

    pub fn key(&mut self, key: KeyEvent, ctx: &Ctx) -> Nav {
        if let Some(step) = step_of(key, 1) {
            self.selected = moved(self.selected, step, ROWS);
            return Nav::Stay;
        }

        match key.code {
            KeyCode::Enter => match self.selected {
                0 => self.browse(Slot::A, ctx),
                1 => self.browse(Slot::B, ctx),
                SWAP => {
                    std::mem::swap(&mut self.a, &mut self.b);
                    Nav::Stay
                }
                _ => self.start(),
            },
            KeyCode::Char('a') => self.browse(Slot::A, ctx),
            KeyCode::Char('b') => self.browse(Slot::B, ctx),
            KeyCode::Char('s') => {
                std::mem::swap(&mut self.a, &mut self.b);
                Nav::Stay
            }
            KeyCode::Esc | KeyCode::Char('q') => Nav::Pop,
            _ => Nav::Stay,
        }
    }

    /// The file browser for one side, opened where that side points (or
    /// next to the other side, or at the data folder).
    fn browse(&self, slot: Slot, ctx: &Ctx) -> Nav {
        let (mine, other) = match slot {
            Slot::A => (&self.a, &self.b),
            Slot::B => (&self.b, &self.a),
        };
        let home = ctx.paths.root.join("data");
        let start = mine
            .as_deref()
            .or(other.as_deref())
            .map(|p| match p.is_dir() {
                true => p.parent().unwrap_or(p).to_path_buf(),
                false => p.parent().map_or_else(|| home.clone(), Path::to_path_buf),
            })
            .unwrap_or_else(|| home.clone());

        Nav::Push(Screen::Picker(Picker::open(slot, &start, &ctx.paths.root, &home)))
    }

    fn start(&self) -> Nav {
        let (Some(a), Some(b)) = (&self.a, &self.b) else {
            return Nav::Say("choose both A and B first".to_owned());
        };
        if a.is_dir() != b.is_dir() {
            return Nav::Say("A and B must both be folders or both be files".to_owned());
        }
        if a == b {
            return Nav::Say("A and B are the same; choose another for one of them".to_owned());
        }

        Nav::Run(Task::StartCompare {
            a: a.clone(),
            b: b.clone(),
        })
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect, ctx: &Ctx) {
        let [about, rows, notes] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(ROWS as u16 + 1),
            Constraint::Min(0),
        ])
        .areas(area);

        frame.render_widget(
            Paragraph::new(vec![
                Line::from("Two folders of solutions (matched by file name) or two solution files of one instance."),
                Line::styled(
                    "Each solution is checked by our coverage count and the official verifier before it is compared.",
                    Style::new().fg(Color::Gray),
                ),
            ]),
            about,
        );

        let side = |slot: Slot, path: &Option<PathBuf>| -> Line<'static> {
            let value = match path {
                Some(p) => Span::styled(describe(p, &ctx.paths.root), Style::new().fg(Color::Cyan)),
                None => Span::styled("choose…", Style::new().fg(Color::DarkGray)),
            };
            Line::from(vec![
                Span::styled(
                    format!("{}  ", slot.letter()),
                    Style::new().add_modifier(Modifier::BOLD),
                ),
                value,
            ])
        };
        let ready = self.a.is_some() && self.b.is_some();
        let lines = [
            side(Slot::A, &self.a),
            side(Slot::B, &self.b),
            Line::from("⇅  swap A and B"),
            Line::styled(
                "[ Compare ]",
                match ready {
                    true => Style::new().fg(Color::Green).add_modifier(Modifier::BOLD),
                    false => Style::new().fg(Color::DarkGray),
                },
            ),
        ];
        let lines: Vec<Line> = lines
            .into_iter()
            .enumerate()
            .map(|(i, line)| match i == self.selected {
                true => {
                    let mut line = line.patch_style(Style::new().add_modifier(Modifier::REVERSED));
                    line.spans.insert(0, Span::raw("▶ "));
                    line
                }
                false => {
                    let mut line = line;
                    line.spans.insert(0, Span::raw("  "));
                    line
                }
            })
            .collect();
        frame.render_widget(Paragraph::new(lines), rows);

        frame.render_widget(
            Paragraph::new(vec![
                Line::styled(
                    "It runs in the background (about a minute for 250 instances); follow it under Runs.",
                    Style::new().fg(Color::Gray),
                ),
                Line::styled(
                    "The page opens in the browser when it is done, and stays in data/rust/view/.",
                    Style::new().fg(Color::Gray),
                ),
            ]),
            notes,
        );
    }
}

/// A chosen path, relative to the repository, with what it holds.
fn describe(path: &Path, root: &Path) -> String {
    match path.is_dir() {
        true => format!("{}/  ({} solutions)", shown(path, root), solutions_in(path)),
        false => shown(path, root),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choosing_both_sides_moves_to_the_button() {
        let mut compare = Compare {
            a: None,
            b: None,
            selected: 0,
        };

        compare.set(Slot::B, PathBuf::from("b"));
        assert_eq!(compare.selected, 0, "A still to choose");

        compare.set(Slot::A, PathBuf::from("a"));
        assert_eq!(compare.selected, START);
        assert_eq!(compare.a.as_deref(), Some(Path::new("a")));
    }

    #[test]
    fn it_starts_only_with_two_different_paths() {
        let compare = Compare {
            a: Some(PathBuf::from("same")),
            b: Some(PathBuf::from("same")),
            selected: START,
        };
        assert!(matches!(compare.start(), Nav::Say(_)));

        let compare = Compare {
            a: None,
            b: Some(PathBuf::from("b")),
            selected: START,
        };
        assert!(matches!(compare.start(), Nav::Say(_)));
    }
}
