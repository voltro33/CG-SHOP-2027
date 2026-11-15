//! Experiments: A/B tests between two kept binaries, the binaries
//! themselves, building one from a commit, and each experiment's report.

use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Cell, Paragraph, Row, Table, TableState};
use ratatui::Frame;

use super::confirm::Confirm;
use super::form::{Field, Form, Outcome};
use super::runs::{read_runs, status_style, Timer};
use super::{draw_with_actions, moved, step_of, Ctx, Nav, Screen, Task};
use crate::binaries::{self, Binary, Commit};
use crate::clock::ago;
use crate::experiments::{self, Choice, Report, Setup};
use crate::recipes::Set;
use crate::runs::{Run, Status};

/// The fixed entries above the experiments.
const NEW: usize = 0;
const BINARIES: usize = 1;
const FIXED: usize = 2;

#[derive(Debug, Default)]
pub struct Experiments {
    pub list: Vec<(PathBuf, Setup, Option<Status>)>,
    selected: usize,
    timer: Timer,
}

impl Experiments {
    pub fn tick(&mut self, ctx: &Ctx) {
        if !self.timer.due() {
            return;
        }

        let runs = read_runs(ctx);
        let status = |id: &str| runs.iter().find(|r| r.spec.id == id).map(|r| r.status.clone());
        self.list = experiments::list(&ctx.paths)
            .into_iter()
            .map(|(dir, setup)| {
                let s = status(&setup.run);
                (dir, setup, s)
            })
            .collect();
    }

    pub fn key(&mut self, key: KeyEvent, ctx: &Ctx) -> Nav {
        if let Some(step) = step_of(key, 10) {
            self.selected = moved(self.selected, step, self.list.len() + FIXED);
            return Nav::Stay;
        }

        match key.code {
            KeyCode::Enter => match self.selected {
                NEW => match NewExperiment::new(ctx) {
                    Ok(form) => Nav::Push(Screen::NewExperiment(form)),
                    Err(e) => Nav::Say(e),
                },
                BINARIES => Nav::Push(Screen::Binaries(Binaries::new(ctx))),
                i => match self.list.get(i - FIXED) {
                    Some((dir, _, _)) => Nav::Run(Task::OpenReport(dir.clone())),
                    None => Nav::Stay,
                },
            },
            KeyCode::Esc | KeyCode::Backspace | KeyCode::Char('q') => Nav::Pop,
            _ => Nav::Stay,
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let rows: Vec<Row> = self
            .list
            .iter()
            .map(|(_, setup, status)| {
                let word = status.as_ref().map_or("?", Status::word);
                let style = status.as_ref().map_or(Style::new(), status_style);
                Row::new([
                    Cell::from(ago(setup.made)),
                    Cell::from(word).style(style),
                    Cell::from(format!("{} vs {}", setup.base, setup.fast)),
                    Cell::from(format!(
                        "{}, {} instances, {} s",
                        setup.set,
                        setup.start.len(),
                        setup.seconds
                    )),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(14),
                Constraint::Length(10),
                Constraint::Length(36),
                Constraint::Min(20),
            ],
        )
        .header(
            Row::new(["made", "status", "binaries", "on"]).style(Style::new().add_modifier(Modifier::BOLD)),
        );
        draw_with_actions(
            frame,
            area,
            &["New experiment…", "Binaries…"],
            table,
            self.selected,
        );
    }
}

/// The kept binaries, and the ways to add one.
#[derive(Debug)]
pub struct Binaries {
    pub list: Vec<Binary>,
    selected: usize,
}

impl Binaries {
    pub fn new(ctx: &Ctx) -> Self {
        Self {
            list: binaries::list(&ctx.paths),
            selected: 0,
        }
    }

    pub fn key(&mut self, key: KeyEvent) -> Nav {
        if let Some(step) = step_of(key, 10) {
            self.selected = moved(self.selected, step, self.list.len() + 2);
            return Nav::Stay;
        }

        match key.code {
            KeyCode::Enter if self.selected == 0 => Nav::Run(Task::SaveBinary),
            KeyCode::Enter if self.selected == 1 => Nav::Run(Task::OpenCommits),
            KeyCode::Esc | KeyCode::Backspace | KeyCode::Char('q') => Nav::Pop,
            _ => Nav::Stay,
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let rows: Vec<Row> = self
            .list
            .iter()
            .map(|b| {
                let (made, subject) = b.meta.as_ref().map_or((String::new(), String::new()), |m| {
                    let dirty = if m.dirty {
                        " (with uncommitted changes)"
                    } else {
                        ""
                    };
                    (ago(m.made), format!("{}{dirty}", m.subject))
                });
                Row::new([b.label.clone(), made, subject])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(36),
                Constraint::Length(14),
                Constraint::Min(20),
            ],
        )
        .header(Row::new(["binary", "kept", "commit"]).style(Style::new().add_modifier(Modifier::BOLD)));
        let actions = ["Save the build this menu runs on", "Build a commit…"];
        draw_with_actions(frame, area, &actions, table, self.selected);
    }
}

/// Recent commits, one to build.
#[derive(Debug)]
pub struct Commits {
    pub list: Vec<Commit>,
    selected: usize,
}

impl Commits {
    pub fn new(list: Vec<Commit>) -> Self {
        Self { list, selected: 0 }
    }

    pub fn key(&mut self, key: KeyEvent) -> Nav {
        if let Some(step) = step_of(key, 10) {
            self.selected = moved(self.selected, step, self.list.len());
            return Nav::Stay;
        }

        match key.code {
            KeyCode::Enter => match self.list.get(self.selected) {
                Some(c) => Nav::Push(Screen::Confirm(Confirm::new(
                    vec![
                        format!("Build {} \"{}\"?", c.short, c.subject),
                        "It checks the commit out in a worktree under data/rust/worktrees and builds it in the background \
                         (a few minutes the first time). Follow it under Runs; the binary then appears under Binaries."
                            .to_owned(),
                    ],
                    Task::Build(c.clone()),
                ))),
                None => Nav::Stay,
            },
            KeyCode::Esc | KeyCode::Backspace | KeyCode::Char('q') => Nav::Pop,
            _ => Nav::Stay,
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let rows: Vec<Row> = self
            .list
            .iter()
            .map(|c| Row::new([c.short.clone(), ago(c.time), c.subject.clone()]))
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(10),
                Constraint::Length(14),
                Constraint::Min(20),
            ],
        )
        .row_highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol("▶ ");
        let mut state = TableState::default().with_selected(Some(self.selected));
        frame.render_stateful_widget(table, area, &mut state);
    }
}

/// The form of a new experiment.
#[derive(Debug)]
pub struct NewExperiment {
    pub form: Form,
    binaries: Vec<Binary>,
    sets: Vec<Set>,
}

const BASE: &str = "base binary";
const FAST: &str = "fast binary";
const INSTANCES: &str = "instances";
const SECONDS: &str = "seconds per instance";
const THREADS: &str = "threads per arm";
const SEED: &str = "seed";

const SECOND_CHOICES: [u32; 6] = [60, 120, 300, 600, 1200, 1800];
const SEEDS: [u64; 4] = [2027, 1, 7, 42];

impl NewExperiment {
    pub fn new(ctx: &Ctx) -> Result<Self, String> {
        let binaries = binaries::list(&ctx.paths);
        if binaries.is_empty() {
            return Err("no binaries kept yet: add one under Binaries first".to_owned());
        }

        let mut sets = vec![
            Set::Bench,
            Set::All,
            Set::Cutters(1),
            Set::Cutters(2),
            Set::Cutters(3),
        ];
        let mut families: Vec<&'static str> = ctx.lawn.instances.iter().map(|i| i.family).collect();
        families.sort_unstable();
        families.dedup();
        sets.extend(families.into_iter().map(Set::Family));

        let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
        let labels: Vec<String> = binaries.iter().map(|b| b.label.clone()).collect();
        let fields = vec![
            Field::new(BASE, labels.clone(), 1.min(labels.len() - 1)),
            Field::new(FAST, labels, 0),
            Field::new(INSTANCES, sets.iter().map(Set::name).collect(), 0),
            Field::new(SECONDS, SECOND_CHOICES.iter().map(|s| s.to_string()).collect(), 2),
            Field::new(
                THREADS,
                (1..=cores / 2).map(|t| t.to_string()).collect(),
                (cores / 2).saturating_sub(2),
            ),
            Field::new(SEED, SEEDS.iter().map(|s| s.to_string()).collect(), 0),
        ];

        Ok(Self {
            form: Form::new(fields, "Start the experiment"),
            binaries,
            sets,
        })
    }

    fn choice(&self) -> Choice {
        Choice {
            base: self.binaries[self.form.index(BASE)].clone(),
            fast: self.binaries[self.form.index(FAST)].clone(),
            set: self.sets[self.form.index(INSTANCES)].clone(),
            seconds: SECOND_CHOICES[self.form.index(SECONDS)],
            threads: self.form.index(THREADS) + 1,
            seed: SEEDS[self.form.index(SEED)],
        }
    }

    pub fn key(&mut self, key: KeyEvent) -> Nav {
        match self.form.key(key) {
            Outcome::Submit => Nav::PopAndRun(Task::StartExperiment(Box::new(self.choice()))),
            Outcome::Cancel => Nav::Pop,
            Outcome::Changed | Outcome::Nothing => Nav::Stay,
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect, ctx: &Ctx) {
        let c = self.choice();
        let count = c.set.uids(&ctx.paths, &ctx.lawn).map_or(0, |u| u.len());
        let hours = count as f64 * f64::from(c.seconds) / c.threads as f64 / 3600.0;

        let mut note = vec![Line::raw(format!(
            "Both arms run lns from copies of the same bests and state, same seed, {} threads each, about {hours:.1} h.",
            c.threads
        ))];
        if c.base.label == c.fast.label {
            note.push(Line::styled(
                "Both arms run the same binary: this measures noise, not a change.",
                Style::new().fg(Color::Yellow),
            ));
        }
        note.push(Line::raw(
            "The stored bests are not touched until you merge from the report.",
        ));

        self.form.draw(frame, area, &note);
    }
}

/// One experiment's verdict, read again every few seconds while it runs.
#[derive(Debug)]
pub struct ReportScreen {
    pub dir: PathBuf,
    pub setup: Setup,
    pub report: Option<Report>,
    pub run: Option<Run>,
    error: Option<String>,
    table: TableState,
    timer: Timer,
}

impl ReportScreen {
    pub fn new(dir: PathBuf, setup: Setup) -> Self {
        Self {
            dir,
            setup,
            report: None,
            run: None,
            error: None,
            table: TableState::default().with_selected(Some(0)),
            timer: Timer::default(),
        }
    }

    pub fn tick(&mut self, ctx: &Ctx) {
        if !self.timer.due() {
            return;
        }

        self.run = read_runs(ctx).into_iter().find(|r| r.spec.id == self.setup.run);
        let logs = self.run.as_ref().map_or_else(Vec::new, Run::logs);
        let (base, fast) = match logs.as_slice() {
            [base, fast, ..] => (base.clone(), fast.clone()),
            _ => (PathBuf::new(), PathBuf::new()),
        };

        match experiments::report(&self.dir, &self.setup, &ctx.lawn, (&base, &fast)) {
            Ok(r) => self.report = Some(r),
            Err(e) => self.error = Some(e),
        }
    }

    pub fn key(&mut self, key: KeyEvent) -> Nav {
        let rows = self.report.as_ref().map_or(0, |r| r.lines.len());
        if let Some(step) = step_of(key, 20) {
            self.table
                .select(Some(moved(self.table.selected().unwrap_or(0), step, rows)));
            return Nav::Stay;
        }

        match key.code {
            KeyCode::Char('m') => {
                if self.run.as_ref().is_some_and(Run::is_running) {
                    return Nav::Say("it is still running: merge when it is done".to_owned());
                }
                Nav::Push(Screen::Confirm(Confirm::new(
                    vec![
                        "Merge the shorter tours of both arms into data/rust/best?".to_owned(),
                        "Only tours shorter than the stored bests are taken, each merge backed up for undo."
                            .to_owned(),
                    ],
                    Task::MergeExperiment(self.dir.clone()),
                )))
            }
            KeyCode::Char('o') => match &self.run {
                Some(run) => Nav::Push(Screen::RunDetail(Box::new(super::runs::RunDetail::new(
                    run.dir.clone(),
                )))),
                None => Nav::Say("its run is not found".to_owned()),
            },
            KeyCode::Esc | KeyCode::Backspace | KeyCode::Char('q') => Nav::Pop,
            _ => Nav::Stay,
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let Some(report) = &self.report else {
            frame.render_widget(
                Paragraph::new(self.error.clone().unwrap_or_else(|| "reading…".to_owned())),
                area,
            );
            return;
        };
        let [head, body] = Layout::vertical([Constraint::Length(5), Constraint::Min(0)]).areas(area);

        let s = &self.setup;
        let status = self.run.as_ref().map_or("?", |r| r.status.word());
        let moves = report.moves_ratio.map_or("–".to_owned(), |m| format!("{m:.2}x"));
        let lines = vec![
            Line::raw(format!("{} (base) vs {} (fast) on {}, {} s, seed {} · {status}", s.base, s.fast, s.set, s.seconds, s.seed)),
            Line::raw(format!(
                "points   base {:+.4}   fast {:+.4}",
                report.points.0, report.points.1
            )),
            Line::raw(format!(
                "shorter than the start   base {}/{n}   fast {}/{n}",
                report.better.0,
                report.better.1,
                n = report.lines.len()
            )),
            Line::raw(format!(
                "head to head   fast shorter on {}, base on {}, tied on {} · fast made {moves} the moves (median)",
                report.ahead.1,
                report.ahead.0,
                report.lines.len() - report.ahead.0 - report.ahead.1
            )),
        ];
        frame.render_widget(Paragraph::new(lines), head);

        let rows: Vec<Row> = report
            .lines
            .iter()
            .map(|l| {
                let (mark, color) = match l.fast.cmp(&l.base) {
                    std::cmp::Ordering::Less => ("fast", Color::Green),
                    std::cmp::Ordering::Greater => ("base", Color::Red),
                    std::cmp::Ordering::Equal => ("=", Color::Gray),
                };
                let ratio = match l.moves {
                    (Some(b), Some(f)) if b > 0 => format!("{:.2}x", f as f64 / b as f64),
                    _ => String::new(),
                };
                Row::new([
                    Cell::from(l.uid.clone()),
                    Cell::from(l.start.to_string()),
                    Cell::from(l.base.to_string()),
                    Cell::from(l.fast.to_string()),
                    Cell::from(mark).style(Style::new().fg(color)),
                    Cell::from(ratio),
                ])
            })
            .collect();
        let table = Table::new(
            rows,
            [
                Constraint::Length(34),
                Constraint::Length(9),
                Constraint::Length(9),
                Constraint::Length(9),
                Constraint::Length(6),
                Constraint::Length(10),
            ],
        )
        .header(
            Row::new(["instance", "start", "base", "fast", "ahead", "moves f/b"])
                .style(Style::new().add_modifier(Modifier::BOLD)),
        )
        .row_highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol("▶ ");
        frame.render_stateful_widget(table, body, &mut self.table);
    }
}
