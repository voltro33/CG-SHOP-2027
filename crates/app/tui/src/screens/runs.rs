//! Runs: every background run with its progress, one run's log, and the
//! form that starts a pass. The lists read the disk again every few seconds.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Gauge, Paragraph, Row, Table, Wrap};
use ratatui::Frame;

use super::confirm::Confirm;
use super::form::{Field, Form, Outcome};
use super::{draw_with_actions, moved, step_of, Ctx, Nav, Screen, Task};
use crate::clock::{ago, span};
use crate::recipes::{self, Pass, Search, Set};
use crate::runs::{self, process, Run, Status};

/// How often a run list or a run's log is read again.
const REFRESH: Duration = Duration::from_secs(3);

/// Log lines shown per log.
const TAIL: usize = 200;

/// Every run on disk, newest first.
pub fn read_runs(ctx: &Ctx) -> Vec<Run> {
    let pids = process::running_pids();
    runs::list(&recipes::runs_dir(&ctx.paths), |pid| pids.contains(&pid))
}

/// When a list was last read, to read it again after [`REFRESH`].
#[derive(Debug, Default)]
pub struct Timer(Option<Instant>);

impl Timer {
    pub fn due(&mut self) -> bool {
        let due = self.0.map_or(true, |t| t.elapsed() >= REFRESH);
        if due {
            self.0 = Some(Instant::now());
        }
        due
    }

    pub fn reset(&mut self) {
        self.0 = None;
    }
}

#[derive(Debug, Default)]
pub struct Runs {
    pub runs: Vec<Run>,
    /// 0 is "Start a pass", then the runs.
    pub selected: usize,
    timer: Timer,
}

impl Runs {
    pub fn tick(&mut self, ctx: &Ctx) {
        if self.timer.due() {
            self.runs = read_runs(ctx);
            self.selected = self.selected.min(self.runs.len());
        }
    }

    pub fn key(&mut self, key: KeyEvent, ctx: &Ctx) -> Nav {
        if let Some(step) = step_of(key, 10) {
            self.selected = moved(self.selected, step, self.runs.len() + 1);
            return Nav::Stay;
        }

        match key.code {
            KeyCode::Enter if self.selected == 0 => Nav::Push(Screen::StartPass(StartPass::new(ctx))),
            KeyCode::Enter => match self.runs.get(self.selected - 1) {
                Some(run) => Nav::Push(Screen::RunDetail(Box::new(RunDetail::new(run.dir.clone())))),
                None => Nav::Stay,
            },
            KeyCode::Esc | KeyCode::Backspace | KeyCode::Char('q') => Nav::Pop,
            _ => Nav::Stay,
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let rows: Vec<Row> = self.runs.iter().map(run_row).collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(13),
                Constraint::Length(11),
                Constraint::Length(10),
                Constraint::Length(17),
                Constraint::Length(10),
                Constraint::Min(20),
            ],
        )
        .header(
            Row::new(["started", "kind", "status", "jobs", "points", "what"])
                .style(Style::new().add_modifier(Modifier::BOLD)),
        );
        draw_with_actions(frame, area, &["Start a pass…"], table, self.selected);
    }
}

fn run_row(run: &Run) -> Row<'static> {
    let jobs = run.progress.map_or(String::new(), |p| {
        format!("{}/{}, {} bests", p.done, p.total, p.bests)
    });
    let points = run
        .done
        .as_ref()
        .and_then(|d| d.score_after)
        .map_or(String::new(), |after| {
            format!("{:+.3}", after - run.spec.score_before)
        });

    Row::new([
        Cell::from(ago(run.spec.started)),
        Cell::from(run.spec.kind.clone()),
        Cell::from(run.status.word()).style(status_style(&run.status)),
        Cell::from(jobs),
        Cell::from(points),
        Cell::from(run.spec.title.clone()),
    ])
}

pub fn status_style(status: &Status) -> Style {
    match status {
        Status::Running => Style::new().fg(Color::Cyan),
        Status::Finished => Style::new().fg(Color::Green),
        Status::Failed(_) | Status::Lost => Style::new().fg(Color::Red),
        Status::Stopped => Style::new().fg(Color::Yellow),
    }
}

/// One run: what it is, how far it got, and the end of its logs.
#[derive(Debug)]
pub struct RunDetail {
    pub dir: PathBuf,
    pub run: Option<Run>,
    error: Option<String>,
    timer: Timer,
}

impl RunDetail {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            run: None,
            error: None,
            timer: Timer::default(),
        }
    }

    pub fn title(&self) -> String {
        self.run.as_ref().map_or("Run".to_owned(), |r| r.spec.id.clone())
    }

    pub fn tick(&mut self) {
        if !self.timer.due() {
            return;
        }

        let pids = process::running_pids();
        match Run::read(&self.dir, |pid| pids.contains(&pid)) {
            Ok(run) => self.run = Some(run),
            Err(e) => self.error = Some(e),
        }
    }

    pub fn key(&mut self, key: KeyEvent) -> Nav {
        match key.code {
            KeyCode::Char('s') => match &self.run {
                Some(run) if run.is_running() => Nav::Push(Screen::Confirm(Confirm::new(
                    vec![
                        format!("Stop {}?", run.spec.title),
                        "Every process it started is stopped. Bests it already stored stay.".to_owned(),
                    ],
                    Task::StopRun(self.dir.clone()),
                ))),
                _ => Nav::Say("it is not running".to_owned()),
            },
            KeyCode::Char('o') => Nav::Run(Task::OpenFolder(self.dir.clone())),
            KeyCode::Esc | KeyCode::Backspace | KeyCode::Char('q') => {
                self.timer.reset();
                Nav::Pop
            }
            _ => Nav::Stay,
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let Some(run) = &self.run else {
            let text = self.error.clone().unwrap_or_else(|| "reading…".to_owned());
            frame.render_widget(Paragraph::new(text), area);
            return;
        };

        let [facts, gauge, logs] =
            Layout::vertical([Constraint::Length(6), Constraint::Length(1), Constraint::Min(3)]).areas(area);

        frame.render_widget(Paragraph::new(facts_of(run)).wrap(Wrap { trim: false }), facts);

        let (ratio, label) = match run.progress {
            Some(p) => (
                p.fraction(),
                format!(
                    "{}/{} jobs, {} bests, {} failed",
                    p.done, p.total, p.bests, p.failed
                ),
            ),
            None => (0.0, "no jobs reported yet".to_owned()),
        };
        frame.render_widget(
            Gauge::default()
                .ratio(ratio)
                .label(label)
                .gauge_style(Style::new().fg(Color::Green)),
            gauge,
        );

        let paths = run.logs();
        if paths.is_empty() {
            return;
        }
        let areas =
            Layout::horizontal(vec![Constraint::Ratio(1, paths.len() as u32); paths.len()]).split(logs);
        for (path, area) in paths.iter().zip(areas.iter()) {
            let name = path
                .file_name()
                .map_or(String::new(), |n| n.to_string_lossy().into_owned());
            let height = area.height.saturating_sub(1) as usize;
            let lines: Vec<Line> = tail(path, TAIL)
                .into_iter()
                .rev()
                .take(height)
                .rev()
                .map(Line::raw)
                .collect();
            frame.render_widget(
                Paragraph::new(lines).block(Block::new().title(name).borders(Borders::TOP)),
                *area,
            );
        }
    }
}

fn facts_of(run: &Run) -> Vec<Line<'static>> {
    let elapsed = match &run.done {
        Some(d) => format!("took {}", span(d.finished.saturating_sub(run.spec.started))),
        None => format!(
            "running for {}",
            span(crate::clock::now().saturating_sub(run.spec.started))
        ),
    };
    let score = match run.done.as_ref().and_then(|d| d.score_after) {
        Some(after) => format!(
            "score {:.3} → {after:.3} ({:+.3})",
            run.spec.score_before,
            after - run.spec.score_before
        ),
        None => format!("score {:.3} at the start", run.spec.score_before),
    };

    let mut lines = vec![
        Line::from(vec![Span::styled(
            run.spec.title.clone(),
            Style::new().add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled(run.status.word(), status_style(&run.status)),
            Span::raw(format!(" · started {} · {elapsed}", ago(run.spec.started))),
        ]),
        Line::raw(score),
        Line::styled(run.dir.display().to_string(), Style::new().fg(Color::DarkGray)),
    ];
    if let Status::Failed(why) = &run.status {
        lines.push(Line::styled(
            format!("failed: {why}"),
            Style::new().fg(Color::Red),
        ));
    }
    if run.status == Status::Lost {
        lines.push(Line::styled(
            "its supervisor is gone without a report: the machine restarted, or it was killed",
            Style::new().fg(Color::Red),
        ));
    }

    lines
}

/// The last `count` lines of a log, carriage returns dropped.
fn tail(path: &std::path::Path, count: usize) -> Vec<String> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();

    lines[lines.len().saturating_sub(count)..]
        .iter()
        .map(|l| l.trim_end_matches('\r').to_owned())
        .collect()
}

/// The form that starts a pass of a search.
#[derive(Debug)]
pub struct StartPass {
    pub form: Form,
    sets: Vec<Set>,
    next_pass: u32,
    running: usize,
}

const STEP: &str = "step";
const INSTANCES: &str = "instances";
const SECONDS: &str = "seconds per instance";
const THREADS: &str = "threads";
const PASS: &str = "lns pass number";

const SECOND_CHOICES: [u32; 8] = [30, 60, 120, 300, 600, 1200, 1800, 3600];

impl StartPass {
    pub fn new(ctx: &Ctx) -> Self {
        let mut sets = vec![
            Set::All,
            Set::Bench,
            Set::Cutters(1),
            Set::Cutters(2),
            Set::Cutters(3),
        ];
        let mut families: Vec<&'static str> = ctx.lawn.instances.iter().map(|i| i.family).collect();
        families.sort_unstable();
        families.dedup();
        sets.extend(families.into_iter().map(Set::Family));

        let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
        let next_pass = recipes::next_lns_pass(&ctx.paths);

        let fields = vec![
            Field::new(STEP, Search::ALL.iter().map(|s| s.name().to_owned()).collect(), 0),
            Field::new(INSTANCES, sets.iter().map(Set::name).collect(), 0),
            Field::new(SECONDS, SECOND_CHOICES.iter().map(|s| s.to_string()).collect(), 3),
            Field::new(
                THREADS,
                (1..=cores).map(|t| t.to_string()).collect(),
                cores.saturating_sub(4).max(1) - 1,
            ),
            Field::new(
                PASS,
                (next_pass..next_pass + 5).map(|p| p.to_string()).collect(),
                0,
            ),
        ];

        Self {
            form: Form::new(fields, "Start"),
            sets,
            next_pass,
            running: read_runs(ctx).iter().filter(|r| r.is_running()).count(),
        }
    }

    fn pass(&self) -> Pass {
        Pass {
            search: Search::ALL[self.form.index(STEP)],
            set: self.sets[self.form.index(INSTANCES)].clone(),
            seconds: SECOND_CHOICES[self.form.index(SECONDS)],
            threads: self.form.index(THREADS) + 1,
            pass: self.next_pass + self.form.index(PASS) as u32,
        }
    }

    pub fn key(&mut self, key: KeyEvent, ctx: &Ctx) -> Nav {
        match self.form.key(key) {
            Outcome::Submit => match recipes::pass_spec(&ctx.paths, &ctx.lawn, &self.pass()) {
                Ok(spec) => Nav::PopAndRun(Task::StartRun(Box::new(spec))),
                Err(e) => Nav::Say(e),
            },
            Outcome::Cancel => Nav::Pop,
            Outcome::Changed | Outcome::Nothing => Nav::Stay,
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect, ctx: &Ctx) {
        frame.render_widget(Paragraph::new("Start a pass"), area);
        let body = Rect {
            y: area.y + 2,
            height: area.height.saturating_sub(2),
            ..area
        };
        self.form.draw(frame, body, &self.note(ctx));
    }

    /// What Start will do, and anything to know first.
    fn note(&self, ctx: &Ctx) -> Vec<Line<'static>> {
        let pass = self.pass();
        let count = pass.set.uids(&ctx.paths, &ctx.lawn).map_or(0, |u| u.len());
        let hours = count as f64 * f64::from(pass.seconds) / pass.threads as f64 / 3600.0;

        let mut lines = vec![Line::raw(format!(
            "{} on {} ({count} instances), about {hours:.1} h on {} threads. It keeps running when the menus close.",
            pass.search.name(),
            pass.set.name(),
            pass.threads
        ))];
        if pass.search == Search::Lns && pass.set == Set::All {
            lines.push(Line::raw(format!(
                "Pass {} is a new set of jobs; a pass number already logged only runs what it missed.",
                pass.pass
            )));
        }
        if pass.search != Search::Lns && pass.set == Set::All {
            let logged = recipes::logged_jobs(&ctx.paths, pass.search);
            lines.push(Line::styled(
                format!(
                    "{logged} {} jobs are logged already; those instances are skipped.",
                    pass.search.name()
                ),
                Style::new().fg(Color::Yellow),
            ));
        }
        if self.running > 0 {
            lines.push(Line::styled(
                format!(
                    "{} run(s) going already: they will share the processor.",
                    self.running
                ),
                Style::new().fg(Color::Yellow),
            ));
        }

        lines
    }
}
