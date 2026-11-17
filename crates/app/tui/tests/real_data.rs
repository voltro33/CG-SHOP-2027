//! The menus on the repository's own data, drawn off screen. Skipped when
//! `data/rust` is missing.

use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

use mow_tui::app::App;
use mow_tui::paths_under;

const INSTANCES: &str = "data/instances/challenge_v0/instances";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn press(app: &mut App, code: KeyCode) {
    app.key(KeyEvent::new(code, KeyModifiers::NONE));
    if let Some(task) = app.take_pending() {
        app.run(task);
    }
}

fn screen(app: &mut App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(150, 45)).unwrap();
    terminal.draw(|f| app.draw(f)).unwrap();

    let buffer = terminal.backend().buffer().clone();
    let width = buffer.area.width as usize;
    let cells: Vec<&str> = buffer.content.iter().map(|c| c.symbol()).collect();
    cells
        .chunks(width)
        .map(|row| row.concat())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn every_screen_draws_from_the_stored_data() {
    let paths = paths_under(&root(), INSTANCES);
    if !paths.data.join("catalog.csv").is_file() {
        eprintln!("skipped: no data/rust/catalog.csv");
        return;
    }
    let mut app = App::new(paths).unwrap();
    let show = std::env::var("MOW_TUI_SHOW").is_ok();

    let menu = screen(&mut app);
    assert!(menu.contains("Dashboard") && menu.contains("score"));

    press(&mut app, KeyCode::Enter);
    let dashboard = screen(&mut app);
    assert!(dashboard.contains("By cutter family"));
    if show {
        println!("{dashboard}");
    }

    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Enter);
    let list = screen(&mut app);
    assert!(list.contains("250 shown"));

    press(&mut app, KeyCode::Enter);
    let detail = screen(&mut app);
    assert!(
        detail.contains("lower bound") && detail.contains("Best tour"),
        "{detail}"
    );
    if show {
        println!("{list}\n{detail}");
    }

    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Enter);
    let candidates = screen(&mut app);
    assert!(candidates.contains("Candidates"));

    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Enter);
    let jobs = screen(&mut app);
    assert!(jobs.contains("Job history"));
    if show {
        println!("{candidates}\n{jobs}");
    }
}

#[test]
fn the_work_screens_draw_without_starting_anything() {
    let paths = paths_under(&root(), INSTANCES);
    if !paths.data.join("catalog.csv").is_file() {
        eprintln!("skipped: no data/rust/catalog.csv");
        return;
    }
    let mut app = App::new(paths).unwrap();
    let show = std::env::var("MOW_TUI_SHOW").is_ok();
    let down = |app: &mut App, n: usize| (0..n).for_each(|_| press(app, KeyCode::Down));

    // Runs, then its start form
    down(&mut app, 2);
    press(&mut app, KeyCode::Enter);
    app.tick();
    let runs = screen(&mut app);
    assert!(runs.contains("Start a pass"), "{runs}");
    press(&mut app, KeyCode::Enter);
    let form = screen(&mut app);
    assert!(
        form.contains("seconds per instance") && form.contains("[ Start ]"),
        "{form}"
    );
    if show {
        println!(
            "{runs}
{form}"
        );
    }
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Esc);

    // Experiments and its binaries
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Enter);
    app.tick();
    assert!(screen(&mut app).contains("New experiment"));
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Enter);
    let binaries = screen(&mut app);
    assert!(binaries.contains("Build a commit"), "{binaries}");
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Esc);

    // Bests: the folders to merge from (which rows fit depends on how many
    // data folders there are)
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Enter);
    let bests = screen(&mut app);
    assert!(
        bests.contains("Merge from") && bests.contains("folders to merge from") && bests.contains("/best"),
        "{bests}"
    );
    press(&mut app, KeyCode::Esc);

    // Submission: the zips so far
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Enter);
    let submission = screen(&mut app);
    assert!(
        submission.contains("Pack a new zip") && submission.contains(".zip"),
        "{submission}"
    );
    if show {
        println!(
            "{binaries}
{bests}
{submission}"
        );
    }
}

/// The browser page of the largest instance: how big and how slow.
#[test]
#[ignore]
fn the_largest_page_stays_small() {
    let paths = paths_under(&root(), INSTANCES);
    let uid = "smo10575_6398k_3x9";
    let started = std::time::Instant::now();

    let problem = mow_io::load_problem(&paths.instance_file(uid)).unwrap();
    let best = mow_io::load_solution(&paths.best_dir().join(format!("{uid}.solution.json"))).unwrap();
    let loaded = started.elapsed();
    let html = mow_tui::viewer::page(
        uid,
        problem.k(),
        &[mow_tui::viewer::Panel::of("best", &problem, &best)],
    );

    println!(
        "{uid}: load {loaded:.2?}, page {:.2?}, {} KB",
        started.elapsed() - loaded,
        html.len() / 1024
    );
    assert!(html.len() < 16 << 20);
}
