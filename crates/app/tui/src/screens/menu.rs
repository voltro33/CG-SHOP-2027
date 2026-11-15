//! The main menu: where every part of the tool starts.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use super::experiments::Experiments;
use super::runs::Runs;
use super::submission::Submission;
use super::{dashboard::Dashboard, instances::Instances, moved, step_of, Ctx, Nav, Screen, Task};

/// One menu entry: its name and what it is for.
struct Entry {
    name: &'static str,
    about: &'static str,
}

const ENTRIES: [Entry; 7] = [
    Entry {
        name: "Dashboard",
        about: "the score, by k and by cutter family, and the gaps to the bounds",
    },
    Entry {
        name: "Instances",
        about: "all 250: their bests, a look at each tour, candidates and history",
    },
    Entry {
        name: "Runs",
        about: "start a pass, watch it, stop it; runs keep going when the menus close",
    },
    Entry {
        name: "Experiments",
        about: "A/B tests between two binaries, building a commit, merging the winner",
    },
    Entry {
        name: "Bests",
        about: "merge shorter tours from another folder, with undo",
    },
    Entry {
        name: "Submission",
        about: "pack and verify a zip; never uploads",
    },
    Entry {
        name: "Quit",
        about: "",
    },
];

const QUIT: usize = ENTRIES.len() - 1;

#[derive(Debug, Default)]
pub struct Menu {
    pub selected: usize,
}

impl Menu {
    pub fn key(&mut self, key: KeyEvent, ctx: &Ctx) -> Nav {
        if let Some(step) = step_of(key, 3) {
            self.selected = moved(self.selected, step, ENTRIES.len());
            return Nav::Stay;
        }

        match key.code {
            KeyCode::Enter => self.open(ctx),
            KeyCode::Esc | KeyCode::Char('q') => Nav::Quit,
            _ => Nav::Stay,
        }
    }

    fn open(&self, ctx: &Ctx) -> Nav {
        match self.selected {
            0 => Nav::Push(Screen::Dashboard(Dashboard)),
            1 => Nav::Push(Screen::Instances(Instances::new(&ctx.lawn))),
            2 => Nav::Push(Screen::Runs(Runs::default())),
            3 => Nav::Push(Screen::Experiments(Experiments::default())),
            4 => Nav::Run(Task::OpenBests),
            5 => Nav::Push(Screen::Submission(Submission::new(ctx))),
            QUIT => Nav::Quit,
            _ => Nav::Stay,
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect, ctx: &Ctx) {
        let [head, body] = Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).areas(area);

        let score = format!(
            "  score {:.3} of {}   ·   {} bests stored",
            ctx.summary.all.points, ctx.summary.all.count, ctx.summary.all.solved
        );
        frame.render_widget(
            Paragraph::new(score).block(Block::new().borders(Borders::BOTTOM)),
            head,
        );

        let items: Vec<ListItem> = ENTRIES.iter().map(item).collect();
        let list = List::new(items)
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
            .highlight_symbol("▶ ");
        let mut state = ListState::default().with_selected(Some(self.selected));
        frame.render_stateful_widget(list, body, &mut state);
    }
}

fn item(entry: &Entry) -> ListItem<'static> {
    ListItem::new(Line::from(vec![
        Span::styled(
            format!("{:<13}", entry.name),
            Style::new().add_modifier(Modifier::BOLD),
        ),
        Span::styled(entry.about, Style::new().fg(Color::Gray)),
    ]))
}
