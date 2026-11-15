//! The screens, one module each. A screen keeps its own state, draws
//! itself, and answers a key with a [`Nav`]: stay, open a screen, go back,
//! or a [`Task`] the app runs (anything that reads files or is slow).

pub mod candidates;
pub mod dashboard;
pub mod detail;
pub mod instances;
pub mod jobs;
pub mod menu;

use crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::data::{Lawn, Paths};
use crate::history::Candidate;
use crate::summary::Summary;

/// What every screen may read.
#[derive(Debug)]
pub struct Ctx {
    pub paths: Paths,
    pub lawn: Lawn,
    pub summary: Summary,
}

/// What a key asks for.
#[derive(Debug)]
pub enum Nav {
    Stay,
    Push(Screen),
    Pop,
    Quit,
    Run(Task),
    /// A line for the status bar.
    Say(String),
}

/// Work the app does between two frames, after showing that it is busy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Task {
    /// Load the instance and its best, then open its screen.
    Detail(String),
    /// Open the best tour in the browser.
    ViewBest(String),
    /// The best and a candidate side by side in the browser.
    Compare {
        uid: String,
        candidate: Candidate,
    },
    Candidates(String),
    Jobs(String),
    /// Read the catalog and the bests again.
    Reload,
}

impl Task {
    /// What the status bar says while it runs.
    pub fn busy(&self) -> String {
        match self {
            Self::Detail(uid) => format!("loading {uid}…"),
            Self::ViewBest(uid) => format!("drawing {uid} for the browser…"),
            Self::Compare { uid, .. } => format!("drawing {uid} and a candidate…"),
            Self::Candidates(uid) => format!("reading the candidates of {uid}…"),
            Self::Jobs(uid) => format!("reading the job log for {uid}…"),
            Self::Reload => "reading the bests again…".to_owned(),
        }
    }
}

#[derive(Debug)]
pub enum Screen {
    Menu(menu::Menu),
    Dashboard(dashboard::Dashboard),
    Instances(instances::Instances),
    Detail(Box<detail::Detail>),
    Candidates(candidates::Candidates),
    Jobs(jobs::Jobs),
}

impl Screen {
    pub fn title(&self) -> String {
        match self {
            Self::Menu(_) => "Main menu".to_owned(),
            Self::Dashboard(_) => "Dashboard".to_owned(),
            Self::Instances(_) => "Instances".to_owned(),
            Self::Detail(d) => d.uid.clone(),
            Self::Candidates(_) => "Candidates".to_owned(),
            Self::Jobs(_) => "Job history".to_owned(),
        }
    }

    /// The keys this screen answers, for the footer.
    pub fn hints(&self) -> &'static str {
        match self {
            Self::Menu(_) => "↑↓ choose · Enter open · q quit",
            Self::Dashboard(_) => "r refresh · Esc back",
            Self::Instances(i) => i.hints(),
            Self::Detail(_) => "↑↓ choose · Enter do · Esc back",
            Self::Candidates(_) => "↑↓ choose · Enter compare with the best in the browser · Esc back",
            Self::Jobs(_) => "↑↓ PgUp PgDn scroll · Esc back",
        }
    }

    pub fn key(&mut self, key: KeyEvent, ctx: &Ctx) -> Nav {
        match self {
            Self::Menu(s) => s.key(key, ctx),
            Self::Dashboard(s) => s.key(key),
            Self::Instances(s) => s.key(key, ctx),
            Self::Detail(s) => s.key(key),
            Self::Candidates(s) => s.key(key),
            Self::Jobs(s) => s.key(key),
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) {
        match self {
            Self::Menu(s) => s.draw(frame, area, ctx),
            Self::Dashboard(s) => s.draw(frame, area, ctx),
            Self::Instances(s) => s.draw(frame, area, ctx),
            Self::Detail(s) => s.draw(frame, area, ctx),
            Self::Candidates(s) => s.draw(frame, area),
            Self::Jobs(s) => s.draw(frame, area),
        }
    }
}

/// Moves a selection by `step` within `len` entries, stopping at the ends.
pub fn moved(selected: usize, step: isize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }

    selected.saturating_add_signed(step).min(len - 1)
}

/// The selection step of a navigation key, if it is one.
pub fn step_of(key: KeyEvent, page: isize) -> Option<isize> {
    use crossterm::event::KeyCode;

    match key.code {
        KeyCode::Up => Some(-1),
        KeyCode::Down => Some(1),
        KeyCode::PageUp => Some(-page),
        KeyCode::PageDown => Some(page),
        KeyCode::Home => Some(isize::MIN / 2),
        KeyCode::End => Some(isize::MAX / 2),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selections_stop_at_the_ends() {
        assert_eq!(moved(0, -1, 5), 0);
        assert_eq!(moved(3, 10, 5), 4);
        assert_eq!(moved(2, isize::MIN / 2, 5), 0);
        assert_eq!(moved(0, 1, 0), 0);
    }
}
