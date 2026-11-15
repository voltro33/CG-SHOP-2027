//! The menus `mow` opens when run with no arguments: the score, every
//! instance with its best tour, candidates and job history, and pictures of
//! the tours in the terminal and in the browser.
//!
//! Screens ([`screens`]) handle keys and draw; anything that reads files or
//! takes a while is a [`screens::Task`] the [`app::App`] runs after a frame
//! that says it is busy.

pub mod app;
pub mod data;
pub mod history;
pub mod preview;
pub mod screens;
pub mod summary;
pub mod viewer;

use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;

pub use data::{paths_under, Paths};

/// How often the screen redraws without a key, so clocks stay current.
const TICK: Duration = Duration::from_millis(500);

/// Runs the menus until the user quits. The terminal is restored on the
/// way out, also after an error.
pub fn run(paths: Paths) -> Result<(), String> {
    let mut app = app::App::new(paths)?;

    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, &mut app);
    ratatui::restore();

    result
}

fn event_loop(terminal: &mut DefaultTerminal, app: &mut app::App) -> Result<(), String> {
    while !app.quit {
        terminal.draw(|f| app.draw(f)).map_err(|e| e.to_string())?;

        if let Some(task) = app.take_pending() {
            app.run(task);
            continue;
        }

        if !event::poll(TICK).map_err(|e| e.to_string())? {
            continue;
        }
        if let Event::Key(key) = event::read().map_err(|e| e.to_string())? {
            // Windows reports releases too
            if key.kind == KeyEventKind::Press {
                app.key(key);
            }
        }
    }

    Ok(())
}
