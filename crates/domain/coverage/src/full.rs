//! The full coverage check, as the official verifier does it.

use mow_core::{BitGrid, Bounds, Cutter, ModelError, Point, Problem, Route, Walk};

use crate::rows::{clear_tail, or_shifted, widen};

/// Every anchor any route passes, over [`Problem::anchor_frame`]. Each edge
/// is marked whole and clipped to the frame; a route of one corner marks it.
pub fn anchors(problem: &Problem, routes: &[Route]) -> Result<BitGrid, ModelError> {
    let frame = problem.anchor_frame().bounds;
    let mut grid = BitGrid::empty(frame)?;

    for route in routes {
        for (a, b) in route.edges() {
            mark_edge(&mut grid, frame, a, b);
        }
    }

    Ok(grid)
}

/// Every anchor any open walk passes, as [`anchors`] does for routes but
/// with no closing edge. A walk of one corner marks it.
pub fn walk_anchors(problem: &Problem, walks: &[Walk]) -> Result<BitGrid, ModelError> {
    let frame = problem.anchor_frame().bounds;
    let mut grid = BitGrid::empty(frame)?;

    for walk in walks {
        mark_edge(&mut grid, frame, walk.start(), walk.start());
        for (a, b) in walk.edges() {
            mark_edge(&mut grid, frame, a, b);
        }
    }

    Ok(grid)
}

/// Marks the axis parallel edge `a..=b`, clipped to `frame`.
fn mark_edge(grid: &mut BitGrid, frame: Bounds, a: Point, b: Point) {
    if a.y == b.y {
        grid.insert_run(a.y, a.x.min(b.x), a.x.max(b.x));
        return;
    }

    if a.x < frame.x_min || a.x > frame.x_max {
        return;
    }

    let low = a.y.min(b.y).max(frame.y_min);
    let high = a.y.max(b.y).min(frame.y_max);
    for y in low..=high {
        grid.insert(Point::new(a.x, y));
    }
}

/// The cutter rows that dilate by the same length `len`, each as
/// `(dy, low)`: the row `dy` covers `low..low + len` relative to the center.
struct RunGroup {
    len: usize,
    runs: Vec<(i32, i32)>,
}

/// The cutter's maximal row runs grouped by length, shortest first.
fn run_groups(cutter: &Cutter) -> Vec<RunGroup> {
    let mut groups: Vec<RunGroup> = Vec::new();

    for run in cutter.cells().runs() {
        let len = run.len() as usize;
        match groups.iter_mut().find(|g| g.len == len) {
            Some(group) => group.runs.push((run.y, run.x_min)),
            None => groups.push(RunGroup {
                len,
                runs: vec![(run.y, run.x_min)],
            }),
        }
    }

    groups.sort_by_key(|g| g.len);
    groups
}

/// The cells the cutter sweeps standing on every anchor of `anchors`
/// (the Minkowski sum), over the frame of the region's grid. Cells outside
/// that frame are not computed: they never matter.
///
/// Each anchor row is dilated once per distinct cutter run length, growing
/// from the shortest, and or-ed into the output row of every run of that
/// length, shifted to the run's start.
pub fn swept(problem: &Problem, anchors: &BitGrid) -> Result<BitGrid, ModelError> {
    let frame = problem.region().frame();
    let mut out = BitGrid::empty(frame)?;

    let groups = run_groups(problem.cutter());
    let origin = anchors.origin();
    let mut dilated = vec![0u64; anchors.stride()];

    for row in 0..anchors.height() {
        let source = anchors.row_words(row);
        if source.iter().all(|&w| w == 0) {
            continue;
        }

        dilated.copy_from_slice(source);
        let anchor_y = origin.y + row as i32;
        let mut window = 1;

        for group in &groups {
            widen(&mut dilated, window, group.len);
            clear_tail(&mut dilated, anchors.width());
            window = group.len;

            for &(dy, low) in &group.runs {
                let shift = i64::from(origin.x) + i64::from(low) - i64::from(frame.x_min);
                let out_row = i64::from(anchor_y) + i64::from(dy) - i64::from(frame.y_min);

                if (0..i64::from(frame.height())).contains(&out_row) {
                    or_shifted(out.row_words_mut(out_row as u32), &dilated, shift);
                }
            }
        }
    }

    let width = out.width();
    for row in 0..out.height() {
        clear_tail(out.row_words_mut(row), width);
    }

    Ok(out)
}

/// The region cells no route sweeps, over the frame of the region's grid.
pub fn uncovered(problem: &Problem, routes: &[Route]) -> Result<BitGrid, ModelError> {
    unswept(problem, &anchors(problem, routes)?)
}

/// The region cells no open walk sweeps, over the frame of the region's grid.
pub fn uncovered_walks(problem: &Problem, walks: &[Walk]) -> Result<BitGrid, ModelError> {
    unswept(problem, &walk_anchors(problem, walks)?)
}

/// The region cells the cutter misses standing on every anchor of `anchors`.
fn unswept(problem: &Problem, anchors: &BitGrid) -> Result<BitGrid, ModelError> {
    let swept = swept(problem, anchors)?;
    let mut result = problem.region().clone();

    for row in 0..result.height() {
        let mowed = swept.row_words(row);
        for (cell, &m) in result.row_words_mut(row).iter_mut().zip(mowed) {
            *cell &= !m;
        }
    }

    Ok(result)
}

pub fn uncovered_count(problem: &Problem, routes: &[Route]) -> Result<usize, ModelError> {
    let swept = swept(problem, &anchors(problem, routes)?)?;
    let region = problem.region();

    let count = (0..region.height())
        .map(|row| {
            let cells = region.row_words(row).iter().zip(swept.row_words(row));
            cells
                .map(|(&c, &m)| (c & !m).count_ones() as usize)
                .sum::<usize>()
        })
        .sum();

    Ok(count)
}

/// True if the routes together sweep every region cell.
pub fn is_covered(problem: &Problem, routes: &[Route]) -> Result<bool, ModelError> {
    Ok(uncovered_count(problem, routes)? == 0)
}
