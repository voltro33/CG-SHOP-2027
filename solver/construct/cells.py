"""
A sweep per cell: the region is cut into cells that follow its shape
(`decompose.py`), and each cell gets the lane pattern that suits it best.

For every cell the patterns tried are horizontal and vertical lanes, each at
a few lane offsets measured from the cell's own lowest row or leftmost
column, so lanes can hug the cell instead of the global grid. A pattern costs
`length + segment_cost * band` per segment, the same strip cost mixed lanes
uses, which charges every lane end for its overshoot and turn. The cheapest
pattern per cell wins, and all chosen segments are ordered together by the
usual order search.

This is the one shot version: one pattern per cell, chosen on its own. The
search over patterns, merges and boundaries comes later.
"""

import numpy as np

from ..decompose import DEFAULT_MIN_LANES, decompose
from ..model import Problem
from ..tours import Point
from .mixed import _horizontal_ends, _restricted
from .segments import walk_segments

__all__ = ["cell_segment_ends", "pattern_offsets", "segments_cells"]


def segments_cells(
    problem: Problem,
    *,
    min_lanes: int = DEFAULT_MIN_LANES,
    segment_cost: float = 3.0,
    gap_factor: float = 0.0,
    offsets: int = 4,
    kick_seconds: float | None = None,
    seed: int = 0,
    anneal: float = 0.0,
    time_limit: float = 10.0,
) -> list[Point]:
    """
    An open walk over the chosen segments of every cell.
    """
    left, right = cell_segment_ends(
        problem,
        min_lanes=int(min_lanes),
        segment_cost=segment_cost,
        gap_factor=gap_factor,
        offsets=int(offsets),
    )
    if len(left) == 0:
        return []

    return walk_segments(
        left,
        right,
        closed=problem.k == 1,
        kick_seconds=kick_seconds,
        seed=int(seed),
        anneal=anneal,
        time_limit=time_limit,
    )


def cell_segment_ends(
    problem: Problem,
    *,
    min_lanes: int,
    segment_cost: float,
    gap_factor: float,
    offsets: int,
) -> tuple[np.ndarray, np.ndarray]:
    """
    The two end points of every chosen segment, all cells together.
    """
    cells = decompose(problem, min_lanes=min_lanes)

    ends_left = []
    ends_right = []
    for cell in range(cells.count):
        part = _restricted(problem, cells.mask(cell))
        if part is None:
            continue

        left, right = _cheapest_pattern(part, segment_cost, gap_factor, offsets)
        ends_left.append(left)
        ends_right.append(right)

    if not ends_left:
        empty = np.zeros((0, 2), dtype=np.int64)
        return empty, empty

    return np.concatenate(ends_left), np.concatenate(ends_right)


def pattern_offsets(band: int, count: int) -> list[int]:
    """
    Up to `count` lane offsets spread over one band, always including 0.
    """
    if band <= count:
        return list(range(band))

    return sorted({(band * index) // count for index in range(count)})


def _cheapest_pattern(
    part: Problem, segment_cost: float, gap_factor: float, offsets: int
) -> tuple[np.ndarray, np.ndarray]:
    best = None
    best_cost = float("inf")

    for left, right, band in _patterns(part, gap_factor, offsets):
        length = float(np.abs(right - left).sum())
        cost = length + segment_cost * band * len(left)

        if cost < best_cost:
            best_cost = cost
            best = (left, right)

    return best


def _patterns(part: Problem, gap_factor: float, offsets: int):
    """
    Every pattern of one cell, as left ends, right ends and lane height.
    """
    band_y = _band(part.offsets[:, 1])
    lowest_y = _lowest_row(part)
    for offset in pattern_offsets(band_y, offsets):
        left, right = _horizontal_ends(part, lowest_y - offset, band_y, gap_factor)
        yield left, right, band_y

    # vertical lanes are horizontal lanes of the mirrored cell, flipped back
    mirrored = part.transposed()
    band_x = _band(part.offsets[:, 0])
    lowest_x = _lowest_row(mirrored)
    for offset in pattern_offsets(band_x, offsets):
        left, right = _horizontal_ends(mirrored, lowest_x - offset, band_x, gap_factor)
        yield left[:, ::-1], right[:, ::-1], band_x


def _band(values: np.ndarray) -> int:
    return int(values.max() - values.min()) + 1


def _lowest_row(problem: Problem) -> int:
    mask = problem.region.mask
    return problem.region.origin[1] + int(np.flatnonzero(mask.any(axis=1))[0])
