"""
Lower bounds on the objective. No solution can beat them, so the gap between a
bound and a solution is the most room there could possibly be left.

Neither bound is tight; finding better ones is a good project on its own.
"""

from .model import Problem

__all__ = ["area_bound", "extent_bound", "lower_bound"]


def area_bound(problem: Problem) -> int:
    """
    Each cutter starts with one footprint and every unit step adds at most
    `step_gain` new cells. With `k` cutters of length at most `L`:

        k * |C| + k * L * gain >= cells   =>   L >= (cells - k|C|) / (k * gain)

    Closed walks on the grid have even length, so the bound rounds up to even.
    """
    remaining = problem.num_cells - problem.k * len(problem.cutter)
    if remaining <= 0:
        return 0
    per_cutter = -(-remaining // (problem.k * problem.step_gain()))
    return per_cutter + per_cutter % 2


def extent_bound(problem: Problem) -> int:
    """
    With a single cutter, its tour must reach far enough left, right, down and
    up to touch the region's extreme cells, and come back: twice the span of
    the anchors it is forced to visit. Only valid for `k == 1`.
    """
    if problem.k != 1:
        return 0
    rx0, ry0, rx1, ry1 = problem.region.bounds
    cx0, cy0, cx1, cy1 = problem.cutter.bounds
    span_x = max(0, (rx1 - cx1) - (rx0 - cx0))
    span_y = max(0, (ry1 - cy1) - (ry0 - cy0))
    return 2 * (span_x + span_y)


def lower_bound(problem: Problem) -> int:
    return max(area_bound(problem), extent_bound(problem))
