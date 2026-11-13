//! Test doubles: the simplest plugin that does each step's job on small
//! rectangles mowed by a one cell cutter, where anchors are cells.

use std::sync::Mutex;

use mow_core::{
    Assigner, BitGrid, Budget, Closure, Cutter, Decomposer, Event, Improver, Observer, Order, Orderer,
    Partitioner, Point, Problem, Repairer, Route, Segment, SolveError, Splitter, Visit, Walk,
};

use crate::plugins::{Partitioning, Plugins};

/// A `width x height` rectangle at the origin, one cell cutter, `k` cutters.
pub(crate) fn rectangle(width: i32, height: i32, k: usize) -> Problem {
    let cells: Vec<Point> = (0..height)
        .flat_map(|y| (0..width).map(move |x| Point::new(x, y)))
        .collect();

    problem_of(&cells, k)
}

fn problem_of(cells: &[Point], k: usize) -> Problem {
    let region = BitGrid::from_cells(cells).unwrap().unwrap();
    let cutter = BitGrid::from_cells(&[Point::new(0, 0)]).unwrap().unwrap();

    Problem::new("test", region, Cutter::new(cutter).unwrap(), k).unwrap()
}

/// One horizontal segment per region row, edge to edge.
pub(crate) struct RowLanes;

impl Decomposer for RowLanes {
    fn label(&self) -> String {
        "rows".into()
    }

    fn decompose(&self, problem: &Problem) -> Result<Vec<Segment>, SolveError> {
        let b = problem.region_bounds();
        let segments = (b.y_min..=b.y_max)
            .map(|y| Segment::horizontal(y, b.x_min, b.x_max))
            .collect::<Result<_, _>>()?;

        Ok(segments)
    }
}

/// Finds nothing to drive.
pub(crate) struct NoSegments;

impl Decomposer for NoSegments {
    fn label(&self) -> String {
        "none".into()
    }

    fn decompose(&self, _problem: &Problem) -> Result<Vec<Segment>, SolveError> {
        Ok(Vec::new())
    }
}

/// Breaks an internal invariant.
pub(crate) struct Panics;

impl Decomposer for Panics {
    fn label(&self) -> String {
        "panics".into()
    }

    fn decompose(&self, _problem: &Problem) -> Result<Vec<Segment>, SolveError> {
        panic!("broken invariant")
    }
}

/// The segments in list order, every second one reversed.
pub(crate) struct Snake;

impl Orderer for Snake {
    fn label(&self) -> String {
        "snake".into()
    }

    fn order(&self, segments: &[Segment], _closure: Closure, _budget: Budget) -> Result<Order, SolveError> {
        let visits = (0..segments.len() as u32)
            .map(|segment| Visit {
                segment,
                reversed: segment % 2 == 1,
            })
            .collect();

        Ok(Order::new(visits, segments.len())?)
    }
}

/// Cuts a walk at its corners into at most k pieces of whole edges.
pub(crate) struct AtCorners;

impl Splitter for AtCorners {
    fn label(&self) -> String {
        "corners".into()
    }

    fn split(&self, walk: &Walk, k: usize) -> Result<Vec<Walk>, SolveError> {
        let corners = walk.corners();
        let size = (corners.len() - 1).div_ceil(k.max(1)) + 1;
        let size = size.max(2);

        // consecutive pieces share their cut corner, so no position is lost
        let mut pieces = Vec::new();
        let mut first = 0;
        while first + 1 < corners.len() || pieces.is_empty() {
            let last = (first + size - 1).min(corners.len() - 1);
            pieces.push(Walk::new(corners[first..=last].to_vec())?);
            first = last;
        }

        Ok(pieces)
    }
}

/// Leaves everything as it is.
pub(crate) struct Nothing;

impl Repairer for Nothing {
    fn label(&self) -> String {
        "none".into()
    }

    fn repair_walk(&self, _problem: &Problem, walk: Walk) -> Result<Walk, SolveError> {
        Ok(walk)
    }

    fn repair(&self, _problem: &Problem, routes: Vec<Route>) -> Result<Vec<Route>, SolveError> {
        Ok(routes)
    }
}

impl Improver for Nothing {
    fn label(&self) -> String {
        "none".into()
    }

    fn improve_walk(&self, _problem: &Problem, walk: Walk, _budget: Budget) -> Result<Walk, SolveError> {
        Ok(walk)
    }

    fn improve(
        &self,
        _problem: &Problem,
        routes: Vec<Route>,
        _budget: Budget,
    ) -> Result<Vec<Route>, SolveError> {
        Ok(routes)
    }
}

/// Fails every time.
pub(crate) struct Fails;

impl Improver for Fails {
    fn label(&self) -> String {
        "fails".into()
    }

    fn improve_walk(&self, _problem: &Problem, _walk: Walk, _budget: Budget) -> Result<Walk, SolveError> {
        Err(SolveError::OutOfTime { step: "fails" })
    }

    fn improve(
        &self,
        _problem: &Problem,
        _routes: Vec<Route>,
        _budget: Budget,
    ) -> Result<Vec<Route>, SolveError> {
        Err(SolveError::OutOfTime { step: "fails" })
    }
}

/// One closed route through every segment, in the orderer's order.
pub(crate) struct OneRoute;

impl Assigner for OneRoute {
    fn label(&self) -> String {
        "one".into()
    }

    fn assign(
        &self,
        _problem: &Problem,
        segments: &[Segment],
        orderer: &dyn Orderer,
        budget: Budget,
    ) -> Result<Vec<Route>, SolveError> {
        let order = orderer.order(segments, Closure::Closed, budget)?;

        Ok(vec![Route::through(&order.waypoints(segments))?])
    }
}

/// Horizontal bands of whole rows, as many rows each as the shares say.
pub(crate) struct RowBands;

impl Partitioner for RowBands {
    fn label(&self) -> String {
        "rows".into()
    }

    fn bands(&self, problem: &Problem, shares: &[f64]) -> Result<Vec<Problem>, SolveError> {
        let b = problem.region_bounds();
        let rows = f64::from(b.y_max - b.y_min + 1);
        let total: f64 = shares.iter().sum();

        let mut bands = Vec::new();
        let mut reached = 0.0;
        let mut low = b.y_min;
        for share in shares {
            reached += share / total * rows;
            let high = b.y_min + reached.round() as i32 - 1;

            let cells: Vec<Point> = problem
                .region()
                .cells()
                .filter(|p| (low..=high).contains(&p.y))
                .collect();
            if !cells.is_empty() {
                bands.push(problem_of(&cells, 1));
            }
            low = high + 1;
        }

        Ok(bands)
    }
}

/// Gives the first band twice the share each round.
pub(crate) fn skew(shares: &[f64], _lengths: &[u64]) -> Vec<f64> {
    shares
        .iter()
        .enumerate()
        .map(|(i, share)| if i == 0 { share * 2.0 } else { *share })
        .collect()
}

/// Writes down every event's kind and step.
#[derive(Default)]
pub(crate) struct Recorder {
    pub events: Mutex<Vec<String>>,
}

impl Observer for Recorder {
    fn observe(&self, event: &Event<'_>) {
        let name = match event {
            Event::Started { .. } => "started".to_string(),
            Event::Segments { .. } => "segments".to_string(),
            Event::Ordered { .. } => "ordered".to_string(),
            Event::Walk { step, .. } => format!("walk:{step}"),
            Event::Routes { step, .. } => format!("routes:{step}"),
            Event::Finished { .. } => "finished".to_string(),
        };

        self.events.lock().unwrap().push(name);
    }
}

impl Recorder {
    pub(crate) fn names(&self) -> Vec<String> {
        self.events.lock().unwrap().clone()
    }
}

pub(crate) fn snake(_segment_count: usize) -> Box<dyn Orderer> {
    Box::new(Snake)
}

/// Every double, with the given decomposer and observer.
pub(crate) fn doubles<'a>(decomposer: &'a dyn Decomposer, observer: &'a Recorder) -> Plugins<'a> {
    Plugins {
        decomposer,
        orderer: &snake,
        splitter: &AtCorners,
        assigner: Some(&OneRoute),
        partitioning: Some(Partitioning {
            partitioner: &RowBands,
            rebalance: skew,
        }),
        repairer: &Nothing,
        improver: &Nothing,
        observer,
    }
}
