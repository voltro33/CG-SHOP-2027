//! Iterated local search over segment orders, port of
//! `ordering.improve_order`: nearest neighbour, a descent with 2-opt and
//! Or-opt, then kick and descend again until the kick time is up, keeping
//! the best order.

use std::time::{Duration, Instant};

use mow_core::{Budget, Closure, Order, Orderer, Segment, SolveError};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use crate::kicks::kick;
use crate::moves::{or_opt, two_opt, Touched};
use crate::nearest::nearest_neighbour;
use crate::neighbours::Neighbours;
use crate::snapshot::Snapshot;
use crate::tour::Tour;

/// Annealing temperature shrinks by this factor after every kick.
const COOLING: f64 = 0.995;

/// Looking at the clock is not free, so only every this many checks.
const CLOCK_EVERY: u64 = 256;

const DEFAULT_NEIGHBOURS: usize = 8;

/// Nearest neighbour, then local search with kicks for `kick_seconds`.
/// `anneal` above zero makes it simulated annealing, starting at that
/// fraction of the order length as temperature. `neighbours` is how many
/// nearest segments each move looks at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IteratedLocalSearch {
    pub kick_seconds: f64,
    pub anneal: f64,
    pub neighbours: usize,
}

impl Default for IteratedLocalSearch {
    fn default() -> Self {
        Self {
            kick_seconds: 3.0,
            anneal: 0.0,
            neighbours: DEFAULT_NEIGHBOURS,
        }
    }
}

/// What one search did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    /// Segments the descents looked at for a move.
    pub checks: u64,
    /// Improving moves made.
    pub moves: u64,
    pub kicks: u64,
    /// Link length of the nearest neighbour order the search started from.
    pub start_length: u64,
    pub elapsed: Duration,
}

/// The order found, and how it was found.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub order: Order,
    pub stats: Stats,
}

impl Orderer for IteratedLocalSearch {
    fn label(&self) -> String {
        let mut label = format!("ils/kick={}/anneal={}", self.kick_seconds, self.anneal);
        if self.neighbours != DEFAULT_NEIGHBOURS {
            label += &format!("/near={}", self.neighbours);
        }

        label
    }

    fn order(&self, segments: &[Segment], closure: Closure, budget: Budget) -> Result<Order, SolveError> {
        Ok(self.search(segments, closure, budget).order)
    }
}

impl IteratedLocalSearch {
    /// The best order found within the budget, with statistics. Never fails:
    /// when time runs out, the best order so far comes back.
    pub fn search(&self, segments: &[Segment], closure: Closure, budget: Budget) -> Outcome {
        let started = Instant::now();
        let start = nearest_neighbour(segments);
        let mut stats = Stats {
            start_length: start.link_length(segments, closure),
            ..Stats::default()
        };

        if segments.len() < 3 {
            stats.elapsed = started.elapsed();
            return Outcome { order: start, stats };
        }

        let neighbours = Neighbours::build(segments, self.neighbours);
        let tour = Tour::new(segments, &start, closure);
        let mut search = Search::new(tour, &neighbours, segments.len(), budget.seed);

        search.descend_all(budget.deadline);
        let stop = budget.deadline.min(Instant::now() + seconds(self.kick_seconds));
        search.iterate(stop, self.temperature(search.tour.cost));

        let visits = search.best_order();
        let stats = Stats {
            start_length: stats.start_length,
            elapsed: started.elapsed(),
            ..search.stats
        };

        let order = Order::new(visits, segments.len()).expect("the search keeps every segment once");
        Outcome { order, stats }
    }

    fn temperature(&self, cost: i64) -> f64 {
        if self.anneal.is_finite() && self.anneal > 0.0 {
            self.anneal * cost as f64
        } else {
            0.0
        }
    }
}

/// A duration from seconds, zero for anything negative or not a number.
fn seconds(value: f64) -> Duration {
    Duration::try_from_secs_f64(value.max(0.0)).unwrap_or(Duration::MAX)
}

/// The working state of one search. It owns every buffer and changes them
/// in place; nothing in it outlives the search.
struct Search<'a> {
    tour: Tour,
    neighbours: &'a Neighbours,
    segments: usize,
    rng: SmallRng,

    // the descent's stack of segments to look at, and who is on it
    queue: Vec<u32>,
    queued: Vec<bool>,

    stats: Stats,
}

impl<'a> Search<'a> {
    fn new(tour: Tour, neighbours: &'a Neighbours, segments: usize, seed: u64) -> Self {
        let stops = tour.len();

        Self {
            tour,
            neighbours,
            segments,
            rng: SmallRng::seed_from_u64(seed),
            queue: Vec::with_capacity(stops),
            queued: vec![false; stops],
            stats: Stats::default(),
        }
    }

    /// A first descent from the start order, every segment queued.
    fn descend_all(&mut self, deadline: Instant) {
        for s in 0..self.segments as u32 {
            self.enqueue(s);
        }
        self.descend(deadline);
    }

    fn enqueue(&mut self, s: u32) {
        if !self.queued[s as usize] {
            self.queued[s as usize] = true;
            self.queue.push(s);
        }
    }

    /// Applies improving moves until no queued segment has one left. A move
    /// queues the segments around it again, since their surroundings changed.
    fn descend(&mut self, deadline: Instant) {
        while let Some(s) = self.queue.pop() {
            self.queued[s as usize] = false;

            self.stats.checks += 1;
            if self.stats.checks % CLOCK_EVERY == 0 && Instant::now() >= deadline {
                self.clear_queue();
                return;
            }

            let moved = two_opt(&mut self.tour, self.neighbours, s)
                .or_else(|| or_opt(&mut self.tour, self.neighbours, s));
            if let Some(touched) = moved {
                self.stats.moves += 1;
                self.enqueue(s);
                self.enqueue_all(touched);
            }
        }
    }

    fn enqueue_all(&mut self, touched: Touched) {
        touched.ids().iter().for_each(|&id| self.enqueue(id));
    }

    fn clear_queue(&mut self) {
        while let Some(s) = self.queue.pop() {
            self.queued[s as usize] = false;
        }
    }

    /// Kick, descend, keep if not worse, else go back. With a temperature
    /// above zero a worse order is sometimes kept, with probability
    /// `exp(-worse / T)`, and `T` cools a little every kick; the best order
    /// is then remembered apart from the kept one.
    fn iterate(&mut self, stop: Instant, mut temperature: f64) {
        self.tour.journal.clear();
        let mut kept = Snapshot::of(&self.tour);
        let mut best = Snapshot::of(&self.tour);
        let mut best_is_kept = true;

        while Instant::now() < stop {
            let touched = kick(&mut self.tour, self.neighbours, self.segments, &mut self.rng);
            self.stats.kicks += 1;
            self.enqueue_all(touched);
            self.descend(stop);

            let cost = self.tour.cost;
            let new_best = cost < best.cost;
            let keep = cost <= kept.cost || self.accept_worse(cost - kept.cost, temperature);
            temperature *= COOLING;

            if !keep {
                kept.undo(&mut self.tour);
                continue;
            }

            // leaving the best order: save it before the kept one moves on
            if best_is_kept && !new_best && cost > best.cost {
                best.copy_from(&kept);
                best_is_kept = false;
            }
            kept.keep(&mut self.tour);

            if new_best {
                best.cost = cost;
                best_is_kept = true;
            }
        }

        if !best_is_kept {
            best.restore(&mut self.tour);
        }
    }

    fn accept_worse(&mut self, worse_by: i64, temperature: f64) -> bool {
        temperature > 0.0 && self.rng.gen::<f64>() < (-(worse_by as f64) / temperature).exp()
    }

    fn best_order(&self) -> Vec<mow_core::Visit> {
        debug_assert_eq!(self.tour.cost, self.tour.full_cost());
        self.tour.order(self.segments)
    }
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
