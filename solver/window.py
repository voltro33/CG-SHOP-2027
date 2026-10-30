"""
Re-solve one window of a finished tour with CP-SAT, keeping the rest.

The tour inside a square window of anchors falls apart into pieces, each a
run of positions from an entry to an exit. Outside the window nothing
changes: after the exit of piece `i` the tour runs outside and comes back at
the entry of piece `i + 1`. The model finds new ways through the window:

- one integer per directed unit step between neighbouring anchors in the
  window, how often the new walk takes it (0, 1 or 2, so a walk can go into
  a dead end and back out)
- two port nodes per piece: `in_i` steps onto entry `i`, exit `i` steps onto
  `out_i`, and a fixed arc `out_i -> in_{i+1}` stands for the tour outside
- every node has as many steps in as out, and a flow from the first port
  reaches every node the walk visits, so the steps form one closed walk
- every region cell that nothing outside the cut pieces sweeps has a visited
  anchor among `cell - offsets`
- minimize the number of steps inside the window

Any such walk splices back into one closed tour: it leaves each `out_i`
along the old outside run of piece `i`, which ends next to entry `i + 1`.
The walk may join an entry to another piece's exit, so the outside runs can
be reordered. The current pieces are given as a hint and the old length as
an upper bound, so a result is never longer than what was there.
"""

import time
from dataclasses import dataclass
from itertools import pairwise

import numpy as np
from ortools.sat.python import cp_model

from .model import Problem

__all__ = ["Piece", "Window", "WindowResult", "cut_pieces", "solve_window"]

STEPS = ((1, 0), (-1, 0), (0, 1), (0, -1))

# how often one directed step may be taken
MAX_USES = 2


@dataclass(frozen=True)
class Window:
    """
    Anchors with `x0 <= x < x1` and `y0 <= y < y1`.
    """

    x0: int
    y0: int
    x1: int
    y1: int

    @property
    def width(self) -> int:
        return self.x1 - self.x0

    @property
    def height(self) -> int:
        return self.y1 - self.y0

    def contains(self, points: np.ndarray) -> np.ndarray:
        xs, ys = points[:, 0], points[:, 1]
        return (xs >= self.x0) & (xs < self.x1) & (ys >= self.y0) & (ys < self.y1)


@dataclass(frozen=True)
class Piece:
    """
    Positions `first .. last` of a closed tour, inside the window, with the
    tour outside it just before `first` and just after `last`.
    """

    first: int
    last: int


@dataclass(frozen=True)
class WindowResult:
    old_steps: int
    new_steps: int
    status: str
    seconds: float
    nodes: int
    pieces: int
    required: int
    positions: np.ndarray | None

    @property
    def saved(self) -> int:
        return self.old_steps - self.new_steps


def cut_pieces(positions: np.ndarray, window: Window) -> list[Piece] | None:
    """
    The pieces of a closed tour inside `window`, in tour order, starting
    with the first piece after position 0. None if the tour never leaves
    the window, since then there is nothing outside to keep.
    """
    inside = window.contains(positions)
    if inside.all():
        return None

    count = len(positions)

    # rotate so the tour starts outside, then runs of inside positions are plain runs
    start = int(np.flatnonzero(~inside)[0])
    rotated = np.roll(inside, -start)

    edges = np.diff(np.concatenate([[0], rotated.astype(np.int64), [0]]))
    run_starts = np.flatnonzero(edges == 1)
    run_stops = np.flatnonzero(edges == -1)

    pieces = []
    for run_start, run_stop in zip(run_starts.tolist(), run_stops.tolist()):
        first = (run_start + start) % count
        last = (run_stop - 1 + start) % count
        pieces.append(Piece(first, last))

    return pieces


def solve_window(
    problem: Problem,
    tours: list[np.ndarray],
    tour_index: int,
    window: Window,
    *,
    time_limit: float = 10.0,
    workers: int = 8,
) -> WindowResult | None:
    """
    Re-solves the part of tour `tour_index` inside `window`. `tours` are the
    dense closed tours of a covering solution. Returns None if the window
    holds no piece of that tour or the whole tour.
    """
    started = time.monotonic()
    positions = tours[tour_index]

    pieces = cut_pieces(positions, window)
    if not pieces:
        return None

    old_steps = sum(_piece_steps(piece, len(positions)) for piece in pieces)
    required = _required_cells(problem, tours, tour_index, pieces, window)

    model = _WindowModel(problem, window, positions, pieces, required)
    model.add_hint(positions, pieces)
    model.limit_steps(old_steps)

    solver = cp_model.CpSolver()
    solver.parameters.max_time_in_seconds = time_limit
    solver.parameters.num_workers = workers
    status = solver.solve(model.model)

    status_name = solver.status_name(status)
    found = status in (cp_model.OPTIMAL, cp_model.FEASIBLE)

    new_positions = None
    new_steps = old_steps
    if found:
        new_steps = round(solver.objective_value)
        new_positions = model.splice(solver, positions, pieces)

    return WindowResult(
        old_steps=old_steps,
        new_steps=new_steps,
        status=status_name,
        seconds=round(time.monotonic() - started, 2),
        nodes=window.width * window.height,
        pieces=len(pieces),
        required=len(required),
        positions=new_positions,
    )


def _piece_steps(piece: Piece, count: int) -> int:
    # unit steps from the entry to the exit, inside the window
    return (piece.last - piece.first) % count


def _required_cells(
    problem: Problem,
    tours: list[np.ndarray],
    tour_index: int,
    pieces: list[Piece],
    window: Window,
) -> np.ndarray:
    """
    Region cells that anchors in the window can sweep and that no kept
    anchor sweeps: every other tour, and this tour outside its pieces.
    """
    offsets = problem.offsets
    offset_low = offsets.min(axis=0)
    offset_high = offsets.max(axis=0)

    # the cells the window can reach, and the anchors that can sweep them
    cell_x0 = window.x0 + int(offset_low[0])
    cell_y0 = window.y0 + int(offset_low[1])
    cell_width = window.width + int(offset_high[0] - offset_low[0])
    cell_height = window.height + int(offset_high[1] - offset_low[1])

    anchor_x0 = cell_x0 - int(offset_high[0])
    anchor_y0 = cell_y0 - int(offset_high[1])
    anchor_width = cell_width + int(offset_high[0] - offset_low[0])
    anchor_height = cell_height + int(offset_high[1] - offset_low[1])

    kept = np.zeros((anchor_height, anchor_width), dtype=bool)
    for index, positions in enumerate(tours):
        keep = np.ones(len(positions), dtype=bool)
        if index == tour_index:
            keep = ~_in_pieces(len(positions), pieces)

        points = positions[keep]
        xs = points[:, 0] - anchor_x0
        ys = points[:, 1] - anchor_y0
        near = (xs >= 0) & (xs < anchor_width) & (ys >= 0) & (ys < anchor_height)
        kept[ys[near], xs[near]] = True

    # a cell is swept if a kept anchor sits at cell minus some offset
    swept = np.zeros((cell_height, cell_width), dtype=bool)
    for dx, dy in offsets.tolist():
        left = cell_x0 - dx - anchor_x0
        top = cell_y0 - dy - anchor_y0
        swept |= kept[top : top + cell_height, left : left + cell_width]

    in_region = _region_window(problem, cell_x0, cell_y0, cell_width, cell_height)
    ys, xs = np.nonzero(in_region & ~swept)

    return np.stack([xs + cell_x0, ys + cell_y0], axis=1)


def _in_pieces(count: int, pieces: list[Piece]) -> np.ndarray:
    inside = np.zeros(count, dtype=bool)

    for piece in pieces:
        steps = _piece_steps(piece, count)
        indices = (piece.first + np.arange(steps + 1)) % count
        inside[indices] = True

    return inside


def _region_window(problem: Problem, x0: int, y0: int, width: int, height: int) -> np.ndarray:
    mask = problem.region.mask
    rx0, ry0 = problem.region.origin

    result = np.zeros((height, width), dtype=bool)

    # the overlap of the window with the region mask
    left = max(x0, rx0)
    right = min(x0 + width, rx0 + mask.shape[1])
    bottom = max(y0, ry0)
    top = min(y0 + height, ry0 + mask.shape[0])

    if left < right and bottom < top:
        result[bottom - y0 : top - y0, left - x0 : right - x0] = mask[
            bottom - ry0 : top - ry0, left - rx0 : right - rx0
        ]

    return result


class _WindowModel:
    """
    The CP-SAT model of one window. Grid nodes are numbered row by row, then
    two port nodes per piece.
    """

    def __init__(
        self,
        problem: Problem,
        window: Window,
        positions: np.ndarray,
        pieces: list[Piece],
        required: np.ndarray,
    ) -> None:
        self.window = window
        self.model = cp_model.CpModel()

        grid_nodes = window.width * window.height
        self.grid_nodes = grid_nodes
        node_count = grid_nodes + 2 * len(pieces)

        # arcs as (tail, head), grid steps first
        self.arcs: list[tuple[int, int]] = []
        self.uses = []
        for node in range(grid_nodes):
            x, y = self._point(node)
            for dx, dy in STEPS:
                neighbour = self._node(x + dx, y + dy)
                if neighbour is None:
                    continue

                self.arcs.append((node, neighbour))
                self.uses.append(self.model.new_int_var(0, MAX_USES, ""))

        self.grid_arcs = len(self.arcs)

        # port arcs, all taken exactly once
        self.ports = []
        for index, piece in enumerate(pieces):
            port_in = grid_nodes + 2 * index
            port_out = port_in + 1
            next_in = grid_nodes + 2 * ((index + 1) % len(pieces))

            entry = self._node(*positions[piece.first].tolist())
            exit_ = self._node(*positions[piece.last].tolist())

            for tail, head in ((port_in, entry), (exit_, port_out), (port_out, next_in)):
                self.arcs.append((tail, head))
                self.uses.append(self.model.new_constant(1))

            self.ports.append((port_in, port_out))

        self._balance_and_connect(node_count)
        self._cover(problem, required)

        self.model.minimize(sum(self.uses[: self.grid_arcs]))

    def _point(self, node: int) -> tuple[int, int]:
        row, column = divmod(node, self.window.width)
        return self.window.x0 + column, self.window.y0 + row

    def _node(self, x: int, y: int) -> int | None:
        column = x - self.window.x0
        row = y - self.window.y0
        if 0 <= column < self.window.width and 0 <= row < self.window.height:
            return row * self.window.width + column

        return None

    def _balance_and_connect(self, node_count: int) -> None:
        model = self.model

        incoming = [[] for _ in range(node_count)]
        outgoing = [[] for _ in range(node_count)]
        for index, (tail, head) in enumerate(self.arcs):
            outgoing[tail].append(index)
            incoming[head].append(index)

        # a grid node is visited if the walk steps onto it
        self.visited = []
        for node in range(self.grid_nodes):
            arriving = sum(self.uses[index] for index in incoming[node])
            leaving = sum(self.uses[index] for index in outgoing[node])
            model.add(arriving == leaving)

            visited = model.new_bool_var("")
            limit = MAX_USES * len(incoming[node])
            model.add(arriving >= 1).only_enforce_if(visited)
            model.add(arriving <= limit * visited)
            self.visited.append(visited)

        # one unit of flow from the first port to every other visited node
        demand = list(self.visited) + [1] * (node_count - self.grid_nodes)
        root = self.grid_nodes
        demand[root] = 0
        total = node_count

        flows = []
        for index in range(len(self.arcs)):
            # an arc taken twice can carry flow on both passes
            flow = model.new_int_var(0, MAX_USES * total, "")
            model.add(flow <= total * self.uses[index])
            flows.append(flow)
        self.flows = flows

        for node in range(node_count):
            inflow = sum(flows[index] for index in incoming[node])
            outflow = sum(flows[index] for index in outgoing[node])
            if node == root:
                continue

            model.add(inflow - outflow == demand[node])

    def _cover(self, problem: Problem, required: np.ndarray) -> None:
        offsets = problem.offsets

        for cell in required.tolist():
            sweepers = []
            for dx, dy in offsets.tolist():
                node = self._node(cell[0] - dx, cell[1] - dy)
                if node is not None:
                    sweepers.append(self.visited[node])

            self.model.add_bool_or(sweepers)

    def add_hint(self, positions: np.ndarray, pieces: list[Piece]) -> None:
        """
        The current pieces as a complete starting solution, flows included,
        so the solver holds a feasible walk from the first moment.
        """
        count = len(positions)
        index_of = {arc: index for index, arc in enumerate(self.arcs)}

        # the old walk as a node sequence through the ports
        walk = []
        for index, piece in enumerate(pieces):
            port_in, port_out = self.ports[index]
            walk.append(port_in)

            steps = _piece_steps(piece, count)
            for offset in range(steps + 1):
                here = positions[(piece.first + offset) % count]
                walk.append(self._node(*here.tolist()))

            walk.append(port_out)
        walk.append(self.ports[0][0])

        visited = [0] * self.grid_nodes
        for node in walk:
            if node < self.grid_nodes:
                visited[node] = 1

        # walk the old tour once, dropping one unit of flow at every first visit
        taken = [0] * len(self.arcs)
        flow = [0] * len(self.arcs)
        remaining = sum(visited) + 2 * len(pieces) - 1
        seen = {walk[0]}

        for tail, head in pairwise(walk):
            index = index_of[(tail, head)]
            taken[index] += 1
            flow[index] += remaining

            if head not in seen:
                seen.add(head)
                remaining -= 1

        for index in range(self.grid_arcs):
            self.model.add_hint(self.uses[index], taken[index])

        for index in range(len(self.arcs)):
            self.model.add_hint(self.flows[index], flow[index])

        for node in range(self.grid_nodes):
            self.model.add_hint(self.visited[node], visited[node])

    def limit_steps(self, old_steps: int) -> None:
        self.model.add(sum(self.uses[: self.grid_arcs]) <= old_steps)

    def splice(self, solver: cp_model.CpSolver, positions: np.ndarray, pieces: list[Piece]) -> np.ndarray:
        """
        The new dense closed tour: an Euler walk over the chosen steps, with
        the old outside run put in after every exit port.
        """
        count = len(positions)

        remaining: dict[int, list[int]] = {}
        for index, (tail, head) in enumerate(self.arcs):
            for _ in range(int(solver.value(self.uses[index]))):
                remaining.setdefault(tail, []).append(head)

        walk = _euler_walk(remaining, start=self.grid_nodes)

        outside_after = {}
        for index, piece in enumerate(pieces):
            next_first = pieces[(index + 1) % len(pieces)].first
            run = (next_first - piece.last) % count - 1
            outside_after[self.ports[index][1]] = (piece.last + 1 + np.arange(run)) % count

        result = []
        for node in walk[:-1]:
            if node < self.grid_nodes:
                result.append(self._point(node))
            elif node in outside_after:
                result.extend(map(tuple, positions[outside_after[node]].tolist()))

        return np.array(result, dtype=np.int64)


def _euler_walk(remaining: dict[int, list[int]], start: int) -> list[int]:
    """
    Hierholzer's algorithm on a balanced, connected directed multigraph.
    Consumes `remaining`. The walk starts and ends at `start`.
    """
    stack = [start]
    walk = []

    while stack:
        node = stack[-1]
        heads = remaining.get(node)
        if heads:
            stack.append(heads.pop())
        else:
            walk.append(stack.pop())

    walk.reverse()

    return walk
