//! Group first, then close: k closed tours straight from the segments (port
//! of `construct/groups.py`).
//!
//! 1. Order all segments as one walk (closed when k is 1, open otherwise).
//! 2. Cut that order into k consecutive groups, each priced with its way
//!    home, so the longest closed group is as short as possible.
//! 3. Order each group again as a closed tour, move single segments out of
//!    the longest group while that helps, and order each group once more.

use std::time::Duration;

use mow_core::{
    Assigner, Axis, Budget, Closure, Order, Orderer, Point, Problem, Route, Segment, SolveError, Visit,
};

use crate::cut::group_order;
use crate::rebalance::{closed_length, ends, rebalance, Group};

const STEP: &str = "groups";

/// The longest kick time `auto_kick_seconds` hands out.
const MAX_AUTO_KICK: f64 = 60.0;

/// Longer than any run, so a cap never overflows the clock.
const LONGEST_CAP: Duration = Duration::from_secs(365 * 24 * 3600);

/// Up to `k` closed tours through the segments, see the module docs.
///
/// `kick_seconds` is the total kick time, as in Python: the first ordering
/// gets all of it, then each group's two closing orders get its share of
/// half of it each. `None` grows it with the segment count like
/// `auto_kick_seconds`. The orderer is handed a budget capped to each share,
/// so its own kick time should be at least as long as the shares.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Groups {
    pub kick_seconds: Option<f64>,
}

impl Assigner for Groups {
    fn label(&self) -> String {
        match self.kick_seconds {
            Some(kick) => format!("{STEP}/kick_seconds={kick}"),
            None => STEP.to_owned(),
        }
    }

    fn assign(
        &self,
        problem: &Problem,
        segments: &[Segment],
        orderer: &dyn Orderer,
        budget: Budget,
    ) -> Result<Vec<Route>, SolveError> {
        if segments.is_empty() {
            return Ok(Vec::new());
        }

        let kick = self.kick(segments.len())?;
        let closure = if problem.k() == 1 {
            Closure::Closed
        } else {
            Closure::Open
        };
        let order = orderer.order(segments, closure, budget.capped(cap(kick)))?;

        let ends: Vec<(Point, Point)> = order.ends(segments).collect();
        let visits = order.visits();
        let groups: Vec<Group> = group_order(&ends, problem.k())
            .into_iter()
            .map(|(first, last)| visits[first..=last].to_vec())
            .collect();

        // the kick time is shared: each group gets its share, once before
        // rebalancing and once after
        let closer = Closer {
            segments,
            orderer,
            kick: kick / 2.0,
        };
        let groups = closer.close_all(groups, budget)?;
        let groups = rebalance(segments, groups, budget.deadline);
        let groups = closer.close_all(groups, budget.with_seed(budget.seed.wrapping_add(1)))?;

        groups.iter().map(|group| route(segments, group)).collect()
    }
}

impl Groups {
    fn kick(&self, segment_count: usize) -> Result<f64, SolveError> {
        match self.kick_seconds {
            Some(kick) if kick.is_finite() && kick >= 0.0 => Ok(kick),
            Some(kick) => Err(SolveError::Unsupported {
                step: STEP,
                reason: format!("kick_seconds must be a time, not {kick}"),
            }),
            None => Ok(auto_kick_seconds(segment_count)),
        }
    }
}

/// More segments need more kicks before the order settles.
pub fn auto_kick_seconds(segment_count: usize) -> f64 {
    MAX_AUTO_KICK.min(2.0 + segment_count as f64 / 50.0)
}

/// Orders each group as a closed tour, its kick time in proportion to its
/// size.
struct Closer<'a> {
    segments: &'a [Segment],
    orderer: &'a dyn Orderer,
    kick: f64,
}

impl Closer<'_> {
    fn close_all(&self, groups: Vec<Group>, budget: Budget) -> Result<Vec<Group>, SolveError> {
        let total: usize = groups.iter().map(Vec::len).sum();

        groups
            .into_iter()
            .map(|group| {
                let share = self.kick * group.len() as f64 / total as f64;
                self.close(group, budget.capped(cap(share)))
            })
            .collect()
    }

    /// The group reordered closed. Python's search starts from the group's
    /// own order and so never makes it longer; an orderer starts afresh, so
    /// the old order stays when the new one is not shorter.
    fn close(&self, group: Group, budget: Budget) -> Result<Group, SolveError> {
        let local: Vec<Segment> = group.iter().map(|v| self.segments[v.segment as usize]).collect();
        let order: Order = self.orderer.order(&local, Closure::Closed, budget)?;

        let reordered: Group = order
            .visits()
            .iter()
            .map(|v| Visit {
                segment: group[v.segment as usize].segment,
                reversed: v.reversed,
            })
            .collect();

        if closed_length(self.segments, &reordered) <= closed_length(self.segments, &group) {
            Ok(reordered)
        } else {
            Ok(group)
        }
    }
}

/// The closed tour through a group's segments. Python builds vertical
/// groups on the mirrored problem, so their bends go vertical first; the
/// same happens here when every segment is vertical.
fn route(segments: &[Segment], group: &[Visit]) -> Result<Route, SolveError> {
    let waypoints: Vec<Point> = group
        .iter()
        .flat_map(|&v| {
            let (entry, exit) = ends(segments, v);
            [entry, exit]
        })
        .collect();

    let vertical = group
        .iter()
        .all(|v| segments[v.segment as usize].axis() == Axis::Vertical);

    if !vertical {
        return Ok(Route::through(&waypoints)?);
    }

    let mirrored: Vec<Point> = waypoints.iter().map(|p| p.transposed()).collect();
    let corners = Route::through(&mirrored)?.into_corners();

    Ok(Route::new(corners.into_iter().map(Point::transposed).collect())?)
}

fn cap(seconds: f64) -> Duration {
    Duration::try_from_secs_f64(seconds.max(0.0))
        .unwrap_or(LONGEST_CAP)
        .min(LONGEST_CAP)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn forwards(n: u32) -> Vec<Visit> {
        (0..n)
            .map(|segment| Visit {
                segment,
                reversed: false,
            })
            .collect()
    }

    #[test]
    fn vertical_groups_bend_vertical_first_like_python() {
        // (0,4) -> (3,5): vertical first bends at (0,5), not at (3,4)
        let segments = [
            Segment::vertical(0, 0, 4).unwrap(),
            Segment::vertical(3, 5, 9).unwrap(),
        ];
        let route = route(&segments, &forwards(2)).unwrap();

        assert!(route.corners().contains(&Point::new(0, 5)));
        assert!(!route.corners().contains(&Point::new(3, 4)));
    }

    #[test]
    fn horizontal_groups_bend_horizontal_first() {
        let segments = [
            Segment::horizontal(0, 0, 4).unwrap(),
            Segment::horizontal(3, 5, 9).unwrap(),
        ];
        let route = route(&segments, &forwards(2)).unwrap();

        assert!(route.corners().contains(&Point::new(5, 0)));
    }
}
