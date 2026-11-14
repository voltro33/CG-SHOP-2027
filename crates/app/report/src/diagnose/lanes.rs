//! The waste of one tour that comes with each lane (`_lane_overheads`).

use mow_core::Point;

/// Overshoot near lane ends, short and long dead stretches, and lanes.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LaneParts {
    pub overshoot_at_lane_ends: f64,
    pub dead_short: f64,
    pub dead_travel: f64,
    pub lanes: u64,
}

impl LaneParts {
    pub fn plus(&self, other: &Self) -> Self {
        Self {
            overshoot_at_lane_ends: self.overshoot_at_lane_ends + other.overshoot_at_lane_ends,
            dead_short: self.dead_short + other.dead_short,
            dead_travel: self.dead_travel + other.dead_travel,
            lanes: self.lanes + other.lanes,
        }
    }
}

/// `new[i]` and `outside[i]` belong to the step from position `i` to the
/// next one, the last step closing the tour. Straight runs are cut where
/// the step changes; a run is a lane when one of its steps sweeps a new cell.
pub fn overheads(positions: &[Point], new: &[usize], outside: &[f64], span: i64) -> LaneParts {
    let count = positions.len();
    let step = |i: usize| {
        let (a, b) = (positions[i], positions[(i + 1) % count]);
        (b.x - a.x, b.y - a.y)
    };

    // straight runs, as (first step, length), in tour order
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for i in 0..count {
        match runs.last_mut() {
            Some((_, length)) if step(i) == step(i - 1) => *length += 1,
            _ => runs.push((i, 1)),
        }
    }

    let mut parts = LaneParts::default();
    for &(first, length) in &runs {
        let steps = first..first + length;
        if steps.clone().all(|i| new[i] == 0) {
            continue;
        }
        parts.lanes += 1;

        // where each step sits inside its run, counted from either end
        for i in steps {
            let from_start = (i - first) as i64;
            let from_end = (length - 1) as i64 - from_start;
            if from_start.min(from_end) < span {
                parts.overshoot_at_lane_ends += outside[i];
            }
        }
    }

    for stretch in dead_stretches(new) {
        let stretch = stretch as i64;
        if stretch < span {
            parts.dead_short += stretch as f64;
        }
        if stretch >= 4 * span {
            parts.dead_travel += stretch as f64;
        }
    }

    parts
}

/// The lengths of the runs of consecutive dead steps, not wrapping around.
fn dead_stretches(new: &[usize]) -> Vec<usize> {
    let mut stretches = Vec::new();
    let mut current = 0;

    for &cells in new {
        match cells {
            0 => current += 1,
            _ if current > 0 => stretches.push(std::mem::take(&mut current)),
            _ => {}
        }
    }
    if current > 0 {
        stretches.push(current);
    }

    stretches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_runs_and_dead_stretches() {
        // right 0..4 and back: two runs of four steps, the second all dead
        let positions: Vec<Point> = [0, 1, 2, 3, 4, 3, 2, 1]
            .iter()
            .map(|&x| Point::new(x, 0))
            .collect();
        let new = [1, 1, 1, 1, 0, 0, 0, 0];
        let outside = [0.5, 0.5, 0.5, 0.5, 0.0, 0.0, 0.0, 0.0];

        let parts = overheads(&positions, &new, &outside, 2);

        assert_eq!(parts.lanes, 1);
        assert_eq!(parts.overshoot_at_lane_ends, 2.0);
        assert_eq!(parts.dead_short, 0.0);
        assert_eq!(parts.dead_travel, 0.0);
        assert_eq!(dead_stretches(&[0, 1, 0, 0, 1, 0]), [1, 2, 1]);
    }
}
