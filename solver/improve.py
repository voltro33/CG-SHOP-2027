"""
Local search: shorten routes by cutting corners as long as every region cell
stays swept.

The move: for a corner `b` between neighbours `a` and `c`, replace `a -> b -> c`
with the shortest way `a -> c` (straight, or one of its two L-shaped bends).
By the triangle inequality this never gets longer. The only question is
whether the cells `b` was responsible for are still swept by something else.

To answer that quickly, `VisitCounts` keeps, for every anchor, how many times
any route stands there. A cell is swept iff some anchor in its neighborhood has
a positive count, so a move only has to re-check the cells around the
positions it gives up.

Ideas for further moves (see the docs): 2-opt between corners, moving a whole
lane to another cutter, merging two parallel passes into one.
"""

import time
from collections.abc import Sequence
from itertools import pairwise

import numpy as np

from cgshop2027_pyutils.grid import CellSet, dilate

from .model import Problem, points_to_cellset
from .tours import Point, densify, l1, normalize_cycle, passes_straight

__all__ = ["VisitCounts", "prune_routes"]

# above this many removed positions times cutter cells, check coverage with
# masks instead of point lists
_POINTWISE_LIMIT = 2_000_000


class VisitCounts:
    """
    Visit multiplicity per anchor over the anchor frame of a problem. This is
    working state owned by one local search run, so it is updated in place.
    """

    def __init__(self, problem: Problem, routes: Sequence[np.ndarray]) -> None:
        self._problem = problem
        self._origin, shape = problem.anchor_frame()
        self._counts = np.zeros(shape, dtype=np.int32)
        self._offsets = problem.offsets
        for positions in routes:
            self.add(positions, 1)

    def add(self, positions: np.ndarray, sign: int) -> None:
        rows, cols, inside = self._index(positions)
        np.add.at(self._counts, (rows[inside], cols[inside]), sign)

    def all_swept_around(self, removed: np.ndarray) -> bool:
        """
        True if every region cell in the footprints of `removed` is still
        swept by some visited anchor.
        """
        if len(removed) == 0:
            return True

        # big cutters times long removals blow up memory, so switch to masks
        if len(removed) * len(self._offsets) > _POINTWISE_LIMIT:
            return self._all_swept_windowed(removed)

        cells = (removed[:, None, :] + self._offsets[None, :, :]).reshape(-1, 2)
        cells = np.unique(cells, axis=0)
        cells = cells[self._problem.region.contains_points(cells)]
        if len(cells) == 0:
            return True

        # the anchors below are cells times cutter cells, which can be far
        # more than removed times cutter cells for a big cutter
        if len(cells) * len(self._offsets) > _POINTWISE_LIMIT:
            return self._all_swept_windowed(removed)

        anchors = (cells[:, None, :] - self._offsets[None, :, :]).reshape(-1, 2)
        rows, cols, inside = self._index(anchors)
        hits = np.zeros(len(anchors), dtype=np.int32)
        hits[inside] = self._counts[rows[inside], cols[inside]]
        return bool(np.all(hits.reshape(len(cells), -1).sum(axis=1) > 0))

    def _all_swept_windowed(self, removed: np.ndarray) -> bool:
        """
        Same answer as `all_swept_around`, computed with whole masks inside a
        window around `removed` instead of one cell at a time. Memory grows
        with the window area, not with removed positions times cutter cells.
        """
        cutter = self._problem.cutter
        cx0, cy0, cx1, cy1 = cutter.bounds

        # the region cells the removed stamps used to sweep
        footprint = dilate(points_to_cellset(removed), cutter)
        needed = self._problem.region & footprint
        if len(needed) == 0:
            return True

        # every anchor whose stamp could reach one of those cells
        nx0, ny0, nx1, ny1 = needed.bounds
        ax0 = nx0 - cx1
        ay0 = ny0 - cy1
        ax1 = nx1 - cx0
        ay1 = ny1 - cy0

        visited = np.zeros((ay1 - ay0 + 1, ax1 - ax0 + 1), dtype=bool)

        # copy the part of the counts frame that overlaps the window
        fx0, fy0 = self._origin
        height, width = self._counts.shape
        left = max(ax0, fx0)
        right = min(ax1, fx0 + width - 1)
        bottom = max(ay0, fy0)
        top = min(ay1, fy0 + height - 1)

        if left <= right and bottom <= top:
            counts_window = self._counts[
                bottom - fy0 : top - fy0 + 1, left - fx0 : right - fx0 + 1
            ]
            visited[
                bottom - ay0 : top - ay0 + 1, left - ax0 : right - ax0 + 1
            ] = counts_window > 0

        swept = dilate(CellSet(visited, (ax0, ay0)), cutter)

        return len(needed.difference(swept)) == 0

    def _index(self, positions: np.ndarray):
        pts = np.asarray(positions, dtype=np.int64).reshape(-1, 2)
        cols = pts[:, 0] - self._origin[0]
        rows = pts[:, 1] - self._origin[1]
        height, width = self._counts.shape
        inside = (rows >= 0) & (rows < height) & (cols >= 0) & (cols < width)
        return rows, cols, inside


def prune_routes(
    problem: Problem,
    routes: Sequence[Sequence[Point]],
    *,
    closed: bool,
    deadline: float,
) -> list[list[Point]]:
    """
    Cuts corners of every route while coverage allows, longest route first
    (the longest one is the objective). Returns new routes.
    """
    counts = VisitCounts(problem, [densify(r, closed=closed) for r in routes])
    result = [list(r) for r in routes]
    order = sorted(range(len(result)), key=lambda i: -_length(result[i], closed))
    for index in order:
        if time.monotonic() >= deadline:
            break
        result[index] = _prune_route(result[index], counts, closed, deadline)
    return result


def _length(route: Sequence[Point], closed: bool) -> int:
    total = sum(l1(a, b) for a, b in pairwise(route))
    return total + (l1(route[-1], route[0]) if closed and len(route) > 1 else 0)


def _prune_route(
    route: list[Point], counts: VisitCounts, closed: bool, deadline: float
) -> list[Point]:
    improved = True
    while improved and time.monotonic() < deadline:
        improved = False
        i = 0 if closed else 1
        while i < (len(route) if closed else len(route) - 1):
            if len(route) < 3 or time.monotonic() >= deadline:
                break
            candidate = _try_cut(route, i, counts)
            if candidate is None:
                i += 1
            else:
                route = (
                    normalize_cycle(candidate) if closed else _normalize_open(candidate)
                )
                improved = True
    return route


def _try_cut(route: list[Point], i: int, counts: VisitCounts) -> list[Point] | None:
    """
    Tries to replace corner `i` by a shorter way between its neighbours.
    Applies the change to `counts` and returns the new route on success;
    leaves everything as it was and returns `None` otherwise.
    """
    n = len(route)
    a, b, c = route[i - 1], route[i], route[(i + 1) % n]
    if a == c:  # a spike out to b and straight back: drop it entirely
        removed = densify([a, b, a])[1:]
        drop = {i, (i + 1) % n}
        options = [([p for j, p in enumerate(route) if j not in drop], removed, None)]
    else:
        old_inner = densify([a, b, c])[1:-1]
        aligned = a[0] == c[0] or a[1] == c[1]
        bends = [[]] if aligned else [[(c[0], a[1])], [(a[0], c[1])]]
        gain = l1(a, b) + l1(b, c) - l1(a, c)
        if gain == 0 and not aligned:
            return None  # same length, same number of corners: no progress
        options = [
            (route[:i] + bend + route[i + 1 :], old_inner, densify([a, *bend, c])[1:-1])
            for bend in bends
        ]
    for new_route, removed, added in options:
        counts.add(removed, -1)
        if added is not None:
            counts.add(added, 1)
        if counts.all_swept_around(removed):
            return new_route
        if added is not None:
            counts.add(added, -1)
        counts.add(removed, 1)
    return None


def _normalize_open(route: Sequence[Point]) -> list[Point]:
    """
    Like `normalize_cycle`, but for an open walk: its two ends always stay.
    """
    result: list[Point] = []
    for point in route:
        if result and point == result[-1]:
            continue
        while len(result) >= 2 and passes_straight(result[-2], result[-1], point):
            result.pop()
        result.append(point)
    return result
