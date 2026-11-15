//! The app: a stack of screens, the status line, and the tasks screens ask
//! for. A task runs after a frame that says what it is doing.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use mow_core::{Problem, Solution};

use crate::data::{Lawn, Paths};
use crate::history;
use crate::preview::Preview;
use crate::screens::{candidates::Candidates, detail::Detail, jobs::Jobs, menu::Menu};
use crate::screens::{Ctx, Nav, Screen, Task};
use crate::summary::Summary;
use crate::viewer::{self, Panel};

pub struct App {
    pub ctx: Ctx,
    stack: Vec<Screen>,
    pending: Option<Task>,
    status: String,
    pub quit: bool,
    /// The last instance loaded, kept for the next picture of it.
    problem: Option<Problem>,
}

impl App {
    pub fn new(paths: Paths) -> Result<Self, String> {
        let lawn = Lawn::load(&paths)?;
        let summary = Summary::of(&lawn);

        Ok(Self {
            ctx: Ctx { paths, lawn, summary },
            stack: vec![Screen::Menu(Menu::default())],
            pending: None,
            status: String::new(),
            quit: false,
            problem: None,
        })
    }

    pub fn key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.quit = true;
            return;
        }

        self.status.clear();
        let Some(top) = self.stack.last_mut() else {
            self.quit = true;
            return;
        };

        match top.key(key, &self.ctx) {
            Nav::Stay => {}
            Nav::Push(screen) => self.stack.push(screen),
            Nav::Pop => {
                self.stack.pop();
                self.quit = self.stack.is_empty();
            }
            Nav::Quit => self.quit = true,
            Nav::Say(text) => self.status = text,
            Nav::Run(task) => {
                self.status = task.busy();
                self.pending = Some(task);
            }
        }
    }

    /// The task to run now that its busy line is on screen.
    pub fn take_pending(&mut self) -> Option<Task> {
        self.pending.take()
    }

    pub fn run(&mut self, task: Task) {
        let result = match task {
            Task::Detail(uid) => self.open_detail(&uid),
            Task::ViewBest(uid) => self.view_best(&uid),
            Task::Compare { uid, candidate } => self.compare(&uid, &candidate),
            Task::Candidates(uid) => self.open_candidates(&uid),
            Task::Jobs(uid) => self.open_jobs(&uid),
            Task::Reload => self.reload(),
        };

        self.status = match result {
            Ok(message) => message,
            Err(message) => format!("error: {message}"),
        };
    }

    fn open_detail(&mut self, uid: &str) -> Result<String, String> {
        let (problem, best) = self.load(uid)?;
        let preview = Preview::new(problem, best.routes());
        let lengths = best.routes().iter().map(|r| r.length()).collect();

        self.stack.push(Screen::Detail(Box::new(Detail::new(
            uid.to_owned(),
            preview,
            lengths,
        ))));
        Ok(String::new())
    }

    fn view_best(&mut self, uid: &str) -> Result<String, String> {
        let (problem, best) = self.load(uid)?;
        let panel = Panel::of("best", problem, &best);

        let html = viewer::page(uid, problem.k(), &[panel]);
        self.publish(uid, &html)
    }

    fn compare(&mut self, uid: &str, candidate: &history::Candidate) -> Result<String, String> {
        let other = mow_io::load_solution(&candidate.path).map_err(|e| e.to_string())?;
        let (problem, best) = self.load(uid)?;
        let panels = [
            Panel::of("best", problem, &best),
            Panel::of(&candidate.label, problem, &other),
        ];

        let html = viewer::page(&format!("{uid}: best and a candidate"), problem.k(), &panels);
        self.publish(&format!("{uid}-vs-{}", candidate.label), &html)
    }

    fn publish(&self, name: &str, html: &str) -> Result<String, String> {
        let path = viewer::write(&self.ctx.paths.view_dir(), name, html)?;
        viewer::open(&path)?;

        Ok(format!("opened {} in the browser", path.display()))
    }

    fn open_candidates(&mut self, uid: &str) -> Result<String, String> {
        let list = history::candidates(&self.ctx.paths, uid);
        let best = self
            .ctx
            .lawn
            .find(uid)
            .and_then(|i| i.best.as_ref())
            .map(|b| b.objective);
        let count = list.len();

        self.stack
            .push(Screen::Candidates(Candidates::new(uid.to_owned(), best, list)));
        Ok(format!("{count} candidates"))
    }

    fn open_jobs(&mut self, uid: &str) -> Result<String, String> {
        let records = history::jobs(&self.ctx.paths, uid)?;
        let count = records.len();

        self.stack.push(Screen::Jobs(Jobs::new(uid.to_owned(), records)));
        Ok(format!("{count} jobs logged"))
    }

    fn reload(&mut self) -> Result<String, String> {
        self.ctx.lawn = Lawn::load(&self.ctx.paths)?;
        self.ctx.summary = Summary::of(&self.ctx.lawn);

        for screen in &mut self.stack {
            if let Screen::Instances(list) = screen {
                list.refresh(&self.ctx.lawn);
            }
        }
        Ok(format!("score {:.3}", self.ctx.summary.all.points))
    }

    /// The instance, read once and kept, and its stored best.
    fn load(&mut self, uid: &str) -> Result<(&Problem, Solution), String> {
        let best_path = self
            .ctx
            .lawn
            .find(uid)
            .and_then(|i| i.best.as_ref())
            .map(|b| b.path.clone())
            .ok_or_else(|| format!("{uid} has no stored best"))?;
        let best = mow_io::load_solution(&best_path).map_err(|e| format!("{}: {e}", best_path.display()))?;

        if self.problem.as_ref().map_or(true, |p| p.uid() != uid) {
            let path = self.ctx.paths.instance_file(uid);
            let problem = mow_io::load_problem(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            self.problem = Some(problem);
        }

        let problem = self.problem.as_ref().ok_or("the instance did not load")?;
        Ok((problem, best))
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        let [title, body, footer] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(3), Constraint::Length(2)])
                .areas(frame.area());

        let trail: Vec<String> = self.stack.iter().map(Screen::title).collect();
        let heading = Line::from(vec![
            Span::styled(
                " mow ",
                Style::new()
                    .fg(Color::Black)
                    .bg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(" {}", trail.join(" › "))),
        ]);
        frame.render_widget(Paragraph::new(heading), title);

        let hints = self.stack.last().map_or("", Screen::hints);
        if let Some(top) = self.stack.last_mut() {
            top.draw(frame, body.inner(ratatui::layout::Margin::new(1, 0)), &self.ctx);
        }

        let lines = vec![
            Line::styled(self.status.clone(), Style::new().fg(Color::Yellow)),
            Line::styled(hints, Style::new().fg(Color::DarkGray)),
        ];
        frame.render_widget(
            Paragraph::new(lines).block(Block::new().borders(Borders::NONE)),
            footer,
        );
    }
}
