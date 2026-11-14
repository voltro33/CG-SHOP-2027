//! Each move on small hand made tours over a rectangle mowed by a 3 x 3
//! square: what it saves, and that the counts still match the corners.

use mow_core::{BitGrid, Bounds, Cutter, Point, Problem, Route};
use mow_coverage::{is_covered, VisitCounts};

use super::{transfer, Ctx, Move};
use crate::path::normalize;
use crate::TurnsOptions;

fn pts(raw: &[(i32, i32)]) -> Vec<Point> {
    raw.iter().map(|&p| p.into()).collect()
}

fn rectangle(width: i32, height: i32) -> Problem {
    let frame = Bounds {
        x_min: 0,
        y_min: 0,
        x_max: width - 1,
        y_max: height - 1,
    };
    let mut region = BitGrid::empty(frame).unwrap();
    for y in 0..height {
        region.insert_run(y, 0, width - 1);
    }

    let offsets: Vec<Point> = (-1..=1)
        .flat_map(|y| (-1..=1).map(move |x| Point::new(x, y)))
        .collect();
    let cutter = Cutter::new(BitGrid::from_cells(&offsets).unwrap().unwrap()).unwrap();
    Problem::new("rect", region, cutter, 1).unwrap()
}

/// Runs `mv` at every corner until it saves nothing more. Returns the
/// steps saved, after checking the counts against a fresh full count.
fn run(problem: &Problem, corners: &mut Vec<Point>, mv: Move) -> u64 {
    let route = Route::new(corners.clone()).unwrap();
    let mut counts = VisitCounts::from_routes(problem, &[route]).unwrap();
    let mut ctx = Ctx::new(&mut counts, TurnsOptions::default());

    let mut total = 0;
    let mut i = 0;
    while i < corners.len() {
        let saved = mv.try_at(corners, i, &mut ctx).unwrap();
        if saved == 0 {
            i += 1;
            continue;
        }
        normalize(corners);
        total += saved;
    }

    let route = Route::new(corners.clone()).unwrap();
    let fresh = VisitCounts::from_routes(problem, std::slice::from_ref(&route)).unwrap();
    for p in route.positions() {
        assert_eq!(counts.visits(p), fresh.visits(p), "visits of {p:?}");
    }
    assert!(counts.is_covered());
    assert!(is_covered(problem, &[route]).unwrap());

    total
}

fn length(corners: &[Point]) -> u64 {
    Route::new(corners.to_vec()).unwrap().length()
}

#[test]
fn slide_pulls_an_overshooting_turn_back() {
    let problem = rectangle(10, 6);
    let mut corners = pts(&[(1, 1), (12, 1), (12, 4), (1, 4)]);

    let saved = run(&problem, &mut corners, Move::Slide);

    assert_eq!(saved, 8);
    assert_eq!(corners, pts(&[(1, 1), (8, 1), (8, 4), (1, 4)]));
}

#[test]
fn slide_pulls_a_jogged_connector_back_whole() {
    let problem = rectangle(10, 6);
    let mut corners = pts(&[(1, 1), (12, 1), (12, 2), (13, 2), (13, 4), (1, 4)]);

    let saved = run(&problem, &mut corners, Move::Slide);

    // the whole jog moves 4 to the left, then what is left of it shortens
    assert!(saved >= 8);
    assert_eq!(length(&corners), 30 - saved);
}

#[test]
fn shortcut_drops_a_bump_the_lane_already_sweeps() {
    let problem = rectangle(10, 6);
    let mut corners = pts(&[(1, 1), (8, 1), (8, 4), (5, 4), (5, 5), (4, 5), (4, 4), (1, 4)]);

    let saved = run(&problem, &mut corners, Move::Shortcut);

    assert_eq!(saved, 2);
    assert_eq!(length(&corners), 20);
}

#[test]
fn two_opt_puts_the_lanes_in_order() {
    let problem = rectangle(10, 12);
    let lanes = pts(&[(1, 1), (8, 1), (8, 7), (1, 7), (1, 4), (8, 4), (8, 10), (1, 10)]);
    let mut corners = Route::through(&lanes).unwrap().into_corners();
    let before = length(&corners);

    let saved = run(&problem, &mut corners, Move::TwoOpt);

    assert!(saved > 0);
    assert_eq!(length(&corners), before - saved);
}

#[test]
fn or_opt_moves_a_lane_end_next_to_its_lane() {
    let problem = rectangle(10, 6);
    // lane y = 1 driven out and back, then lane y = 4 out and back: 34 steps
    let mut corners = pts(&[(1, 1), (8, 1), (1, 1), (1, 4), (8, 4), (1, 4)]);

    let saved = run(&problem, &mut corners, Move::OrOpt);

    assert!(saved >= 8);
    assert_eq!(length(&corners), 34 - saved);
}

#[test]
fn no_move_gives_up_a_needed_lane() {
    let problem = rectangle(10, 6);
    let tight = pts(&[(1, 1), (8, 1), (8, 4), (1, 4)]);

    for mv in Move::ALL {
        let mut corners = tight.clone();
        assert_eq!(run(&problem, &mut corners, mv), 0, "{}", mv.name());
    }
}

#[test]
fn transfer_hands_a_lane_to_the_idle_tour() {
    let problem = rectangle(10, 9);
    let mut busy = pts(&[(1, 1), (8, 1), (8, 4), (1, 4), (1, 7), (8, 7), (8, 1)]);
    normalize(&mut busy);
    let mut idle = pts(&[(1, 1)]);
    let before = length(&busy);

    let routes = [
        Route::new(busy.clone()).unwrap(),
        Route::new(idle.clone()).unwrap(),
    ];
    let mut counts = VisitCounts::from_routes(&problem, &routes).unwrap();
    let mut ctx = Ctx::new(&mut counts, TurnsOptions::default());

    let lowered = transfer(&mut busy, &mut idle, &mut ctx).unwrap();

    assert!(lowered > 0);
    assert_eq!(length(&busy).max(length(&idle)), before - lowered);
    let after = [Route::new(busy).unwrap(), Route::new(idle).unwrap()];
    assert!(counts.is_covered());
    assert!(is_covered(&problem, &after).unwrap());
}
