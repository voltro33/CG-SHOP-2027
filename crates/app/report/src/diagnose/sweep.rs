//! The step by step accounting of [`diagnose`].

use mow_core::{lower_bound, Point, Problem, Solution};

use super::lanes::{overheads, LaneParts};
use super::{lane_gain, lane_reference, Breakdown};
use crate::family::family;

const DIRECTIONS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

/// A box that holds every footprint of every tour and the whole region,
/// with a margin of one cell.
struct Frame {
    low: Point,
    width: usize,
    height: usize,
}

impl Frame {
    fn around(tours: &[Vec<Point>], problem: &Problem, offsets: &[Point]) -> Self {
        let region = problem.region_bounds();
        let corners = [
            Point::new(region.x_min, region.y_min),
            Point::new(region.x_max, region.y_max),
        ];
        let all = tours.iter().flatten().chain(corners.iter());

        let (low, high) = extent(all);
        let (o_low, o_high) = extent(offsets.iter());

        let low = Point::new(low.x + o_low.x - 1, low.y + o_low.y - 1);
        let high = Point::new(high.x + o_high.x + 1, high.y + o_high.y + 1);

        Self {
            low,
            width: (high.x - low.x + 1) as usize,
            height: (high.y - low.y + 1) as usize,
        }
    }

    fn index(&self, p: Point) -> usize {
        (p.y - self.low.y) as usize * self.width + (p.x - self.low.x) as usize
    }
}

/// The smallest and largest coordinates, per axis.
fn extent<'a>(points: impl Iterator<Item = &'a Point>) -> (Point, Point) {
    points.fold(
        (Point::new(i32::MAX, i32::MAX), Point::new(i32::MIN, i32::MIN)),
        |(lo, hi), p| {
            (
                Point::new(lo.x.min(p.x), lo.y.min(p.y)),
                Point::new(hi.x.max(p.x), hi.y.max(p.y)),
            )
        },
    )
}

/// Per unit direction `e`, the offsets `o` whose cell is fresh after a step
/// by `e`: `o + e` is not a cutter cell.
fn fresh_offsets(offsets: &[Point]) -> [Vec<Point>; 4] {
    let cells: std::collections::HashSet<Point> = offsets.iter().copied().collect();

    DIRECTIONS.map(|(dx, dy)| {
        offsets
            .iter()
            .copied()
            .filter(|o| !cells.contains(&o.shifted(dx, dy)))
            .collect()
    })
}

/// The cutter's own trail: cells swept fewer steps ago than the cutter is
/// long, over both axes at once as in Python.
fn span(offsets: &[Point]) -> i64 {
    let coordinates = || offsets.iter().flat_map(|o| [o.x, o.y]);
    let high = coordinates().max().unwrap_or(0);
    let low = coordinates().min().unwrap_or(0);

    i64::from(high - low) + 1
}

/// Everything the walk keeps between steps and tours.
struct Walker<'a> {
    frame: Frame,
    offsets: &'a [Point],
    fresh: [Vec<Point>; 4],
    gain: f64,
    gain_cells: usize,
    span: i64,
    in_region: Vec<bool>,
    /// The global step at which each cell was first swept, -1 for never.
    first_swept: Vec<i64>,
    clock: i64,
    parts: [f64; 6],
    lane: LaneParts,
    counts: [u64; 3],
    new_total: u64,
    start_total: u64,
}

/// What one step's fresh cells were.
struct Step {
    new: usize,
    outside: usize,
    near: usize,
    far: usize,
    fresh: usize,
}

impl Walker<'_> {
    /// The first footprint of a tour comes free, before any step.
    fn start(&mut self, at: Point) {
        for &o in self.offsets {
            let index = self.frame.index(Point::new(at.x + o.x, at.y + o.y));
            if self.in_region[index] && self.first_swept[index] < 0 {
                self.first_swept[index] = self.clock;
                self.start_total += 1;
            }
        }
    }

    fn step(&mut self, there: Point, direction: usize) -> Step {
        self.clock += 1;
        let mut step = Step {
            new: 0,
            outside: 0,
            near: 0,
            far: 0,
            fresh: self.fresh[direction].len(),
        };

        for &o in &self.fresh[direction] {
            let index = self.frame.index(Point::new(there.x + o.x, there.y + o.y));
            let seen_at = self.first_swept[index];

            match (self.in_region[index], seen_at) {
                (false, _) => step.outside += 1,
                (true, seen) if seen < 0 => {
                    self.first_swept[index] = self.clock;
                    step.new += 1;
                }
                (true, seen) if self.clock - seen <= self.span => step.near += 1,
                (true, _) => step.far += 1,
            }
        }

        step
    }

    /// Books one step into the parts; returns its overshoot.
    fn book(&mut self, step: &Step) -> f64 {
        self.new_total += step.new as u64;

        if step.new == 0 {
            self.parts[5] += 1.0;
            self.counts[0] += 1;
            return 0.0;
        }

        let outside = step.outside as f64 / self.gain;
        self.parts[0] += step.new as f64 / self.gain;
        self.parts[1] += outside;
        self.parts[2] += step.near as f64 / self.gain;
        self.parts[3] += step.far as f64 / self.gain;
        self.parts[4] += (self.gain_cells - step.fresh) as f64 / self.gain;

        let full = step.new == self.gain_cells;
        self.counts[if full { 2 } else { 1 }] += 1;

        outside
    }

    fn tour(&mut self, positions: &[Point]) -> Result<(), String> {
        self.start(positions[0]);

        let count = positions.len();
        let mut new_per_step = vec![0usize; count];
        let mut outside_per_step = vec![0.0f64; count];

        for index in 0..count {
            let here = positions[index];
            let there = positions[(index + 1) % count];
            let direction = DIRECTIONS
                .iter()
                .position(|&(dx, dy)| there.x - here.x == dx && there.y - here.y == dy)
                .ok_or_else(|| format!("{here:?} to {there:?} is not a unit step"))?;

            let step = self.step(there, direction);
            new_per_step[index] = step.new;
            outside_per_step[index] = self.book(&step);
        }

        let lane = overheads(positions, &new_per_step, &outside_per_step, self.span);
        self.lane = self.lane.plus(&lane);

        Ok(())
    }
}

/// The breakdown of one solution. Errors if a tour is empty, a step is not
/// a unit step, or the accounting does not add up (the solution does not
/// cover the region).
pub fn diagnose(problem: &Problem, solution: &Solution) -> Result<Breakdown, String> {
    let tours: Vec<Vec<Point>> = solution
        .routes()
        .iter()
        .map(|r| r.positions().collect())
        .collect();
    if tours.is_empty() || tours.iter().any(Vec::is_empty) {
        return Err("a solution needs tours with at least one position".to_owned());
    }

    let offsets = problem.cutter().offsets();
    let frame = Frame::around(&tours, problem, offsets);
    let mut in_region = vec![false; frame.width * frame.height];
    for cell in problem.region().cells() {
        in_region[frame.index(cell)] = true;
    }

    let mut walker = Walker {
        first_swept: vec![-1; in_region.len()],
        in_region,
        frame,
        offsets,
        fresh: fresh_offsets(offsets),
        gain: f64::from(problem.cutter().step_gain()),
        gain_cells: problem.cutter().step_gain() as usize,
        span: span(offsets),
        clock: 0,
        parts: [0.0; 6],
        lane: LaneParts::default(),
        counts: [0; 3],
        new_total: 0,
        start_total: 0,
    };
    for positions in &tours {
        walker.tour(positions)?;
    }

    let total_length: u64 = tours.iter().map(|t| t.len() as u64).sum();
    check_accounting(&walker, total_length, problem.num_cells())?;

    Ok(breakdown(problem, &walker, &tours, total_length))
}

fn check_accounting(walker: &Walker, total_length: u64, cells: usize) -> Result<(), String> {
    let summed = walker.parts.iter().fold(0.0, |sum, part| sum + part);
    let length = total_length as f64;
    if (summed - length).abs() > 1e-6 * length.max(1.0) {
        return Err(format!(
            "parts add up to {summed:.3}, tours are {total_length} long"
        ));
    }

    // a verified solution sweeps every region cell exactly once first
    let swept = walker.new_total + walker.start_total;
    if swept != cells as u64 {
        return Err(format!("{swept} cells swept first, region has {cells}"));
    }

    Ok(())
}

fn breakdown(problem: &Problem, walker: &Walker, tours: &[Vec<Point>], total_length: u64) -> Breakdown {
    Breakdown {
        instance: problem.uid().to_owned(),
        family: family(problem.uid()),
        k: problem.k(),
        cells: problem.num_cells(),
        gain: problem.cutter().step_gain(),
        lane_gain: lane_gain(problem),
        lower_bound: lower_bound(problem),
        lane_reference: lane_reference(problem),
        objective: tours.iter().map(|t| t.len() as u64).max().unwrap_or(0),
        total_length,
        new_cells: walker.new_total,
        start_cells: walker.start_total,
        parts: walker.parts,
        overshoot_at_lane_ends: walker.lane.overshoot_at_lane_ends,
        dead_short: walker.lane.dead_short,
        dead_travel: walker.lane.dead_travel,
        lanes: walker.lane.lanes,
        dead_steps: walker.counts[0],
        partial_steps: walker.counts[1],
        full_steps: walker.counts[2],
    }
}
