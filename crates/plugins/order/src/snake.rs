//! The boustrophedon order of plain lanes (port of the walk in
//! `construct/lanes.py`): no search at all.

use mow_core::{Budget, Closure, Order, Orderer, Segment, SolveError, Visit};

/// The segments in list order, every second one reversed, so lanes laid
/// out bottom to top are mowed back and forth. Ignores the closure and the
/// budget, as `lanes.py` builds its snake directly.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Snake;

impl Orderer for Snake {
    fn label(&self) -> String {
        "snake".to_owned()
    }

    fn order(&self, segments: &[Segment], _closure: Closure, _budget: Budget) -> Result<Order, SolveError> {
        Ok(snake(segments.len()))
    }
}

/// The snake over `segment_count` segments.
pub fn snake(segment_count: usize) -> Order {
    let visits = (0..segment_count as u32)
        .map(|segment| Visit {
            segment,
            reversed: segment % 2 == 1,
        })
        .collect();

    Order::new(visits, segment_count).expect("every segment once, in order")
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use mow_core::Point;

    use super::*;

    #[test]
    fn mows_back_and_forth() {
        let segments: Vec<Segment> = (0..3).map(|y| Segment::horizontal(y, 0, 4).unwrap()).collect();
        let budget = Budget::new(Duration::ZERO, 0);

        let order = Snake.order(&segments, Closure::Open, budget).unwrap();
        let expected = [(0, 0), (4, 0), (4, 1), (0, 1), (0, 2), (4, 2)].map(|(x, y)| Point::new(x, y));

        assert_eq!(order.waypoints(&segments), expected);
    }

    #[test]
    fn no_segments_is_an_empty_order() {
        assert!(snake(0).is_empty());
    }
}
