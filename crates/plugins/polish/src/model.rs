//! The CP-SAT model of one window, as `_WindowModel` in `solver/window.py`.
//!
//! Variables and constraints are made in the order Python makes them, so
//! the two models can be compared one constraint at a time. Python's
//! `new_constant(1)` returns one shared variable however often it is
//! called; here it is made once, at the first port arc, where Python makes it.

use mow_core::{CpModel, Linear, Lit, Point, Problem, Var};

use crate::window::{Piece, Window};

/// The four unit steps, in Python's order.
const STEPS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

/// How often one directed step may be taken.
pub const MAX_USES: i64 = 2;

/// Grid nodes are numbered row by row, then two port nodes per piece.
#[derive(Debug, Clone)]
pub struct WindowModel {
    pub(crate) window: Window,
    pub(crate) model: CpModel,
    pub(crate) grid_nodes: usize,
    /// As `(tail, head)`, grid steps first, then three per piece.
    pub(crate) arcs: Vec<(usize, usize)>,
    pub(crate) uses: Vec<Var>,
    pub(crate) grid_arcs: usize,
    /// `(in, out)` port nodes of every piece.
    pub(crate) ports: Vec<(usize, usize)>,
    pub(crate) visited: Vec<Var>,
    pub(crate) flows: Vec<Var>,
}

impl WindowModel {
    /// The model of `window`. `pieces` come from [`crate::cut_pieces`] on
    /// `positions` and must not be empty; `required` from
    /// [`crate::required_cells`].
    pub fn new(
        problem: &Problem,
        window: Window,
        positions: &[Point],
        pieces: &[Piece],
        required: &[Point],
    ) -> Self {
        let grid_nodes = window.nodes();

        let mut this = Self {
            window,
            model: CpModel::new(),
            grid_nodes,
            arcs: Vec::new(),
            uses: Vec::new(),
            grid_arcs: 0,
            ports: Vec::new(),
            visited: Vec::new(),
            flows: Vec::new(),
        };

        this.add_grid_arcs();
        this.add_port_arcs(positions, pieces);

        this.balance_and_connect(grid_nodes + 2 * pieces.len());
        this.cover(problem, required);

        let objective = Linear::sum(this.uses[..this.grid_arcs].iter().copied());
        this.model.minimize(objective);

        this
    }

    pub fn model(&self) -> &CpModel {
        &self.model
    }

    pub fn into_model(self) -> CpModel {
        self.model
    }

    /// How often each arc is taken, grid arcs first. The grid arcs are the
    /// first variables of the model, in this order.
    pub fn uses(&self) -> &[Var] {
        &self.uses
    }

    pub fn grid_arcs(&self) -> usize {
        self.grid_arcs
    }

    pub(crate) fn point(&self, node: usize) -> Point {
        let width = self.window.width() as usize;
        let (row, column) = (node / width, node % width);

        Point::new(self.window.x0 + column as i32, self.window.y0 + row as i32)
    }

    pub(crate) fn node(&self, p: Point) -> Option<usize> {
        self.window
            .contains(p)
            .then(|| ((p.y - self.window.y0) * self.window.width() + p.x - self.window.x0) as usize)
    }

    fn add_grid_arcs(&mut self) {
        for node in 0..self.grid_nodes {
            let here = self.point(node);

            for (dx, dy) in STEPS {
                let Some(neighbour) = self.node(here.shifted(dx, dy)) else {
                    continue;
                };

                self.arcs.push((node, neighbour));
                let uses = self.model.new_int(0, MAX_USES);
                self.uses.push(uses);
            }
        }

        self.grid_arcs = self.arcs.len();
    }

    /// Port arcs, all taken exactly once.
    fn add_port_arcs(&mut self, positions: &[Point], pieces: &[Piece]) {
        let one = self.model.new_constant(1);

        for (index, piece) in pieces.iter().enumerate() {
            let port_in = self.grid_nodes + 2 * index;
            let port_out = port_in + 1;
            let next_in = self.grid_nodes + 2 * ((index + 1) % pieces.len());

            let entry = self
                .node(positions[piece.first])
                .expect("a piece starts in the window");
            let exit = self
                .node(positions[piece.last])
                .expect("a piece ends in the window");

            for arc in [(port_in, entry), (exit, port_out), (port_out, next_in)] {
                self.arcs.push(arc);
                self.uses.push(one);
            }

            self.ports.push((port_in, port_out));
        }
    }

    fn balance_and_connect(&mut self, node_count: usize) {
        let mut incoming = vec![Vec::new(); node_count];
        let mut outgoing = vec![Vec::new(); node_count];
        for (index, &(tail, head)) in self.arcs.iter().enumerate() {
            outgoing[tail].push(index);
            incoming[head].push(index);
        }

        // a grid node is visited if the walk steps onto it
        for node in 0..self.grid_nodes {
            let arriving: Vec<(Var, i64)> = incoming[node].iter().map(|&i| (self.uses[i], 1)).collect();
            let leaving = outgoing[node].iter().map(|&i| (self.uses[i], -1));

            let balance = arriving.iter().copied().chain(leaving).collect();
            self.model.add_linear(Linear::new(balance), 0, 0);

            let visited = self.model.new_bool();
            let limit = MAX_USES * incoming[node].len() as i64;

            self.model
                .add_linear(Linear::new(arriving.clone()), 1, i64::MAX)
                .only_enforce_if(visited);

            let mut capped = arriving;
            capped.push((visited, -limit));
            self.model.add_linear(Linear::new(capped), i64::MIN, 0);

            self.visited.push(visited);
        }

        self.connect(node_count, &incoming, &outgoing);
    }

    /// One unit of flow from the first port to every other visited node.
    fn connect(&mut self, node_count: usize, incoming: &[Vec<usize>], outgoing: &[Vec<usize>]) {
        let root = self.grid_nodes;
        let total = node_count as i64;

        for index in 0..self.arcs.len() {
            // an arc taken twice can carry flow on both passes
            let flow = self.model.new_int(0, MAX_USES * total);
            let capped = vec![(flow, 1), (self.uses[index], -total)];
            self.model.add_linear(Linear::new(capped), i64::MIN, 0);

            self.flows.push(flow);
        }

        for node in (0..node_count).filter(|&node| node != root) {
            let inflow = incoming[node].iter().map(|&i| (self.flows[i], 1));
            let outflow = outgoing[node].iter().map(|&i| (self.flows[i], -1));
            let mut terms: Vec<(Var, i64)> = inflow.chain(outflow).collect();

            // a grid node takes one unit if visited, a port node always one
            if node < self.grid_nodes {
                terms.push((self.visited[node], -1));
                self.model.add_linear(Linear::new(terms), 0, 0);
            } else {
                self.model.add_linear(Linear::new(terms), 1, 1);
            }
        }
    }

    fn cover(&mut self, problem: &Problem, required: &[Point]) {
        for &cell in required {
            let sweepers = problem
                .cutter()
                .offsets()
                .iter()
                .filter_map(|o| self.node(cell.shifted(-o.x, -o.y)))
                .map(|node| Lit::from(self.visited[node]))
                .collect();

            self.model.add_bool_or(sweepers);
        }
    }

    /// No more steps inside the window than the old pieces take.
    pub fn limit_steps(&mut self, old_steps: u64) {
        let old = i64::try_from(old_steps).unwrap_or(i64::MAX);

        // with no grid arc Python's `sum([]) <= old` is plain `True`, which
        // `cp_model` adds as `bool_or` of the constant one
        if self.grid_arcs == 0 {
            let one = self.uses[0];
            self.model.add_bool_or(vec![Lit::from(one)]);
            return;
        }

        let steps = Linear::sum(self.uses[..self.grid_arcs].iter().copied());
        self.model.add_linear(steps, i64::MIN, old);
    }
}
