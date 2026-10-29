"""
Boustrophedon ("ox-turning") lanes: mow back and forth in straight lanes, each
one cutter-height apart, like mowing a real lawn.

Moving horizontally from anchor x = a to x = b sweeps, in cutter row `dy`, the
cells from `a + lx(dy)` to `b + rx(dy)`, where `lx`/`rx` are that row's
leftmost/rightmost cutter offsets. So a lane whose center runs along `y = Y`
sweeps the rows `Y + dy` for every cutter row `dy`, and only needs to run as
far as the region's cells in those rows demand.

Each lane spans from the leftmost to the rightmost region cell in its band,
gaps included; teaching it to skip gaps is a good first improvement.
"""

import numpy as np

from ..model import Problem
from ..tours import Point, connect

__all__ = ["lanes_horizontal", "lanes_vertical"]


def lanes_horizontal(problem: Problem) -> list[Point]:
    """
    An open snake walk over horizontal lanes, bottom to top.
    """
    extents = problem.row_extents()
    dy_low, dy_high = min(extents), max(extents)
    band = dy_high - dy_low + 1
    rows = _row_ranges(problem)
    if not rows:
        return []
    y_first = min(rows) - dy_low
    y_last = max(rows) - dy_high
    corners: list[Point] = []
    lane = 0
    for y in range(y_first, max(y_first, y_last + band - 1) + 1, band):
        span = _lane_span(rows, extents, y)
        if span is None:
            continue
        a, b = span
        corners += [(a, y), (b, y)] if lane % 2 == 0 else [(b, y), (a, y)]
        lane += 1
    return connect(corners)


def lanes_vertical(problem: Problem) -> list[Point]:
    """
    The same snake over vertical lanes, by solving the mirrored problem.
    """
    return [(y, x) for x, y in lanes_horizontal(problem.transposed())]


def _row_ranges(problem: Problem) -> dict[int, tuple[int, int]]:
    """
    For every grid row holding region cells: its leftmost and rightmost cell.
    """
    mask, (x0, y0) = problem.region.mask, problem.region.origin
    ranges = {}
    for row in np.nonzero(mask.any(axis=1))[0].tolist():
        cols = np.nonzero(mask[row])[0]
        ranges[y0 + row] = (x0 + int(cols[0]), x0 + int(cols[-1]))
    return ranges


def _lane_span(
    rows: dict[int, tuple[int, int]], extents: dict[int, tuple[int, int]], y: int
) -> tuple[int, int] | None:
    """
    The anchor x-range `[a, b]` a lane at height `y` must travel, or `None`
    if its band holds no region cells.
    """
    starts, ends = [], []
    for dy, (lx, rx) in extents.items():
        if y + dy in rows:
            low, high = rows[y + dy]
            starts.append(low - lx)
            ends.append(high - rx)
    if not starts:
        return None
    a, b = min(starts), max(ends)
    return (a, max(a, b))
