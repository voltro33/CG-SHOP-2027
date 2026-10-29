"""
Lane segments: boustrophedon lanes that skip the gaps.

Plain lanes (`lanes.py`) run from the leftmost to the rightmost region cell of
their band, so a lane over a U shape pays for the empty middle too. Here each
lane is cut into *segments*, the stretches of anchor positions that actually
have cells to mow, and the segments are then put in a short order:

1. For every lane, every region run in every cutter row of its band asks for
   an interval of anchor positions. Overlapping intervals are merged, and so
   are intervals separated by less than `max_gap`: crossing a short gap in a
   straight line is cheaper than making a separate trip later.
2. Nearest neighbour picks an order (each segment can be driven either way).
3. Windowed 2-opt untangles the order.

`max_gap` is the knob: tiny values make many small segments to reorder, huge
values give back the plain lanes.
"""

import time
from dataclasses import dataclass

import numpy as np

from ..model import Problem
from ..tours import Point, connect

__all__ = [
    "Segment",
    "lane_segments",
    "order_segments",
    "segments_horizontal",
    "segments_vertical",
]


@dataclass(frozen=True)
class Segment:
    """
    A horizontal stretch of anchor positions `x = a..b` at height `y`.
    """

    y: int
    a: int
    b: int


def segments_horizontal(
    problem: Problem,
    *,
    max_gap: int | None = None,
    offset: int = 0,
    time_limit: float = 10.0,
) -> list[Point]:
    """
    An open walk over horizontal lane segments.
    """
    segments = lane_segments(problem, max_gap=max_gap, offset=offset)
    if not segments:
        return []

    deadline = time.monotonic() + time_limit
    entries, exits = order_segments(segments, deadline=deadline)

    # walk each segment from its entry to its exit, in order
    corners: list[Point] = []
    for entry, exit_ in zip(entries.tolist(), exits.tolist(), strict=True):
        corners.append((entry[0], entry[1]))
        corners.append((exit_[0], exit_[1]))

    return connect(corners)


def segments_vertical(
    problem: Problem,
    *,
    max_gap: int | None = None,
    offset: int = 0,
    time_limit: float = 10.0,
) -> list[Point]:
    """
    The same over vertical lanes, by solving the mirrored problem.
    """
    walk = segments_horizontal(
        problem.transposed(), max_gap=max_gap, offset=offset, time_limit=time_limit
    )

    # mirror back across the diagonal
    return [(y, x) for x, y in walk]


# step 1, cut lanes into segments


def lane_segments(
    problem: Problem, *, max_gap: int | None = None, offset: int = 0
) -> list[Segment]:
    """
    The segments of every horizontal lane, bottom to top, left to right.

    Lanes are one cutter height apart. `offset` shifts all lanes down by that
    many rows, which changes where the band borders fall.
    """
    extents = problem.row_extents()
    dy_low = min(extents)
    dy_high = max(extents)
    band = dy_high - dy_low + 1

    if max_gap is None:
        max_gap = 2 * band

    runs_by_row = _row_runs(problem)
    if not runs_by_row:
        return []

    lowest_row = min(runs_by_row)
    highest_row = max(runs_by_row)

    # the first lane must reach the lowest row with its top cutter row
    y_first = lowest_row - dy_low - (offset % band)
    y_last = highest_row - dy_low

    segments: list[Segment] = []
    for y in range(y_first, y_last + 1, band):
        intervals = _lane_intervals(runs_by_row, extents, y)
        for a, b in _merge(intervals, max_gap):
            segments.append(Segment(y, a, b))

    return segments


def _row_runs(problem: Problem) -> dict[int, np.ndarray]:
    """
    For each grid row with region cells: its runs of consecutive cells as an
    `(r, 2)` array of `[first_x, last_x]`.
    """
    mask = problem.region.mask
    x0, y0 = problem.region.origin

    runs: dict[int, np.ndarray] = {}
    for row in np.nonzero(mask.any(axis=1))[0].tolist():
        # pad with false on both sides so every run has a start and an end
        padded = np.concatenate(([False], mask[row], [False])).astype(np.int8)
        changes = np.diff(padded)

        starts = np.nonzero(changes == 1)[0]
        ends = np.nonzero(changes == -1)[0] - 1

        runs[y0 + row] = np.stack([starts + x0, ends + x0], axis=1)

    return runs


def _lane_intervals(
    runs_by_row: dict[int, np.ndarray],
    extents: dict[int, tuple[int, int]],
    y: int,
) -> np.ndarray:
    """
    The anchor intervals a lane at height `y` must drive so that every region
    run in its band gets swept, as an `(m, 2)` array, unsorted.

    Cutter row `dy` spans `lx..rx` around the center, so driving from `a` to
    `b` sweeps `a + lx .. b + rx` in grid row `y + dy`. A run `s..e` is
    therefore swept by the interval `s - lx .. e - rx`. When the run is
    shorter than the cutter row, that interval is backwards and any single
    anchor in between does the job; we take the middle one.
    """
    pieces = []
    for dy, (lx, rx) in extents.items():
        runs = runs_by_row.get(y + dy)
        if runs is None:
            continue

        a = runs[:, 0] - lx
        b = runs[:, 1] - rx

        short = a > b
        middle = (a + b) // 2
        a = np.where(short, middle, a)
        b = np.where(short, middle, b)

        pieces.append(np.stack([a, b], axis=1))

    if not pieces:
        return np.zeros((0, 2), dtype=np.int64)

    return np.concatenate(pieces)


def _merge(intervals: np.ndarray, max_gap: int) -> list[tuple[int, int]]:
    """
    Sorts intervals and merges any two that overlap or whose gap is at most
    `max_gap`.
    """
    if len(intervals) == 0:
        return []

    order = np.argsort(intervals[:, 0], kind="stable")
    starts = intervals[order, 0]
    ends = intervals[order, 1]

    # the furthest point reached so far, so a long interval swallows short ones
    reach = np.maximum.accumulate(ends)

    # a new segment starts where the gap to everything before is too big
    breaks = np.nonzero(starts[1:] > reach[:-1] + max_gap)[0] + 1
    first_of_each = np.concatenate(([0], breaks))
    last_of_each = np.concatenate((breaks - 1, [len(starts) - 1]))

    merged = []
    for first, last in zip(first_of_each.tolist(), last_of_each.tolist(), strict=True):
        merged.append((int(starts[first]), int(reach[last])))

    return merged


# steps 2 and 3, order the segments


def order_segments(
    segments: list[Segment], *, deadline: float, window: int = 60
) -> tuple[np.ndarray, np.ndarray]:
    """
    A short order for driving all segments, as two `(n, 2)` arrays: where the
    walk enters each segment and where it leaves it.
    """
    left = np.array([(s.a, s.y) for s in segments], dtype=np.int64)
    right = np.array([(s.b, s.y) for s in segments], dtype=np.int64)

    entries, exits = _nearest_neighbor(left, right)
    entries, exits = _two_opt(entries, exits, deadline=deadline, window=window)

    return entries, exits


def _nearest_neighbor(
    left: np.ndarray, right: np.ndarray
) -> tuple[np.ndarray, np.ndarray]:
    """
    Greedy: from where we are, drive to the closest end of any segment not
    yet done, then across it to its other end.
    """
    n = len(left)
    done = np.zeros(n, dtype=bool)
    entries = np.empty((n, 2), dtype=np.int64)
    exits = np.empty((n, 2), dtype=np.int64)

    # start at the first segment, entering from the left
    current = 0
    entries[0] = left[0]
    exits[0] = right[0]
    done[0] = True
    position = right[0]

    big = np.iinfo(np.int64).max // 4
    for step in range(1, n):
        to_left = np.abs(left - position).sum(axis=1)
        to_right = np.abs(right - position).sum(axis=1)

        # finished segments must never be picked again
        to_left[done] = big
        to_right[done] = big

        best_left = int(np.argmin(to_left))
        best_right = int(np.argmin(to_right))

        if to_left[best_left] <= to_right[best_right]:
            current = best_left
            entries[step] = left[current]
            exits[step] = right[current]
        else:
            current = best_right
            entries[step] = right[current]
            exits[step] = left[current]

        done[current] = True
        position = exits[step]

    return entries, exits


def _two_opt(
    entries: np.ndarray, exits: np.ndarray, *, deadline: float, window: int
) -> tuple[np.ndarray, np.ndarray]:
    """
    Windowed 2-opt on an open sequence of segments. Reversing segments
    `i+1..j` also swaps each one's entry and exit, so only the two links
    at the ends of the reversed block change:

        exit[i] -> entry[i+1]   becomes   exit[i] -> exit[j]
        exit[j] -> entry[j+1]   becomes   entry[i+1] -> entry[j+1]
    """
    entries = entries.copy()
    exits = exits.copy()
    n = len(entries)

    improved = True
    while improved and time.monotonic() < deadline:
        improved = False

        for i in range(n - 1):
            # checking the clock is not free, so only do it now and then
            if i % 256 == 0 and time.monotonic() >= deadline:
                break

            j = np.arange(i + 1, min(n, i + 1 + window))
            next_of_j = np.minimum(j + 1, n - 1)
            j_is_last = j == n - 1

            old_first = np.abs(exits[i] - entries[i + 1]).sum()
            new_first = np.abs(exits[i] - exits[j]).sum(axis=1)

            old_second = np.abs(exits[j] - entries[next_of_j]).sum(axis=1)
            new_second = np.abs(entries[i + 1] - entries[next_of_j]).sum(axis=1)

            # the last segment has no link after it
            old_second = np.where(j_is_last, 0, old_second)
            new_second = np.where(j_is_last, 0, new_second)

            delta = (new_first + new_second) - (old_first + old_second)
            best = int(np.argmin(delta))
            if delta[best] >= 0:
                continue

            last = int(j[best])
            block_entries = entries[i + 1 : last + 1][::-1].copy()
            block_exits = exits[i + 1 : last + 1][::-1].copy()

            # reversed driving direction swaps the ends of every segment
            entries[i + 1 : last + 1] = block_exits
            exits[i + 1 : last + 1] = block_entries
            improved = True

    return entries, exits
