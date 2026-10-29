"""
Tour splitting: cut one long walk into `k` consecutive pieces and close each
piece into a tour, so that the longest closed piece is as short as possible.

A piece covering positions `i..j` of a unit-step walk costs

    (j - i)  +  |P[j] - P[i]|_1
    ^ walking    ^ driving back to the start

Binary search on the target length `T` and greedily make every piece as long
as `T` allows; this is the classic Frederickson-Hecht-Kim idea for min-max
k-TSP, applied to an already-ordered walk.
"""

import numpy as np

__all__ = ["closed_cost", "split_walk"]


def closed_cost(positions: np.ndarray, i: int, j: int) -> int:
    return int((j - i) + np.abs(positions[j] - positions[i]).sum())


def split_walk(positions: np.ndarray, k: int) -> list[np.ndarray]:
    """
    Splits a unit-step walk into at most `k` consecutive pieces whose closed
    costs have the smallest achievable maximum (up to the greedy's accuracy).
    Every position lands in exactly one piece.
    """
    pts = np.asarray(positions, dtype=np.int64)
    n = len(pts)
    if n == 0 or k <= 1:
        return [pts]
    low, high = 0, closed_cost(pts, 0, n - 1)
    best = _greedy(pts, k, high)
    while low < high:
        target = (low + high) // 2
        pieces = _greedy(pts, k, target)
        if pieces is None:
            low = target + 1
        else:
            best, high = pieces, target
    if best is None:  # cannot happen: `high` starts feasible with one piece
        return [pts]
    return [pts[i : j + 1] for i, j in best]


def _greedy(pts: np.ndarray, k: int, target: int) -> list[tuple[int, int]] | None:
    """
    Pieces as long as `target` allows, or `None` if `k` pieces do not suffice.
    """
    n = len(pts)
    pieces: list[tuple[int, int]] = []
    start = 0
    while start < n:
        if len(pieces) == k:
            return None
        stop = min(n - 1, start + target)
        window = pts[start : stop + 1]
        costs = np.arange(len(window)) + np.abs(window - pts[start]).sum(axis=1)
        ok = np.nonzero(costs <= target)[0]
        end = start + int(ok[-1])
        pieces.append((start, end))
        start = end + 1
    return pieces
