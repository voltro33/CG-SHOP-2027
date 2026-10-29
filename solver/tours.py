"""
Conversions between the three shapes a route takes in the solver:

- *corners*: a short list of points, consecutive ones axis-parallel (what the
  solution file stores);
- *positions*: every unit-step point along the route, as an `(n, 2)` array
  (what coverage and splitting work on);
- `CutterTour`: the validated schema object that goes into a solution.

All functions return new objects and leave their inputs untouched.
"""

from collections.abc import Sequence
from itertools import pairwise

import numpy as np

from cgshop2027_pyutils.schemas import CutterTour

Point = tuple[int, int]

__all__ = [
    "Point",
    "close_route",
    "compress",
    "connect",
    "densify",
    "l1",
    "normalize_cycle",
    "passes_straight",
    "route_length",
    "to_cutter_tour",
]


def l1(a: Point, b: Point) -> int:
    """
    Taxicab distance; the length of any shortest axis-parallel path.
    """
    return abs(a[0] - b[0]) + abs(a[1] - b[1])


def connect(points: Sequence[Point]) -> list[Point]:
    """
    Inserts a bend between consecutive points that differ in both coordinates
    (horizontal first), so every consecutive pair is axis-parallel. Repeated
    points are dropped.
    """
    result: list[Point] = []
    for point in points:
        p = (int(point[0]), int(point[1]))
        if result:
            prev = result[-1]
            if p == prev:
                continue
            if prev[0] != p[0] and prev[1] != p[1]:
                result.append((p[0], prev[1]))
        result.append(p)
    return result


def route_length(corners: Sequence[Point], *, closed: bool) -> int:
    pairs = pairwise(corners)
    length = sum(l1(a, b) for a, b in pairs)
    if closed and len(corners) > 1:
        length += l1(corners[-1], corners[0])
    return length


def densify(corners: Sequence[Point], *, closed: bool = False) -> np.ndarray:
    """
    The unit-step positions along axis-parallel `corners`. An open route
    includes both ends; a closed route includes the way back to the start but
    not the start a second time.
    """
    route = list(corners)
    if not route:
        return np.zeros((0, 2), dtype=np.int64)
    if closed and len(route) > 1:
        route = [*route, route[0]]
    chunks = [np.array([route[0]], dtype=np.int64)]
    for (x0, y0), (x1, y1) in pairwise(route):
        if x0 != x1 and y0 != y1:
            msg = f"Corners {(x0, y0)} and {(x1, y1)} are not axis-parallel."
            raise ValueError(msg)
        steps = abs(x1 - x0) + abs(y1 - y0)
        t = np.arange(1, steps + 1, dtype=np.int64)
        xs = x0 + np.sign(x1 - x0) * t
        ys = y0 + np.sign(y1 - y0) * t
        chunks.append(np.stack([xs, ys], axis=1))
    dense = np.concatenate(chunks)
    return dense[:-1] if closed and len(corners) > 1 else dense


def compress(positions: np.ndarray) -> list[Point]:
    """
    The corners of a unit-step path: its ends plus every point where the
    direction changes (reversals included, since they matter for coverage).
    """
    pts = np.asarray(positions, dtype=np.int64)
    if len(pts) <= 2:
        return [tuple(map(int, p)) for p in pts]
    steps = np.diff(pts, axis=0)
    turns = np.any(steps[1:] != steps[:-1], axis=1)
    keep = np.concatenate([[True], turns, [True]])
    return [tuple(map(int, p)) for p in pts[keep]]


def close_route(corners: Sequence[Point]) -> list[Point]:
    """
    Treats `corners` as a closed tour and makes it valid: adds a bend if the
    way back to the start is not axis-parallel, then normalizes it.
    """
    route = connect(corners)
    if len(route) > 1:
        first, last = route[0], route[-1]
        if first[0] != last[0] and first[1] != last[1]:
            route = [*route, (first[0], last[1])]
    return normalize_cycle(route)


def normalize_cycle(corners: Sequence[Point]) -> list[Point]:
    """
    Removes, cyclically, points that repeat their predecessor and points that
    lie on a straight run between their neighbours. Neither removal changes
    the positions the tour passes through.
    """
    route = list(corners)
    changed = True
    while changed and len(route) > 1:
        changed = False
        for i in range(len(route)):
            n = len(route)
            prev, cur, nxt = route[i - 1], route[i], route[(i + 1) % n]
            if cur == nxt or (n > 2 and passes_straight(prev, cur, nxt)):
                route = route[:i] + route[i + 1 :]
                changed = True
                break
    return route


def passes_straight(a: Point, b: Point, c: Point) -> bool:
    """
    True if `b` lies strictly on the way from `a` to `c` along one axis.
    """
    if a[0] == b[0] == c[0]:
        return (a[1] < b[1] < c[1]) or (a[1] > b[1] > c[1])
    if a[1] == b[1] == c[1]:
        return (a[0] < b[0] < c[0]) or (a[0] > b[0] > c[0])
    return False


def to_cutter_tour(corners: Sequence[Point]) -> CutterTour:
    route = close_route(corners)
    return CutterTour(x=[p[0] for p in route], y=[p[1] for p in route])
