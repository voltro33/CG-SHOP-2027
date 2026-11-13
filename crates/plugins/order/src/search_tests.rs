use std::time::{Duration, Instant};

use mow_core::{Budget, Closure, Orderer, Segment};

use super::*;

fn h(y: i32, from: i32, to: i32) -> Segment {
    Segment::horizontal(y, from, to).unwrap()
}

fn quick() -> IteratedLocalSearch {
    IteratedLocalSearch {
        kick_seconds: 0.05,
        ..IteratedLocalSearch::default()
    }
}

fn budget() -> Budget {
    Budget::new(Duration::from_secs(10), 1)
}

/// `count` lanes of one width, two apart, in a scrambled list order.
fn shuffled_lanes(count: i32) -> Vec<Segment> {
    (0..count).map(|i| h(2 * ((i * 7) % count), 0, 20)).collect()
}

/// A repeatable cloud of short segments, both axes.
fn scatter(count: i32) -> Vec<Segment> {
    (0..count)
        .map(|i| {
            let (a, b) = ((i * 7919) % 1009, (i * 104_729) % 997);
            let len = i % 5;
            if i % 2 == 0 {
                Segment::horizontal(b, a, a + len).unwrap()
            } else {
                Segment::vertical(a, b, b + len).unwrap()
            }
        })
        .collect()
}

#[test]
fn shuffled_lanes_come_back_to_the_zigzag() {
    // count must be coprime with 7 for the shuffle to be a permutation
    let segments = shuffled_lanes(30);

    for (closure, best) in [(Closure::Open, 2 * 29), (Closure::Closed, 2 * 29 + 2 * 29)] {
        let order = quick().order(&segments, closure, budget()).unwrap();

        assert_eq!(order.len(), 30);
        assert_eq!(order.link_length(&segments, closure), best, "{closure:?}");
    }
}

#[test]
fn picks_the_right_direction() {
    // forwards then forwards drives back across; the optimum reverses one
    let segments = vec![h(0, 0, 100), h(1, 0, 100), h(2, 0, 100), h(3, 0, 100)];

    let order = quick().order(&segments, Closure::Open, budget()).unwrap();
    let flips: Vec<bool> = order.visits().iter().map(|v| v.reversed).collect();

    assert_eq!(order.link_length(&segments, Closure::Open), 3);
    assert!(flips.windows(2).all(|w| w[0] != w[1]));
}

#[test]
fn never_worse_than_nearest_neighbour_and_always_valid() {
    let segments = scatter(400);

    for closure in [Closure::Open, Closure::Closed] {
        let outcome = quick().search(&segments, closure, budget());
        let length = outcome.order.link_length(&segments, closure);

        assert_eq!(outcome.order.len(), 400);
        assert!(length < outcome.stats.start_length);
        assert!(outcome.stats.kicks > 0 && outcome.stats.moves > 0);
    }
}

#[test]
fn closed_orders_pay_the_way_home() {
    let segments = scatter(200);

    let open = quick().order(&segments, Closure::Open, budget()).unwrap();
    let closed = quick().order(&segments, Closure::Closed, budget()).unwrap();

    assert!(closed.link_length(&segments, Closure::Closed) >= open.link_length(&segments, Closure::Open));
}

#[test]
fn the_same_seed_gives_the_same_order_within_a_fixed_number_of_kicks() {
    // a run cut by the clock depends on speed, the descent alone does not
    let segments = scatter(300);
    let none = IteratedLocalSearch {
        kick_seconds: 0.0,
        ..IteratedLocalSearch::default()
    };

    let a = none.order(&segments, Closure::Closed, budget()).unwrap();
    let b = none.order(&segments, Closure::Closed, budget()).unwrap();

    assert_eq!(a, b);
}

#[test]
fn an_expired_budget_still_gives_an_order() {
    let segments = scatter(2000);
    let spent = Budget::new(Duration::ZERO, 0);

    let started = Instant::now();
    let order = IteratedLocalSearch::default()
        .order(&segments, Closure::Open, spent)
        .unwrap();

    assert_eq!(order.len(), 2000);
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn the_deadline_caps_the_kick_time() {
    let segments = scatter(500);
    let long = IteratedLocalSearch {
        kick_seconds: 30.0,
        ..IteratedLocalSearch::default()
    };

    let started = Instant::now();
    long.order(
        &segments,
        Closure::Closed,
        Budget::new(Duration::from_millis(200), 0),
    )
    .unwrap();

    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn annealing_still_returns_its_best() {
    let segments = scatter(300);
    let hot = IteratedLocalSearch {
        kick_seconds: 0.1,
        anneal: 0.05,
        ..IteratedLocalSearch::default()
    };

    let outcome = hot.search(&segments, Closure::Closed, budget());

    assert!(outcome.order.link_length(&segments, Closure::Closed) < outcome.stats.start_length);
}

#[test]
fn tiny_inputs_and_odd_knobs() {
    let odd = IteratedLocalSearch {
        kick_seconds: f64::NAN,
        anneal: -1.0,
        neighbours: 0,
    };

    assert!(odd.order(&[], Closure::Closed, budget()).unwrap().is_empty());
    assert_eq!(
        odd.order(&[h(0, 0, 1)], Closure::Open, budget()).unwrap().len(),
        1
    );
    assert_eq!(
        odd.order(&scatter(10), Closure::Open, budget()).unwrap().len(),
        10
    );
    assert_eq!(IteratedLocalSearch::default().label(), "ils/kick=3/anneal=0");
    assert_eq!(odd.label(), "ils/kick=NaN/anneal=-1/near=0");
}
