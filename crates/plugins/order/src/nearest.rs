//! The nearest neighbour order, port of `segments._nearest_neighbor`.

use mow_core::{Budget, Closure, Order, Orderer, Point, Segment, SolveError, Visit};

use crate::grid::EndGrid;

/// Greedy: from where we are, drive to the closest end of any segment not
/// yet done, then across it to its other end. Starts with segment 0 forwards.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NearestNeighbour;

impl Orderer for NearestNeighbour {
    fn label(&self) -> String {
        "nn".to_owned()
    }

    fn order(&self, segments: &[Segment], _closure: Closure, _budget: Budget) -> Result<Order, SolveError> {
        Ok(nearest_neighbour(segments))
    }
}

/// The nearest neighbour order of `segments`. Ties go as in Python: the
/// shorter drive, then a start end over a far end, then the lower index.
pub fn nearest_neighbour(segments: &[Segment]) -> Order {
    if segments.is_empty() {
        return Order::identity(0);
    }

    // end 2s is the start of segment s, 2s + 1 its end
    let ends = segments.iter().flat_map(|s| [s.start(), s.end()]).collect();
    let mut grid = EndGrid::new(ends);

    let mut visits = Vec::with_capacity(segments.len());
    let mut at = segments[0].end();
    take(&mut grid, &mut visits, 0);

    for _ in 1..segments.len() {
        let end = closest(&grid, at);
        take(&mut grid, &mut visits, end);
        at = grid.point(end ^ 1);
    }

    Order::new(visits, segments.len()).expect("each segment is taken once")
}

/// Drives the segment of `end`, entering at that end.
fn take(grid: &mut EndGrid, visits: &mut Vec<Visit>, end: u32) {
    grid.remove(end);
    grid.remove(end ^ 1);

    visits.push(Visit {
        segment: end / 2,
        reversed: end % 2 == 1,
    });
}

/// The live end closest to `from`, ranked by `(distance, side, segment)`
/// with side 0 for a start and 1 for a far end.
fn closest(grid: &EndGrid, from: Point) -> u32 {
    let center = grid.cell_of(from);
    let mut best: Option<(u64, u32, u32)> = None;

    for r in 0..=grid.last_ring(center) {
        grid.ring(center, r, |end| {
            let key = (from.l1(grid.point(end)), end % 2, end / 2);
            if best.map_or(true, |b| key < b) {
                best = Some(key);
            }
        });

        // anything in a further ring is more than r * cell away
        if let Some((distance, _, _)) = best {
            if distance <= r as u64 * grid.cell() as u64 {
                break;
            }
        }
    }

    let (_, side, segment) = best.expect("a segment is left");
    2 * segment + side
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(y: i32, from: i32, to: i32) -> Segment {
        Segment::horizontal(y, from, to).unwrap()
    }

    fn plain(order: &Order) -> Vec<(u32, bool)> {
        order.visits().iter().map(|v| (v.segment, v.reversed)).collect()
    }

    #[test]
    fn shuffled_lanes_come_back_as_a_zigzag() {
        let segments = vec![h(0, 0, 10), h(6, 0, 10), h(2, 0, 10), h(4, 0, 10)];

        let order = nearest_neighbour(&segments);

        assert_eq!(plain(&order), vec![(0, false), (2, true), (3, false), (1, true)]);
        assert_eq!(order.link_length(&segments, Closure::Open), 6);
    }

    #[test]
    fn ties_prefer_the_start_then_the_lower_index() {
        // from (10, 0): segment 1 start and segment 2 end are both 2 away
        let segments = vec![h(0, 0, 10), h(2, 10, 20), h(-2, 0, 10)];

        let order = nearest_neighbour(&segments);

        assert_eq!(plain(&order)[1], (1, false));
    }

    #[test]
    fn far_apart_segments_are_still_found() {
        let segments = vec![h(0, 0, 1), h(100_000, -50_000, -49_999), h(3, 0, 1)];

        let order = nearest_neighbour(&segments);

        assert_eq!(plain(&order), vec![(0, false), (2, true), (1, true)]);
    }

    #[test]
    fn empty_and_single() {
        assert!(nearest_neighbour(&[]).is_empty());
        assert_eq!(plain(&nearest_neighbour(&[h(1, 2, 3)])), vec![(0, false)]);
        assert_eq!(NearestNeighbour.label(), "nn");
    }
}
