//! One instance: its facts, what can be done with it, and a look at its
//! best tour drawn in the terminal.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::widgets::canvas::{Canvas, Line as Stroke, Points};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use super::{moved, step_of, Ctx, Nav, Task};
use crate::preview::Preview;

/// The colors of the cutters, as in the SVG pictures (Okabe-Ito).
pub const CUTTER_COLORS: [Color; 6] = [
    Color::Rgb(0x00, 0x72, 0xB2),
    Color::Rgb(0xE6, 0x9F, 0x00),
    Color::Rgb(0xCC, 0x79, 0xA7),
    Color::Rgb(0x00, 0x9E, 0x73),
    Color::Rgb(0x56, 0xB4, 0xE9),
    Color::Rgb(0xEE, 0xEE, 0xEE),
];

const LAWN: Color = Color::Rgb(0x4a, 0x6b, 0x3f);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    ViewBest,
    Candidates,
    Jobs,
}

const ACTIONS: [(Action, &str); 3] = [
    (Action::ViewBest, "Open the best tour in the browser"),
    (Action::Candidates, "Candidates: compare one with the best"),
    (Action::Jobs, "Job history"),
];

#[derive(Debug)]
pub struct Detail {
    pub uid: String,
    pub preview: Preview,
    /// Tour lengths of the best, one per cutter.
    pub lengths: Vec<u64>,
    pub selected: usize,
}

impl Detail {
    pub fn new(uid: String, preview: Preview, lengths: Vec<u64>) -> Self {
        Self {
            uid,
            preview,
            lengths,
            selected: 0,
        }
    }

    pub fn key(&mut self, key: KeyEvent) -> Nav {
        if let Some(step) = step_of(key, 3) {
            self.selected = moved(self.selected, step, ACTIONS.len());
            return Nav::Stay;
        }

        match key.code {
            KeyCode::Enter => {
                let uid = self.uid.clone();
                Nav::Run(match ACTIONS[self.selected].0 {
                    Action::ViewBest => Task::ViewBest(uid),
                    Action::Candidates => Task::Candidates(uid),
                    Action::Jobs => Task::Jobs(uid),
                })
            }
            KeyCode::Esc | KeyCode::Backspace | KeyCode::Char('q') => Nav::Pop,
            _ => Nav::Stay,
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect, ctx: &Ctx) {
        let [left, right] = Layout::horizontal([Constraint::Length(52), Constraint::Min(20)]).areas(area);
        let [facts, actions] = Layout::vertical([Constraint::Min(14), Constraint::Length(6)]).areas(left);

        frame.render_widget(self.facts(ctx), facts);

        let items: Vec<ListItem> = ACTIONS.iter().map(|(_, name)| ListItem::new(*name)).collect();
        let list = List::new(items)
            .block(Block::new().title("Do").borders(Borders::TOP))
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
            .highlight_symbol("▶ ");
        let mut state = ListState::default().with_selected(Some(self.selected));
        frame.render_stateful_widget(list, actions, &mut state);

        frame.render_widget(self.canvas(), right);
    }

    fn facts(&self, ctx: &Ctx) -> Paragraph<'static> {
        let Some(inst) = ctx.lawn.find(&self.uid) else {
            return Paragraph::new("not in the catalog");
        };
        let best = inst.best.as_ref();
        let field = |name: &str, value: String| {
            Line::from(vec![
                Span::styled(format!("{name:<13}"), Style::new().fg(Color::Gray)),
                Span::raw(value),
            ])
        };

        let mut lines = vec![
            field("cutters", inst.k.to_string()),
            field("family", inst.family.to_owned()),
            field("cells", inst.cells.to_string()),
            field("cutter cells", inst.cutter_cells.to_string()),
            field("lower bound", inst.lower_bound.to_string()),
            field(
                "best",
                best.map_or("none".to_owned(), |b| b.objective.to_string()),
            ),
            field("gap", inst.gap().map_or("–".to_owned(), |g| format!("{g:.1}%"))),
            field("points", format!("{:.4}", inst.points())),
            field("found by", best.map_or(String::new(), |b| b.label.clone())),
            Line::raw(""),
        ];
        for (i, length) in self.lengths.iter().enumerate() {
            let swatch = Span::styled("■ ", Style::new().fg(CUTTER_COLORS[i % CUTTER_COLORS.len()]));
            lines.push(Line::from(vec![
                swatch,
                Span::raw(format!("cutter {}: {length}", i + 1)),
            ]));
        }

        Paragraph::new(lines).block(Block::new().title("Facts").borders(Borders::TOP))
    }

    fn canvas(&self) -> impl ratatui::widgets::Widget + '_ {
        let p = &self.preview;

        Canvas::default()
            .block(Block::new().title("Best tour").borders(Borders::ALL))
            .marker(Marker::Braille)
            .x_bounds(p.x)
            .y_bounds(p.y)
            .paint(move |ctx| {
                ctx.draw(&Points {
                    coords: &p.region,
                    color: LAWN,
                });
                ctx.layer();

                for (i, route) in p.routes.iter().enumerate() {
                    let color = CUTTER_COLORS[i % CUTTER_COLORS.len()];
                    for pair in route.windows(2) {
                        ctx.draw(&Stroke::new(pair[0].0, pair[0].1, pair[1].0, pair[1].1, color));
                    }
                }
            })
    }
}
