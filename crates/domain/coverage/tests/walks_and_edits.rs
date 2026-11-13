//! Coverage of open walks, and the swept test after an edit is applied.

mod common;

use common::{pts, rectangle, reference_uncovered, square, Rng};
use mow_core::{Point, Route, Walk};
use mow_coverage::{uncovered_walks, walk_anchors, VisitCounts};

#[test]
fn a_walk_has_no_closing_edge() {
    let p = rectangle(9, 6, square(3), 1);
    let walk = Walk::new(pts(&[(1, 1), (7, 1), (7, 4)])).unwrap();

    let marked = walk_anchors(&p, std::slice::from_ref(&walk)).unwrap();
    assert!(!marked.contains(Point::new(4, 4)));
    assert_eq!(marked.len(), walk.positions().count());

    // the closed route would also sweep the top left block
    let left: Vec<Point> = uncovered_walks(&p, &[walk]).unwrap().cells().collect();
    let block: Vec<Point> = (3..6)
        .flat_map(|y| (0..6).map(move |x| Point::new(x, y)))
        .collect();
    assert_eq!(left, block);
}

#[test]
fn a_walk_of_one_corner_sweeps_its_footprint() {
    let p = rectangle(3, 3, square(3), 1);
    let walk = Walk::new(pts(&[(1, 1)])).unwrap();

    assert!(uncovered_walks(&p, &[walk]).unwrap().is_empty());
}

#[test]
fn all_swept_around_agrees_with_a_full_recheck() {
    let p = rectangle(12, 9, square(3), 1);
    let mut rng = Rng::new(3);

    for _ in 0..300 {
        let routes: Vec<Route> = (0..3)
            .map(|_| {
                let a = Point::new(rng.below(12) as i32, rng.below(9) as i32);
                let b = Point::new(rng.below(12) as i32, a.y);
                let c = Point::new(b.x, rng.below(9) as i32);
                Route::new(vec![a, b, c, Point::new(a.x, c.y)]).unwrap()
            })
            .collect();

        let mut counts = VisitCounts::from_routes(&p, &routes).unwrap();
        let removed: Vec<Point> = routes[0].positions().take(5).collect();
        counts.remove_positions(removed.iter().copied()).unwrap();

        // the reference: every region cell in the removed footprints swept
        let mut kept: Vec<Point> = routes.iter().flat_map(|r| r.positions()).collect();
        for r in &removed {
            let at = kept.iter().position(|k| k == r).unwrap();
            kept.remove(at);
        }
        let left = reference_uncovered(&p, &[]);
        let swept_now = |cell: &Point| {
            kept.iter()
                .any(|k| p.cutter().offsets().iter().any(|o| k.shifted(o.x, o.y) == *cell))
        };
        let expected = removed.iter().all(|r| {
            p.cutter()
                .offsets()
                .iter()
                .map(|o| r.shifted(o.x, o.y))
                .filter(|cell| left.contains(cell))
                .all(|cell| swept_now(&cell))
        });

        assert_eq!(counts.all_swept_around(removed.iter().copied()), expected);
    }
}
