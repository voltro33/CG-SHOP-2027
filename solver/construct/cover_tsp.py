"""
"Stamp, then connect the dots":

1. Set cover: pick anchor spots so that their footprints together cover every
   region cell, greedily taking the spot that covers the most still-uncovered
   cells (lazy greedy with a heap).
2. TSP: visit the chosen spots in a short order. Distances are taxicab, since
   holes are not obstacles. Nearest neighbour builds an order, windowed 2-opt
   then removes crossings.

Stamps only count where the cutter *stands*, so the walk between spots sweeps
extra cells for free; the local search later cashes that in by cutting
corners. Swapping in a real TSP solver (LKH, OR-Tools) or an exact set cover
(ILP / CP-SAT) are natural upgrades.
"""

import heapq
import time

import numpy as np

from ..model import Problem
from ..tours import Point, connect

__all__ = ["cover_tsp", "greedy_cover", "nearest_neighbor_order", "two_opt"]


def cover_tsp(problem: Problem, *, time_limit: float = 10.0) -> list[Point]:
    deadline = time.monotonic() + time_limit
    anchors = greedy_cover(problem)
    order = nearest_neighbor_order(anchors)
    order = two_opt(anchors[order], deadline=deadline)
    return connect([(int(x), int(y)) for x, y in order])


def greedy_cover(problem: Problem) -> np.ndarray:
    """
    Anchors whose footprints cover the region, as an `(m, 2)` array.
    """
    (ax, ay), (height, width) = problem.anchor_frame()
    cx0, cy0, cx1, cy1 = problem.cutter.bounds
    # `uncovered[r, c]` is the cell (ax + cx0 + c, ay + cy0 + r).
    uncovered = np.zeros((height + cy1 - cy0, width + cx1 - cx0), dtype=bool)
    rx0, ry0 = problem.region.origin
    row, col = ry0 - (ay + cy0), rx0 - (ax + cx0)
    uncovered[row : row + problem.region.height, col : col + problem.region.width] = (
        problem.region.mask
    )
    dys = problem.offsets[:, 1] - cy0
    dxs = problem.offsets[:, 0] - cx0
    gain = np.zeros((height, width), dtype=np.int32)
    for dy, dx in zip(dys.tolist(), dxs.tolist(), strict=True):
        gain += uncovered[dy : dy + height, dx : dx + width]

    heap = [(-int(g), r, c) for r, c, g in _nonzero(gain)]
    heapq.heapify(heap)
    remaining = int(uncovered.sum())
    chosen: list[Point] = []
    while remaining and heap:
        stale, r, c = heapq.heappop(heap)
        actual = int(uncovered[r + dys, c + dxs].sum())
        if actual == 0:
            continue
        if actual < -stale:
            heapq.heappush(heap, (-actual, r, c))
            continue
        uncovered[r + dys, c + dxs] = False
        remaining -= actual
        chosen.append((ax + c, ay + r))
    return np.array(chosen, dtype=np.int64).reshape(-1, 2)


def _nonzero(grid: np.ndarray):
    rows, cols = np.nonzero(grid)
    return zip(rows.tolist(), cols.tolist(), grid[rows, cols].tolist(), strict=True)


def nearest_neighbor_order(points: np.ndarray, start: int = 0) -> np.ndarray:
    """
    Indices of `points` in nearest-neighbour order (taxicab), from `start`.
    O(n^2) with numpy; fine up to a few tens of thousands of points.
    """
    n = len(points)
    if n == 0:
        return np.zeros(0, dtype=np.int64)
    remaining = np.delete(np.arange(n), start)
    order = [start]
    current = points[start]
    while len(remaining):
        dist = np.abs(points[remaining] - current).sum(axis=1)
        pick = int(np.argmin(dist))
        order.append(int(remaining[pick]))
        current = points[remaining[pick]]
        remaining = np.delete(remaining, pick)
    return np.array(order, dtype=np.int64)


def two_opt(path: np.ndarray, *, deadline: float, window: int = 50) -> np.ndarray:
    """
    Windowed 2-opt on an open path: reverse `path[i+1 .. j]` whenever that
    shortens it, looking only `window` steps ahead. Returns a new path.
    """
    pts = np.array(path, dtype=np.int64)
    n = len(pts)
    improved = True
    while improved and time.monotonic() < deadline:
        improved = False
        for i in range(n - 2):
            if i % 256 == 0 and time.monotonic() >= deadline:
                break
            j = np.arange(i + 2, min(n, i + 2 + window))
            if len(j) == 0:
                continue
            before = np.abs(pts[i] - pts[i + 1]).sum()
            nxt = np.minimum(j + 1, n - 1)
            is_end = j == n - 1
            old = before + np.where(is_end, 0, np.abs(pts[j] - pts[nxt]).sum(axis=1))
            new = np.abs(pts[i] - pts[j]).sum(axis=1) + np.where(
                is_end, 0, np.abs(pts[i + 1] - pts[nxt]).sum(axis=1)
            )
            delta = new - old
            best = int(np.argmin(delta))
            if delta[best] < 0:
                k = int(j[best])
                pts[i + 1 : k + 1] = pts[i + 1 : k + 1][::-1].copy()
                improved = True
    return pts
