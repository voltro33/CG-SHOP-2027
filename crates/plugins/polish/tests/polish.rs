//! A window re-solve keeps the tour one closed walk and the region
//! covered, and polishing never makes a tour longer. Ported from
//! `tests/solver/test_window.py` and `test_polish.py`, with fake solvers in
//! place of CP-SAT.

mod common;

use std::time::{Duration, Instant};

use common::{first_broken, hinted_values, response, HintSolver, Nothing};
use mow_core::{
    BitGrid, CpModel, CpParams, CpResponse, CpSolver, CpStatus, Cutter, Point, PortError, Problem, Route,
};
use mow_polish::{
    polish_routes, polish_tours, prepare_window, solve_window, PolishError, PolishOptions, Window,
};

/// A region of one row, five cells long, and a one cell cutter.
fn strip_problem() -> Problem {
    let row: Vec<Point> = (0..5).map(|x| Point::new(x, 0)).collect();
    let region = BitGrid::from_cells(&row).unwrap().unwrap();
    let cutter = BitGrid::from_cells(&[Point::new(0, 0)]).unwrap().unwrap();

    Problem::new("strip_1s1", region, Cutter::new(cutter).unwrap(), 1).unwrap()
}

fn loop_around(corners: &[(i32, i32)]) -> Vec<Point> {
    let corners = corners.iter().map(|&c| c.into()).collect();
    Route::new(corners).unwrap().positions().collect()
}

/// The tour runs along the row, steps outside at x = 5, and comes back two rows up.
const STRAIGHT: [(i32, i32); 4] = [(-1, 0), (5, 0), (5, 2), (-1, 2)];

/// The same, with a two step dent on the way back.
const DENTED: [(i32, i32); 8] = [(-1, 0), (5, 0), (5, 2), (3, 2), (3, 1), (2, 1), (2, 2), (-1, 2)];

const WINDOW: Window = Window::new(0, 0, 5, 3);

fn is_closed_walk(positions: &[Point]) -> bool {
    let n = positions.len();
    (0..n).all(|i| positions[i].l1(positions[(i + 1) % n]) == 1)
}

fn covers(problem: &Problem, tours: &[Vec<Point>]) -> bool {
    let routes: Vec<Route> = tours
        .iter()
        .map(|t| mow_core::Walk::from_positions(t).unwrap().closed().unwrap())
        .collect();

    mow_coverage::is_covered(problem, &routes).unwrap()
}

/// Answers with the given values, whatever the model.
struct Fixed(Vec<i64>);

impl CpSolver for Fixed {
    fn solve(&self, model: &CpModel, _: &CpParams) -> Result<CpResponse, PortError> {
        if self.0.len() != model.domains.len() {
            let objective = 0.0;
            return Ok(CpResponse {
                status: CpStatus::Optimal,
                objective,
                values: self.0.clone(),
            });
        }

        Ok(response(model, self.0.clone(), CpStatus::Optimal))
    }
}

#[test]
fn the_dent_is_removed_with_the_straight_walk() {
    let problem = strip_problem();
    let straight = loop_around(&STRAIGHT);
    let dented = loop_around(&DENTED);

    // both tours enter and leave the window at the same anchors, so their
    // models number the variables alike: the straight hint answers the dented model
    let answer = hinted_values(
        prepare_window(&problem, &[straight], 0, WINDOW)
            .unwrap()
            .model
            .model(),
    );
    let dented_model = prepare_window(&problem, std::slice::from_ref(&dented), 0, WINDOW).unwrap();
    assert_eq!(first_broken(dented_model.model.model(), &answer), None);

    let solver = Fixed(answer);
    let result = solve_window(
        &problem,
        std::slice::from_ref(&dented),
        0,
        WINDOW,
        Duration::from_secs(1),
        1,
        &solver,
    )
    .unwrap()
    .unwrap();

    // the way back takes six steps inside the window with the dent, four without
    assert_eq!((result.old_steps, result.new_steps, result.saved()), (10, 8, 2));
    assert_eq!(result.status, CpStatus::Optimal);
    assert_eq!((result.pieces, result.nodes, result.required), (2, 15, 5));

    let new_tour = result.positions.clone().unwrap();
    assert_eq!(new_tour.len(), dented.len() - 2);
    assert!(is_closed_walk(&new_tour));
    assert!(covers(&problem, &[new_tour]));
}

#[test]
fn an_optimal_window_stays_as_long() {
    let problem = strip_problem();
    let tour = loop_around(&STRAIGHT);

    let result = solve_window(
        &problem,
        std::slice::from_ref(&tour),
        0,
        WINDOW,
        Duration::from_secs(1),
        1,
        &HintSolver,
    )
    .unwrap()
    .unwrap();

    // the same walk, started at the first entry instead of position 0
    let new_tour = result.positions.clone().unwrap();
    let start = tour.iter().position(|&p| p == new_tour[0]).unwrap();
    let rotated: Vec<Point> = tour[start..].iter().chain(&tour[..start]).copied().collect();

    assert_eq!(result.saved(), 0);
    assert_eq!(new_tour, rotated);
}

#[test]
fn nothing_to_keep_or_nothing_inside_gives_no_result() {
    let problem = strip_problem();
    let tour = loop_around(&STRAIGHT);
    let solve = |window| {
        solve_window(
            &problem,
            std::slice::from_ref(&tour),
            0,
            window,
            Duration::ZERO,
            1,
            &HintSolver,
        )
    };

    assert!(solve(Window::new(-5, -5, 10, 10)).unwrap().is_none());
    assert!(solve(Window::new(20, 20, 30, 30)).unwrap().is_none());
}

#[test]
fn no_solution_keeps_the_old_length_and_a_short_answer_is_an_error() {
    let problem = strip_problem();
    let tours = [loop_around(&DENTED)];

    let result = solve_window(&problem, &tours, 0, WINDOW, Duration::ZERO, 1, &Nothing)
        .unwrap()
        .unwrap();
    assert_eq!((result.new_steps, result.positions), (result.old_steps, None));

    let short = Fixed(vec![0]);
    let error = solve_window(&problem, &tours, 0, WINDOW, Duration::ZERO, 1, &short).unwrap_err();
    assert!(matches!(error, PolishError::ShortAnswer { got: 1, .. }));
}

#[test]
fn a_one_anchor_window_has_no_grid_arcs_and_still_a_feasible_hint() {
    let problem = strip_problem();
    let tours = [loop_around(&STRAIGHT)];

    let prepared = prepare_window(&problem, &tours, 0, Window::new(2, 0, 3, 1)).unwrap();
    let model = prepared.model.model();

    assert_eq!(prepared.model.grid_arcs(), 0);
    assert_eq!(first_broken(model, &hinted_values(model)), None);
}

fn square_tours() -> (Problem, Vec<Vec<Point>>) {
    let cells: Vec<Point> = (0..12)
        .flat_map(|y| (0..12).map(move |x| Point::new(x, y)))
        .collect();
    let region = BitGrid::from_cells(&cells).unwrap().unwrap();
    let cutter = BitGrid::from_cells(&[Point::new(0, 0)]).unwrap().unwrap();
    let problem = Problem::new("square_1s1", region, Cutter::new(cutter).unwrap(), 1).unwrap();

    // a boustrophedon over the square, back home along x = -1
    let mut corners = Vec::new();
    for y in 0..12 {
        let (from, to) = if y % 2 == 0 { (0, 11) } else { (11, 0) };
        corners.extend([(from, y), (to, y)]);
    }
    corners.extend([(-1, 11), (-1, 0)]);

    (problem, vec![loop_around(&corners)])
}

#[test]
fn polishing_with_the_hint_changes_nothing_and_counts_windows() {
    let (problem, tours) = square_tours();

    for aim in [false, true] {
        let deadline = Instant::now() + Duration::from_millis(50);
        let options = PolishOptions {
            side: 5,
            aim,
            ..PolishOptions::default()
        };

        let (polished, stats) =
            polish_tours(&problem, tours.clone(), deadline, &options, &HintSolver).unwrap();
        assert_eq!(polished, tours);
        assert!(stats.passes >= 1 && stats.windows >= 1);
        assert_eq!((stats.improved, stats.saved_steps), (0, 0));
    }
}

#[test]
fn polished_routes_cover_and_are_no_longer() {
    let (problem, tours) = square_tours();
    let routes = vec![mow_core::Walk::from_positions(&tours[0])
        .unwrap()
        .closed()
        .unwrap()];
    let deadline = Instant::now() + Duration::from_millis(20);

    let (polished, _) =
        polish_routes(&problem, &routes, deadline, &PolishOptions::default(), &Nothing).unwrap();

    assert_eq!(polished, routes);
    assert!(mow_coverage::is_covered(&problem, &polished).unwrap());
}

#[test]
fn bad_input_is_an_error() {
    let (problem, tours) = square_tours();
    let deadline = Instant::now();
    let bad_side = PolishOptions {
        side: 0,
        ..PolishOptions::default()
    };

    let error = polish_tours(&problem, tours, deadline, &bad_side, &Nothing).unwrap_err();
    assert!(matches!(error, PolishError::BadSide(0)));

    let error = polish_tours(
        &problem,
        Vec::new(),
        deadline,
        &PolishOptions::default(),
        &Nothing,
    )
    .unwrap_err();
    assert!(matches!(error, PolishError::NoTours));
}

/// A solver that is gone: every solve fails.
struct Broken;

impl CpSolver for Broken {
    fn solve(&self, _: &CpModel, _: &CpParams) -> Result<CpResponse, PortError> {
        Err(PortError::NotFound("the solver".to_owned()))
    }
}

#[test]
fn a_failing_solver_costs_windows_not_the_tours() {
    let (problem, tours) = square_tours();
    let options = PolishOptions {
        side: 5,
        ..PolishOptions::default()
    };

    // a minute of deadline, but three errors in a row end the job at once
    let started = Instant::now();
    let deadline = started + Duration::from_secs(60);
    let (polished, stats) = polish_tours(&problem, tours.clone(), deadline, &options, &Broken).unwrap();

    assert_eq!(polished, tours);
    assert_eq!((stats.errors, stats.windows), (3, 0));
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn a_tour_that_fits_one_window_ends_the_job_early() {
    let (problem, tours) = square_tours();
    let options = PolishOptions {
        // a tile border through the 13 anchor tour is a 1 in 1000 chance per axis
        side: 13_000,
        ..PolishOptions::default()
    };

    let started = Instant::now();
    let deadline = started + Duration::from_secs(60);
    let (polished, stats) = polish_tours(&problem, tours.clone(), deadline, &options, &Nothing).unwrap();

    assert_eq!(polished, tours);
    assert_eq!(stats.windows, 0);
    assert!(started.elapsed() < Duration::from_secs(5));
}
