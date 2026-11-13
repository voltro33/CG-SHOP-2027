//! The order segments are driven in, and the direction of each.

use crate::error::ModelError;
use crate::geom::Point;
use crate::segment::Segment;

/// Whether the walk returns from its last segment to its first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Closure {
    Open,
    Closed,
}

/// One segment of an order: its index into the segment list, and whether it
/// is driven from `end` to `start`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Visit {
    pub segment: u32,
    pub reversed: bool,
}

/// Every segment exactly once. Built only through [`Order::new`], so an
/// `Order` is always a permutation of the segments it was made for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Order {
    visits: Vec<Visit>,
}

impl Order {
    pub fn new(visits: Vec<Visit>, segment_count: usize) -> Result<Self, ModelError> {
        let bad = ModelError::BadOrder {
            visits: visits.len(),
            segments: segment_count,
        };
        if visits.len() != segment_count {
            return Err(bad);
        }

        let mut seen = vec![false; segment_count];
        for visit in &visits {
            match seen.get_mut(visit.segment as usize) {
                Some(slot) if !*slot => *slot = true,
                _ => return Err(bad),
            }
        }

        Ok(Self { visits })
    }

    /// The segments in list order, all forwards.
    pub fn identity(segment_count: usize) -> Self {
        let visits = (0..segment_count as u32)
            .map(|segment| Visit {
                segment,
                reversed: false,
            })
            .collect();
        Self { visits }
    }

    pub fn visits(&self) -> &[Visit] {
        &self.visits
    }

    pub fn into_visits(self) -> Vec<Visit> {
        self.visits
    }

    pub fn len(&self) -> usize {
        self.visits.len()
    }

    pub fn is_empty(&self) -> bool {
        self.visits.is_empty()
    }

    /// `(entry, exit)` of each visit, lazily, borrowing both.
    pub fn ends<'a>(&'a self, segments: &'a [Segment]) -> impl Iterator<Item = (Point, Point)> + 'a {
        self.visits
            .iter()
            .map(|v| segments[v.segment as usize].ends(v.reversed))
    }

    /// The length of the links between segments, the part ordering changes.
    pub fn link_length(&self, segments: &[Segment], closure: Closure) -> u64 {
        let mut ends = self.ends(segments);
        let Some((first_entry, first_exit)) = ends.next() else {
            return 0;
        };

        let (inner, last_exit) = ends.fold((0, first_exit), |(total, exit), (entry, next_exit)| {
            (total + exit.l1(entry), next_exit)
        });

        match closure {
            Closure::Closed => inner + last_exit.l1(first_entry),
            Closure::Open => inner,
        }
    }

    /// Link length plus the length of the segments themselves.
    pub fn walk_length(&self, segments: &[Segment], closure: Closure) -> u64 {
        let driving: u64 = self
            .visits
            .iter()
            .map(|v| segments[v.segment as usize].length())
            .sum();

        driving + self.link_length(segments, closure)
    }

    /// The points the walk drives through, entry and exit of every visit.
    /// Consecutive points need not be axis parallel; [`crate::Route::through`]
    /// inserts the bends.
    pub fn waypoints(&self, segments: &[Segment]) -> Vec<Point> {
        self.ends(segments)
            .flat_map(|(entry, exit)| [entry, exit])
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lanes() -> Vec<Segment> {
        vec![
            Segment::horizontal(0, 0, 10).unwrap(),
            Segment::horizontal(3, 0, 10).unwrap(),
        ]
    }

    #[test]
    fn boustrophedon_links_are_short() {
        let segments = lanes();
        let order = Order::new(
            vec![
                Visit {
                    segment: 0,
                    reversed: false,
                },
                Visit {
                    segment: 1,
                    reversed: true,
                },
            ],
            2,
        )
        .unwrap();

        assert_eq!(order.link_length(&segments, Closure::Open), 3);
        assert_eq!(order.link_length(&segments, Closure::Closed), 6);
        assert_eq!(order.walk_length(&segments, Closure::Closed), 26);
        assert_eq!(order.waypoints(&segments)[2], Point::new(10, 3));
    }

    #[test]
    fn rejects_repeats_and_missing_segments() {
        let twice = vec![
            Visit {
                segment: 0,
                reversed: false
            };
            2
        ];

        assert!(Order::new(twice, 2).is_err());
        assert!(Order::new(
            vec![Visit {
                segment: 5,
                reversed: false
            }],
            1
        )
        .is_err());
        assert_eq!(Order::identity(3).len(), 3);
    }
}
