//! The main menu: where every part of the tool starts.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use super::{dashboard::Dashboard, instances::Instances, moved, step_of, Ctx, Nav, Screen};

/// One menu entry: its name, what it is for, and whether it is built yet.
struct Entry {
    name: &'static str,
    about: &'static str,
    ready: bool,
}

const ENTRIES: [Entry; 7] = [
    Entry {
        name: "Dashboard",
        about: "the score, by k and by cutter family, and the gaps to the bounds",
        ready: true,
    },
    Entry {
        name: "Instances",
        about: "all 250: their bests, a look at each tour, candidates and history",
        ready: true,
    },
    Entry {
        name: "Runs",
        about: "start, watch and stop passes (coming next)",
        ready: false,
    },
    Entry {
        name: "Experiments",
        about: "A/B tests between two commits (coming)",
        ready: false,
    },
    Entry {
        name: "Bests",
        about: "merge shorter tours from a folder, with undo (coming)",
        ready: false,
    },
    Entry {
        name: "Submission",
        about: "pack and verify a zip; never uploads (coming)",
        ready: false,
    },
    Entry {
        name: "Quit",
        about: "",
        ready: true,
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
        let entry = &ENTRIES[self.selected];
        if !entry.ready {
            return Nav::Say(format!("{} is not built yet", entry.name));
        }

        match self.selected {
            0 => Nav::Push(Screen::Dashboard(Dashboard)),
            1 => Nav::Push(Screen::Instances(Instances::new(&ctx.lawn))),
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
    let name_style = match entry.ready {
        true => Style::new().add_modifier(Modifier::BOLD),
        false => Style::new().fg(Color::DarkGray),
    };

    ListItem::new(Line::from(vec![
        Span::styled(format!("{:<13}", entry.name), name_style),
        Span::styled(entry.about, Style::new().fg(Color::Gray)),
    ]))
}
