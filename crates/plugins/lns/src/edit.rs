//! One tour rewritten: its kept chains and the new stops in one closed
//! order, the walks between consecutive chains, and the visit counts kept
//! in step with them.
//!
//! The counts hold the chains as they were. Each walk adds its positions
//! strictly between its two ends, which belong to the chains. A walk of no
//! steps joins two chains at one point, so that point loses a visit.

use std::time::Instant;

use mow_core::{ModelError, Point, Walk};
use mow_coverage::VisitCounts;
use mow_improve::prune_open;

use crate::chains::Chain;
use crate::cycle::{Cycle, Stop};
use crate::error::LnsError;

#[derive(Debug, Clone)]
pub struct Edit {
    pub tour: usize,
    chains: Vec<Chain>,
    cycle: Cycle,
    /// The walk after each chain, in the order's chain order, once built.
    walks: Vec<Vec<Point>>,
}

impl Edit {
    /// The chains of `positions` in tour order, nothing new yet.
    pub fn new(tour: usize, positions: &[Point], chains: Vec<Chain>) -> Self {
        let stops = chains
            .iter()
            .map(|c| Stop {
                entry: c.entry(positions),
                exit: c.exit(positions),
            })
            .collect();

        Self {
            tour,
            chains,
            cycle: Cycle::new(stops),
            walks: Vec::new(),
        }
    }

    /// Puts a new stop where it costs least.
    pub fn insert(&mut self, stop: Stop) {
        self.cycle.insert(stop);
    }

    pub fn improve(&mut self) {
        self.cycle.improve();
    }

    /// Every stop end, where a repair trip may start.
    pub fn stop_ends(&self) -> impl Iterator<Item = Point> + '_ {
        self.cycle.order().iter().flat_map(|&step| {
            let (a, b) = self.cycle.ends(step);
            [a, b]
        })
    }

    /// The tour length before any walk is built: chains, new stops, links.
    pub fn planned_length(&self) -> u64 {
        let chains = self.chains.len();
        let stops: u64 = self
            .cycle
            .order()
            .iter()
            .filter(|step| step.stop >= chains)
            .map(|&step| {
                let (a, b) = self.cycle.ends(step);
                a.l1(b)
            })
            .sum();

        self.chain_steps() + stops + self.cycle.link_length()
    }

    /// The walks between chains, through the new stops in order.
    pub fn build(&mut self) -> Result<(), ModelError> {
        self.cycle.rotate_to(0);
        let chains = self.chains.len();
        let order = self.cycle.order();

        let mut walks = Vec::with_capacity(chains);
        let mut points: Vec<Point> = Vec::new();
        for (i, &step) in order.iter().enumerate() {
            let (entry, exit) = self.cycle.ends(step);
            if step.stop >= chains {
                points.extend([entry, exit]);
                continue;
            }

            if i > 0 {
                points.push(entry);
                walks.push(Walk::through(&points)?.into_corners());
            }
            points = vec![exit];
        }

        let (home, _) = self.cycle.ends(order[0]);
        points.push(home);
        walks.push(Walk::through(&points)?.into_corners());

        self.walks = walks;
        Ok(())
    }

    /// The tour length with the walks as built.
    pub fn length(&self) -> u64 {
        self.chain_steps() + self.walks.iter().map(|w| walk_length(w)).sum::<u64>()
    }

    pub fn apply(&self, counts: &mut VisitCounts) -> Result<(), LnsError> {
        for walk in &self.walks {
            match joined_at(walk) {
                Some(p) => counts.remove_positions([p])?,
                None => counts.add_positions(interior(walk))?,
            }
        }

        Ok(())
    }

    pub fn unapply(&self, counts: &mut VisitCounts) -> Result<(), LnsError> {
        for walk in &self.walks {
            match joined_at(walk) {
                Some(p) => counts.add_positions([p])?,
                None => counts.remove_positions(interior(walk))?,
            }
        }

        Ok(())
    }

    /// Cuts corners of every walk with both ends fixed, the counts applied.
    pub fn prune(&mut self, counts: &mut VisitCounts, deadline: Instant) -> Result<(), LnsError> {
        for walk in &mut self.walks {
            if walk.first() == walk.last() {
                continue;
            }

            *walk = prune_open(std::mem::take(walk), counts, deadline)?;
        }

        Ok(())
    }

    /// The new dense tour, starting with the first chain. A join drops the
    /// second copy of its point.
    pub fn dense(&self, positions: &[Point]) -> Vec<Point> {
        let mut out: Vec<Point> = Vec::with_capacity(positions.len());
        let push = |out: &mut Vec<Point>, p: Point| {
            if out.last() != Some(&p) {
                out.push(p);
            }
        };

        let steps = self.cycle.order().iter().filter(|s| s.stop < self.chains.len());
        for (step, walk) in steps.zip(&self.walks) {
            for p in self.chains[step.stop].positions(positions, step.reversed) {
                push(&mut out, p);
            }
            for p in interior(walk) {
                push(&mut out, p);
            }
        }

        if out.len() > 1 && out.first() == out.last() {
            out.pop();
        }

        out
    }

    fn chain_steps(&self) -> u64 {
        self.chains.iter().map(|c| c.steps()).sum()
    }
}

fn walk_length(corners: &[Point]) -> u64 {
    corners.windows(2).map(|w| w[0].l1(w[1])).sum()
}

/// The point two chains share when the walk between them has no steps.
fn joined_at(corners: &[Point]) -> Option<Point> {
    (walk_length(corners) == 0).then(|| corners[0])
}

/// The positions strictly between the walk's two ends.
fn interior(corners: &[Point]) -> impl Iterator<Item = Point> + '_ {
    let steps = walk_length(corners) as usize;
    let edges = corners.windows(2).flat_map(|w| {
        let (a, b) = (w[0], w[1]);
        let (dx, dy) = ((b.x - a.x).signum(), (b.y - a.y).signum());
        (1..=a.l1(b) as i32).map(move |t| a.shifted(dx * t, dy * t))
    });

    edges.take(steps.saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pts(raw: &[(i32, i32)]) -> Vec<Point> {
        raw.iter().map(|&p| p.into()).collect()
    }

    #[test]
    fn interior_drops_both_ends() {
        let walk = pts(&[(0, 0), (3, 0), (3, 1)]);
        assert_eq!(
            interior(&walk).collect::<Vec<_>>(),
            pts(&[(1, 0), (2, 0), (3, 0)])
        );

        assert_eq!(interior(&pts(&[(0, 0), (1, 0)])).count(), 0);
        assert_eq!(joined_at(&pts(&[(2, 2)])), Some(Point::new(2, 2)));
        assert_eq!(joined_at(&pts(&[(2, 2), (2, 3)])), None);
    }
}
