//! A form without typing: each field a list of choices, changed with ← and
//! →, and a Start button at the bottom.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use super::moved;

#[derive(Debug, Clone)]
pub struct Field {
    pub label: &'static str,
    pub options: Vec<String>,
    pub at: usize,
}

impl Field {
    pub fn new(label: &'static str, options: Vec<String>, at: usize) -> Self {
        let at = at.min(options.len().saturating_sub(1));
        Self { label, options, at }
    }

    pub fn value(&self) -> &str {
        self.options.get(self.at).map_or("", String::as_str)
    }
}

/// What a key did to the form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Nothing,
    Changed,
    Submit,
    Cancel,
}

#[derive(Debug, Clone)]
pub struct Form {
    pub fields: Vec<Field>,
    /// A field, or `fields.len()` for the button.
    pub selected: usize,
    pub button: &'static str,
}

impl Form {
    pub fn new(fields: Vec<Field>, button: &'static str) -> Self {
        Self {
            fields,
            selected: 0,
            button,
        }
    }

    /// The value of the field labelled `label`.
    pub fn value(&self, label: &str) -> &str {
        self.fields
            .iter()
            .find(|f| f.label == label)
            .map_or("", Field::value)
    }

    pub fn index(&self, label: &str) -> usize {
        self.fields.iter().find(|f| f.label == label).map_or(0, |f| f.at)
    }

    pub fn key(&mut self, key: KeyEvent) -> Outcome {
        let rows = self.fields.len() + 1;

        match key.code {
            KeyCode::Up | KeyCode::BackTab => self.selected = moved(self.selected, -1, rows),
            KeyCode::Down | KeyCode::Tab => self.selected = moved(self.selected, 1, rows),
            KeyCode::Left => return self.turn(-1),
            KeyCode::Right => return self.turn(1),
            KeyCode::Enter if self.selected == self.fields.len() => return Outcome::Submit,
            KeyCode::Enter => self.selected = moved(self.selected, 1, rows),
            KeyCode::Esc | KeyCode::Backspace => return Outcome::Cancel,
            _ => {}
        }

        Outcome::Nothing
    }

    /// Steps the selected field's choice, wrapping round.
    fn turn(&mut self, step: isize) -> Outcome {
        let Some(field) = self.fields.get_mut(self.selected) else {
            return Outcome::Nothing;
        };
        let n = field.options.len() as isize;
        if n < 2 {
            return Outcome::Nothing;
        }

        field.at = (field.at as isize + step).rem_euclid(n) as usize;
        Outcome::Changed
    }

    /// The fields, the button, then `note` (what Start will do).
    pub fn draw(&self, frame: &mut Frame, area: Rect, note: &[Line<'static>]) {
        let [fields, notes] = Layout::vertical([
            Constraint::Length(self.fields.len() as u16 + 3),
            Constraint::Min(0),
        ])
        .areas(area);

        let mut lines: Vec<Line> = self
            .fields
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let chosen = i == self.selected;
                let value_style = match chosen {
                    true => Style::new().add_modifier(Modifier::REVERSED),
                    false => Style::new().add_modifier(Modifier::BOLD),
                };
                Line::from(vec![
                    Span::raw(if chosen { "▶ " } else { "  " }),
                    Span::styled(format!("{:<26}", f.label), Style::new().fg(Color::Gray)),
                    Span::styled(format!("◀ {} ▶", f.value()), value_style),
                ])
            })
            .collect();

        let on_button = self.selected == self.fields.len();
        let button_style = match on_button {
            true => Style::new()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
            false => Style::new().fg(Color::Green),
        };
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::raw(if on_button { "▶ " } else { "  " }),
            Span::styled(format!("[ {} ]", self.button), button_style),
        ]));

        frame.render_widget(Paragraph::new(lines), fields);
        frame.render_widget(Paragraph::new(note.to_vec()).wrap(Wrap { trim: false }), notes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn press(form: &mut Form, code: KeyCode) -> Outcome {
        form.key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn arrows_change_choices_and_enter_on_the_button_submits() {
        let mut form = Form::new(
            vec![
                Field::new("step", vec!["lns".into(), "turns".into()], 0),
                Field::new("seconds", vec!["60".into(), "300".into(), "600".into()], 1),
            ],
            "Start",
        );

        assert_eq!(press(&mut form, KeyCode::Left), Outcome::Changed);
        assert_eq!(form.value("step"), "turns");
        press(&mut form, KeyCode::Down);
        press(&mut form, KeyCode::Right);
        press(&mut form, KeyCode::Right);
        assert_eq!(form.value("seconds"), "60");

        assert_eq!(press(&mut form, KeyCode::Enter), Outcome::Nothing);
        assert_eq!(form.selected, 2);
        assert_eq!(press(&mut form, KeyCode::Enter), Outcome::Submit);
        assert_eq!(press(&mut form, KeyCode::Esc), Outcome::Cancel);
    }
}
