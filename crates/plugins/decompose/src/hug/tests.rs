use super::*;
use crate::testing::{horizontal, problem, rectangle, square, sweeps_every_cell, total_length};
use crate::LaneSegments;

fn hugged(strip_lanes: u32) -> HuggedLanes {
    HuggedLanes {
        axis: Axis::Horizontal,
        strip_lanes,
        segment_cost: 2.0,
        gap_factor: 0.0,
    }
}

fn plain() -> LaneSegments {
    LaneSegments {
        gap_factor: 0.0,
        ..LaneSegments::new(Axis::Horizontal)
    }
}

/// Two 12 wide blocks, three rows each, the right one a row higher.
fn step() -> Vec<(i32, i32, i32)> {
    let mut runs: Vec<(i32, i32, i32)> = (0..3).map(|y| (y, 0, 11)).collect();
    runs.extend((1..4).map(|y| (y, 12, 23)));
    runs
}

#[test]
fn a_rectangle_keeps_the_plain_lanes() {
    let p = problem(&rectangle(12, 6), &square(3));

    let segments = hugged(2).decompose(&p).unwrap();

    assert_eq!(segments, plain().decompose(&p).unwrap());
}

#[test]
fn each_block_of_a_step_gets_its_own_flush_lane() {
    let p = problem(&step(), &square(3));

    let segments = hugged(4).decompose(&p).unwrap();
    let base = plain().decompose(&p).unwrap();

    assert_eq!(segments, horizontal(&[(1, 1, 10), (2, 13, 22)]));
    assert!(sweeps_every_cell(&p, &segments));
    assert!(total_length(&segments) < total_length(&base) || segments.len() < base.len());
}

#[test]
fn one_strip_moves_the_whole_grid() {
    // five rows: the plain grid needs two lanes, so does any offset, but
    // the ends of a ragged top row decide which is cheaper
    let mut runs = rectangle(12, 3);
    runs.push((3, 0, 2));
    let p = problem(&runs, &square(3));

    let segments = hugged(0).decompose(&p).unwrap();

    assert!(sweeps_every_cell(&p, &segments));
    assert!(total_length(&segments) <= total_length(&plain().decompose(&p).unwrap()));
}

#[test]
fn vertical_lanes_mirror_back() {
    let runs: Vec<(i32, i32, i32)> = (0..24).map(|y| (y, 0, 2 + i32::from(y >= 12))).collect();
    let p = problem(&runs, &square(3));

    let segments = HuggedLanes {
        axis: Axis::Vertical,
        ..hugged(4)
    }
    .decompose(&p)
    .unwrap();

    assert!(segments.iter().all(|s| s.axis() == Axis::Vertical));
    assert!(sweeps_every_cell(&p, &segments));
}

#[test]
fn holes_and_ragged_edges_stay_covered() {
    let runs: Vec<(i32, i32, i32)> = (0..30)
        .flat_map(|y| {
            let right = 40 + (y * 7) % 11;
            match (10..18).contains(&y) {
                true => vec![(y, y % 4, 15), (y, 22, right)],
                false => vec![(y, y % 4, right)],
            }
        })
        .collect();

    for cutter in [square(3), square(5), crate::testing::x_cutter()] {
        let p = problem(&runs, &cutter);

        for strip_lanes in [0, 1, 2, 5] {
            let segments = hugged(strip_lanes).decompose(&p).unwrap();
            assert!(sweeps_every_cell(&p, &segments), "strip lanes {strip_lanes}");
        }
    }
}

#[test]
fn bad_costs_are_refused_and_labels_name_every_knob() {
    let p = problem(&rectangle(4, 4), &square(3));
    let bad = HuggedLanes {
        segment_cost: f64::NAN,
        ..hugged(2)
    };

    assert!(bad.decompose(&p).is_err());
    assert_eq!(
        hugged(2).label(),
        "hug-h/strip_lanes=2/segment_cost=2.0/gap_factor=0.0"
    );
}
