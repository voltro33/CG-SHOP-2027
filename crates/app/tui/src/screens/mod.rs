//! The screens, one module each. A screen keeps its own state, draws
//! itself, and answers a key with a [`Nav`]: stay, open a screen, go back,
//! or a [`Task`] the app runs (anything that reads files or is slow).

pub mod candidates;
pub mod confirm;
pub mod dashboard;
pub mod detail;
pub mod experiments;
pub mod form;
pub mod instances;
pub mod jobs;
pub mod menu;
pub mod merge;
pub mod runs;
pub mod submission;

use std::path::PathBuf;

use crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::widgets::Table;
use ratatui::Frame;

use crate::bests::{Backup, Plan, Source};
use crate::binaries::Commit;
use crate::data::{Lawn, Paths};
use crate::history::Candidate;
use crate::runs::Spec;
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
    /// Close this screen, then run the task (a form's or a question's).
    PopAndRun(Task),
    Quit,
    Run(Task),
    /// A line for the status bar.
    Say(String),
}

/// Work the app does between two frames, after showing that it is busy.
#[derive(Debug, Clone)]
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
    /// Start a background run.
    StartRun(Box<Spec>),
    /// Stop the run in this folder.
    StopRun(PathBuf),
    /// Show a folder in the file manager.
    OpenFolder(PathBuf),
    /// Open the Bests screen: the sources and the merges so far.
    OpenBests,
    PlanMerge(Source),
    Merge(Box<Plan>),
    Undo(Box<Backup>),
    Pack,
    SaveBinary,
    OpenCommits,
    Build(Commit),
    StartExperiment(Box<crate::experiments::Choice>),
    OpenReport(PathBuf),
    MergeExperiment(PathBuf),
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
            Self::StartRun(spec) => format!("starting {}…", spec.title),
            Self::StopRun(_) => "stopping the run…".to_owned(),
            Self::OpenFolder(_) => "opening the folder…".to_owned(),
            Self::OpenBests => "looking for tours to merge…".to_owned(),
            Self::PlanMerge(source) => format!("reading {}…", source.name),
            Self::Merge(_) => "merging…".to_owned(),
            Self::Undo(_) => "undoing the merge…".to_owned(),
            Self::Pack => "starting the packing…".to_owned(),
            Self::SaveBinary => "saving the binary…".to_owned(),
            Self::OpenCommits => "reading the commits…".to_owned(),
            Self::Build(c) => format!("starting the build of {}…", c.short),
            Self::StartExperiment(_) => "copying the bests into the arms…".to_owned(),
            Self::OpenReport(_) => "reading the experiment…".to_owned(),
            Self::MergeExperiment(_) => "merging both arms…".to_owned(),
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
    Confirm(confirm::Confirm),
    Runs(runs::Runs),
    RunDetail(Box<runs::RunDetail>),
    StartPass(runs::StartPass),
    Bests(merge::Bests),
    MergePlan(merge::MergePlan),
    Submission(submission::Submission),
    Experiments(experiments::Experiments),
    Binaries(experiments::Binaries),
    Commits(experiments::Commits),
    NewExperiment(experiments::NewExperiment),
    Report(Box<experiments::ReportScreen>),
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
            Self::Confirm(_) => "Confirm".to_owned(),
            Self::Runs(_) => "Runs".to_owned(),
            Self::RunDetail(r) => r.title(),
            Self::StartPass(_) => "Start a pass".to_owned(),
            Self::Bests(_) => "Bests".to_owned(),
            Self::MergePlan(m) => m.plan.source.name.clone(),
            Self::Submission(_) => "Submission".to_owned(),
            Self::Experiments(_) => "Experiments".to_owned(),
            Self::Binaries(_) => "Binaries".to_owned(),
            Self::Commits(_) => "Build a commit".to_owned(),
            Self::NewExperiment(_) => "New experiment".to_owned(),
            Self::Report(r) => r.setup.id.clone(),
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
            Self::Confirm(_) => "Enter or y yes · Esc or n no",
            Self::Runs(_) => "↑↓ choose · Enter open · Esc back · reads again every 3 s",
            Self::RunDetail(_) => "s stop it · o open its folder · Esc back · reads again every 3 s",
            Self::StartPass(_) | Self::NewExperiment(_) => {
                "↑↓ field · ←→ change · Enter on the button starts · Esc cancel"
            }
            Self::Bests(b) => b.hints(),
            Self::MergePlan(_) => "↑↓ scroll · Enter or m merge · Esc back",
            Self::Submission(_) => "↑↓ choose · Enter pack, or show the folder · Esc back",
            Self::Experiments(_) => "↑↓ choose · Enter open · Esc back",
            Self::Binaries(_) => "↑↓ choose · Enter do · Esc back",
            Self::Commits(_) => "↑↓ choose · Enter build · Esc back",
            Self::Report(_) => "↑↓ scroll · m merge the shorter tours · o its run · Esc back",
        }
    }

    /// Called before every frame: screens that follow the disk read it again.
    pub fn tick(&mut self, ctx: &Ctx) {
        match self {
            Self::Runs(s) => s.tick(ctx),
            Self::RunDetail(s) => s.tick(),
            Self::Experiments(s) => s.tick(ctx),
            Self::Report(s) => s.tick(ctx),
            _ => {}
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
            Self::Confirm(s) => s.key(key),
            Self::Runs(s) => s.key(key, ctx),
            Self::RunDetail(s) => s.key(key),
            Self::StartPass(s) => s.key(key, ctx),
            Self::Bests(s) => s.key(key),
            Self::MergePlan(s) => s.key(key),
            Self::Submission(s) => s.key(key, ctx),
            Self::Experiments(s) => s.key(key, ctx),
            Self::Binaries(s) => s.key(key),
            Self::Commits(s) => s.key(key),
            Self::NewExperiment(s) => s.key(key),
            Self::Report(s) => s.key(key),
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
            Self::Confirm(s) => s.draw(frame, area),
            Self::Runs(s) => s.draw(frame, area),
            Self::RunDetail(s) => s.draw(frame, area),
            Self::StartPass(s) => s.draw(frame, area, ctx),
            Self::Bests(s) => s.draw(frame, area),
            Self::MergePlan(s) => s.draw(frame, area),
            Self::Submission(s) => s.draw(frame, area),
            Self::Experiments(s) => s.draw(frame, area),
            Self::Binaries(s) => s.draw(frame, area),
            Self::Commits(s) => s.draw(frame, area),
            Self::NewExperiment(s) => s.draw(frame, area, ctx),
            Self::Report(s) => s.draw(frame, area),
        }
    }
}

/// Action lines above a table, one selection moving down through both:
/// `selected` below `actions.len()` is an action, the rest table rows.
pub fn draw_with_actions(frame: &mut Frame, area: Rect, actions: &[&str], table: Table, selected: usize) {
    use ratatui::layout::{Constraint, Layout};
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::text::Line;
    use ratatui::widgets::{Paragraph, TableState};

    let [top, gap, body] = Layout::vertical([
        Constraint::Length(actions.len() as u16),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(area);
    let _ = gap;

    let lines: Vec<Line> = actions
        .iter()
        .enumerate()
        .map(|(i, text)| {
            let style = Style::new().fg(Color::Green).add_modifier(Modifier::BOLD);
            match i == selected {
                true => Line::styled(format!("▶ ▸ {text}"), style.add_modifier(Modifier::REVERSED)),
                false => Line::styled(format!("  ▸ {text}"), style),
            }
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), top);

    let row = selected.checked_sub(actions.len());
    let table = table
        .row_highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol("▶ ");
    let mut state = TableState::default().with_selected(row);
    frame.render_stateful_widget(table, body, &mut state);
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
