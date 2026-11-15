//! A question before anything that changes the bests or stops work: Enter
//! or y does it, Esc or n goes back.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use super::{Nav, Task};

#[derive(Debug)]
pub struct Confirm {
    pub question: Vec<String>,
    pub task: Task,
}

impl Confirm {
    pub fn new(question: Vec<String>, task: Task) -> Self {
        Self { question, task }
    }

    pub fn key(&mut self, key: KeyEvent) -> Nav {
        match key.code {
            KeyCode::Enter | KeyCode::Char('y') => Nav::PopAndRun(self.task.clone()),
            KeyCode::Esc | KeyCode::Backspace | KeyCode::Char('n') | KeyCode::Char('q') => Nav::Pop,
            _ => Nav::Stay,
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let mut lines: Vec<Line> = self.question.iter().map(|q| Line::raw(q.clone())).collect();
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            "Enter or y: yes      Esc or n: no",
            Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));

        let block = Block::new().title("Are you sure?").borders(Borders::ALL);
        frame.render_widget(
            Paragraph::new(lines).block(block).wrap(Wrap { trim: false }),
            area,
        );
    }
}
