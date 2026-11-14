//! The two ends of a solve: the old pieces as a hint going in, and the new
//! tour spliced together from the answer coming out (`add_hint`, `splice`
//! and `_euler_walk` in `solver/window.py`).

use std::collections::HashMap;

use mow_core::{CpResponse, Point};

use crate::error::PolishError;
use crate::model::WindowModel;
use crate::window::Piece;

impl WindowModel {
    /// The current pieces as a complete starting solution, flows included,
    /// so the solver holds a feasible walk from the first moment.
    pub fn add_hint(&mut self, positions: &[Point], pieces: &[Piece]) {
        let walk = self.old_walk(positions, pieces);

        let mut visited = vec![0; self.grid_nodes];
        for &node in walk.iter().filter(|&&node| node < self.grid_nodes) {
            visited[node] = 1;
        }

        // walk the old tour once, dropping one unit of flow at every first visit
        let index_of: HashMap<(usize, usize), usize> = self
            .arcs
            .iter()
            .enumerate()
            .map(|(index, &arc)| (arc, index))
            .collect();

        let mut taken = vec![0; self.arcs.len()];
        let mut flow = vec![0; self.arcs.len()];
        let mut remaining = visited.iter().sum::<i64>() + 2 * pieces.len() as i64 - 1;
        let mut seen = vec![false; self.grid_nodes + 2 * pieces.len()];
        seen[walk[0]] = true;

        for pair in walk.windows(2) {
            let index = index_of[&(pair[0], pair[1])];
            taken[index] += 1;
            flow[index] += remaining;

            if !seen[pair[1]] {
                seen[pair[1]] = true;
                remaining -= 1;
            }
        }

        let grid_uses = self.uses[..self.grid_arcs].iter().zip(&taken);
        let hints = grid_uses
            .chain(self.flows.iter().zip(&flow))
            .chain(self.visited.iter().zip(&visited));

        for (&var, &value) in hints {
            self.model.add_hint(var, value);
        }
    }

    /// The old walk as a node sequence through the ports, back to the first.
    fn old_walk(&self, positions: &[Point], pieces: &[Piece]) -> Vec<usize> {
        let count = positions.len();
        let mut walk = Vec::new();

        for (piece, &(port_in, port_out)) in pieces.iter().zip(&self.ports) {
            walk.push(port_in);

            for offset in 0..=piece.steps(count) {
                let here = positions[(piece.first + offset) % count];
                walk.push(self.node(here).expect("a piece stays in the window"));
            }

            walk.push(port_out);
        }

        walk.push(self.ports[0].0);
        walk
    }

    /// The new dense closed tour: an Euler walk over the steps `response`
    /// chose, with the old outside run put in after every exit port.
    pub fn splice(
        &self,
        response: &CpResponse,
        positions: &[Point],
        pieces: &[Piece],
    ) -> Result<Vec<Point>, PolishError> {
        let expected = self.model.domains.len();
        if response.values.len() != expected {
            return Err(PolishError::ShortAnswer {
                got: response.values.len(),
                expected,
            });
        }

        let node_count = self.grid_nodes + 2 * pieces.len();
        let mut remaining = vec![Vec::new(); node_count];
        let mut arcs = 0;
        for (index, &(tail, head)) in self.arcs.iter().enumerate() {
            let uses = response.values[self.uses[index].0 as usize].max(0);

            for _ in 0..uses {
                remaining[tail].push(head);
                arcs += 1;
            }
        }

        // Python trusts the answer; a walk missing arcs would be a broken tour
        let walk = euler_walk(remaining, self.grid_nodes);
        if walk.len() != arcs + 1 {
            return Err(PolishError::NotOneWalk);
        }

        let outside_after = self.outside_runs(positions.len(), pieces);

        let mut result = Vec::with_capacity(positions.len());
        for &node in &walk[..walk.len() - 1] {
            if node < self.grid_nodes {
                result.push(self.point(node));
            } else if let Some(&(from, run)) = outside_after.get(&node) {
                let count = positions.len();
                result.extend((0..run).map(|t| positions[(from + t) % count]));
            }
        }

        Ok(result)
    }

    /// For every exit port: where the old outside run after its piece
    /// starts, and how many positions it has.
    fn outside_runs(&self, count: usize, pieces: &[Piece]) -> HashMap<usize, (usize, usize)> {
        let mut runs = HashMap::new();

        for (index, piece) in pieces.iter().enumerate() {
            let next_first = pieces[(index + 1) % pieces.len()].first;
            let run = ((next_first + count - piece.last) % count).saturating_sub(1);

            runs.insert(self.ports[index].1, ((piece.last + 1) % count, run));
        }

        runs
    }
}

/// Hierholzer's algorithm on a balanced, connected directed multigraph
/// given as the heads left to take from every node. The walk starts and ends
/// at `start`. Heads are taken from the back, as Python's `list.pop`.
pub(crate) fn euler_walk(mut remaining: Vec<Vec<usize>>, start: usize) -> Vec<usize> {
    let mut stack = vec![start];
    let mut walk = Vec::new();

    while let Some(&node) = stack.last() {
        match remaining.get_mut(node).and_then(Vec::pop) {
            Some(head) => stack.push(head),
            None => walk.extend(stack.pop()),
        }
    }

    walk.reverse();
    walk
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn euler_walk_takes_every_arc_once() {
        // two loops through node 0: 0 -> 1 -> 0 and 0 -> 2 -> 0
        let remaining = vec![vec![1, 2], vec![0], vec![0]];
        let walk = euler_walk(remaining, 0);

        assert_eq!(walk, vec![0, 2, 0, 1, 0]);
    }

    #[test]
    fn euler_walk_of_nothing_is_the_start() {
        assert_eq!(euler_walk(vec![Vec::new()], 0), vec![0]);
    }
}
