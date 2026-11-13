//! Partition first, then route: straight cuts across one axis give every
//! cutter its own band of the region (port of `partition.band_problems` and
//! `partition._rebalanced`).

use mow_core::{Axis, BitGrid, Bounds, Partitioner, Problem, Run, SolveError};

const STEP: &str = "bands";

/// Bands with roughly the given shares of the cells. `Horizontal` cuts
/// along rows into horizontal bands stacked bottom to top (Python's `"y"`),
/// `Vertical` along columns into vertical bands left to right (`"x"`).
/// Each band is a single cutter problem with the same uid and cutter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bands {
    pub axis: Axis,
}

impl Partitioner for Bands {
    fn label(&self) -> String {
        match self.axis {
            Axis::Horizontal => "bands-y".to_owned(),
            Axis::Vertical => "bands-x".to_owned(),
        }
    }

    fn bands(&self, problem: &Problem, shares: &[f64]) -> Result<Vec<Problem>, SolveError> {
        check_shares(shares)?;

        let runs: Vec<Run> = problem.region().runs().collect();
        let lines = Lines::new(self.axis, problem.region_bounds());
        let counts = lines.counts(&runs);
        let cuts = cut_lines(&counts, shares);

        let mut bands = Vec::with_capacity(shares.len());
        for pair in cuts.windows(2) {
            let (start, stop) = (pair[0], pair[1].min(counts.len()));
            if start >= stop {
                continue;
            }

            // a band can come out empty on tiny or very uneven regions
            let region = lines.crop(&runs, start, stop)?;
            if region.is_empty() {
                continue;
            }

            bands.push(Problem::new(problem.uid(), region, problem.cutter().clone(), 1)?);
        }

        Ok(bands)
    }
}

/// New shares that move work away from long tours. Tour length grows
/// roughly with area, so scaling a share by `mean / length` aims every band
/// at the mean; the square root damps the step so rounds do not overshoot.
/// A length of 0 counts as 1, as the pipeline clamps it. Extra entries of
/// the longer list are ignored.
pub fn rebalanced(shares: &[f64], lengths: &[u64]) -> Vec<f64> {
    let total: u64 = lengths.iter().sum();
    let mean = total as f64 / lengths.len() as f64;

    shares
        .iter()
        .zip(lengths)
        .map(|(&share, &length)| share * (mean / length.max(1) as f64).powf(0.5))
        .collect()
}

fn check_shares(shares: &[f64]) -> Result<(), SolveError> {
    let sum: f64 = shares.iter().sum();

    if shares.is_empty() || shares.iter().any(|s| !s.is_finite()) || sum.is_nan() || sum <= 0.0 {
        return Err(SolveError::Unsupported {
            step: STEP,
            reason: format!("shares must be finite with a positive sum, got {shares:?}"),
        });
    }

    Ok(())
}

/// Where each band starts and stops, as line indices: `0`, one cut after
/// the line where the cells so far reach each share, then the line count.
/// The same float steps as numpy's `cumsum(shares)[:-1] / sum * total` and
/// `searchsorted(side="left") + 1`.
fn cut_lines(counts: &[u64], shares: &[f64]) -> Vec<usize> {
    let mut cumulative = Vec::with_capacity(counts.len());
    let mut total = 0;
    for &count in counts {
        total += count;
        cumulative.push(total);
    }

    let sum: f64 = shares.iter().sum();
    let mut running = 0.0;
    let mut cuts = vec![0];

    for &share in &shares[..shares.len() - 1] {
        running += share;
        let target = running / sum * total as f64;
        cuts.push(cumulative.partition_point(|&c| (c as f64) < target) + 1);
    }

    cuts.push(counts.len());
    cuts
}

/// The lines bands are made of: rows for horizontal bands, columns for
/// vertical ones, over the region's cell box.
struct Lines {
    axis: Axis,
    bounds: Bounds,
}

impl Lines {
    const fn new(axis: Axis, bounds: Bounds) -> Self {
        Self { axis, bounds }
    }

    const fn first(&self) -> i32 {
        match self.axis {
            Axis::Horizontal => self.bounds.y_min,
            Axis::Vertical => self.bounds.x_min,
        }
    }

    const fn last(&self) -> i32 {
        match self.axis {
            Axis::Horizontal => self.bounds.y_max,
            Axis::Vertical => self.bounds.x_max,
        }
    }

    /// Cells per line.
    fn counts(&self, runs: &[Run]) -> Vec<u64> {
        let len = (self.last() - self.first() + 1) as usize;
        let at = |line: i32| (line - self.first()) as usize;

        if self.axis == Axis::Horizontal {
            let mut counts = vec![0u64; len];
            for run in runs {
                counts[at(run.y)] += u64::from(run.len());
            }

            return counts;
        }

        // every run adds one to each of its columns: mark where that starts
        // and stops, then add up
        let mut starts = vec![0u64; len + 1];
        let mut stops = vec![0u64; len + 1];
        for run in runs {
            starts[at(run.x_min)] += 1;
            stops[at(run.x_max) + 1] += 1;
        }

        let mut open = 0;
        (0..len)
            .map(|i| {
                open = open + starts[i] - stops[i];
                open
            })
            .collect()
    }

    /// The cells on lines `start..stop`, over the band's box.
    fn crop(&self, runs: &[Run], start: usize, stop: usize) -> Result<BitGrid, SolveError> {
        let low = self.first() + start as i32;
        let high = self.first() + stop as i32 - 1;

        let frame = match self.axis {
            Axis::Horizontal => Bounds {
                y_min: low,
                y_max: high,
                ..self.bounds
            },
            Axis::Vertical => Bounds {
                x_min: low,
                x_max: high,
                ..self.bounds
            },
        };

        let mut grid = BitGrid::empty(frame)?;
        for run in runs {
            match self.axis {
                Axis::Horizontal if (low..=high).contains(&run.y) => {
                    grid.insert_run(run.y, run.x_min, run.x_max);
                }
                Axis::Vertical if run.x_max >= low && run.x_min <= high => {
                    grid.insert_run(run.y, run.x_min.max(low), run.x_max.min(high));
                }
                _ => {}
            }
        }

        Ok(grid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mow_core::{Cutter, Point};

    fn square(side: i32) -> Problem {
        let mut region = BitGrid::empty(Bounds {
            x_min: 0,
            y_min: 0,
            x_max: side - 1,
            y_max: side - 1,
        })
        .unwrap();
        for y in 0..side {
            region.insert_run(y, 0, side - 1);
        }

        let cutter = Cutter::new(BitGrid::from_cells(&[Point::new(0, 0)]).unwrap().unwrap()).unwrap();
        Problem::new("square", region, cutter, 2).unwrap()
    }

    #[test]
    fn equal_shares_halve_a_square() {
        let problem = square(10);

        for axis in [Axis::Horizontal, Axis::Vertical] {
            let bands = Bands { axis }.bands(&problem, &[1.0, 1.0]).unwrap();

            assert_eq!(bands.len(), 2);
            assert!(bands.iter().all(|b| b.num_cells() == 50 && b.k() == 1));
        }
    }

    #[test]
    fn tiny_shares_can_vanish() {
        let bands = Bands { axis: Axis::Vertical }
            .bands(&square(4), &[3.0, 0.001, 1.0])
            .unwrap();

        assert_eq!(bands.len(), 2);
        assert_eq!(bands.iter().map(Problem::num_cells).sum::<usize>(), 16);
    }

    #[test]
    fn rejects_bad_shares() {
        let bands = Bands {
            axis: Axis::Horizontal,
        };

        assert!(bands.bands(&square(4), &[]).is_err());
        assert!(bands.bands(&square(4), &[1.0, f64::NAN]).is_err());
        assert!(bands.bands(&square(4), &[0.0, 0.0]).is_err());
        assert_eq!(bands.label(), "bands-y");
    }

    #[test]
    fn rebalanced_moves_work_off_long_tours() {
        let shares = rebalanced(&[1.0, 1.0], &[100, 400]);

        assert!((shares[0] - 2.5f64.sqrt()).abs() < 1e-12);
        assert!((shares[1] - 0.625f64.sqrt()).abs() < 1e-12);
    }
}
