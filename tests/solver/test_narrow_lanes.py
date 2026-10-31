"""
Narrow lanes answer only for the widest cutter rows, a full pitch must give
back the plain lanes, and closer lanes must still sweep every cell.
"""

import numpy as np

from cgshop2027_pyutils.grid import CellSet

from solver.construct.groups import groups_horizontal
from solver.construct.segments import lane_segments, widest_rows
from solver.model import Problem
from solver.tours import densify


def diamond(radius: int) -> CellSet:
    size = 2 * radius + 1
    mask = np.zeros((size, size), dtype=bool)
    for row in range(size):
        half = radius - abs(row - radius)
        mask[row, radius - half : radius + half + 1] = True

    return CellSet(mask, (-radius, -radius))


def block_problem(cutter: CellSet, k: int = 1) -> Problem:
    mask = np.ones((12, 20), dtype=bool)
    region = CellSet(mask, (0, 0))

    return Problem(uid=f"block_{k}d1", region=region, cutter=cutter, k=k)


def test_widest_rows_pick_the_middle_of_a_diamond():
    extents = {-2: (0, 0), -1: (-1, 1), 0: (-2, 2), 1: (-1, 1), 2: (0, 0)}

    window = widest_rows(extents, 3)

    assert sorted(window) == [-1, 0, 1]


def test_all_rows_when_the_window_is_the_whole_cutter():
    extents = {0: (0, 1), 1: (0, 3)}

    assert widest_rows(extents, 5) == extents


def test_full_pitch_gives_the_plain_lanes():
    problem = block_problem(diamond(2))

    plain = lane_segments(problem, gap_factor=0.0)
    full = lane_segments(problem, gap_factor=0.0, pitch=1.0)

    assert plain == full


def test_narrow_lanes_are_closer_and_shorter():
    problem = block_problem(diamond(2))

    plain = lane_segments(problem, gap_factor=0.0)
    narrow = lane_segments(problem, gap_factor=0.0, pitch=0.6)

    plain_rows = sorted({s.y for s in plain})
    narrow_rows = sorted({s.y for s in narrow})
    assert narrow_rows[1] - narrow_rows[0] == 3
    assert plain_rows[1] - plain_rows[0] == 5

    # the wide middle reaches the border from further in
    assert max(s.b - s.a for s in narrow) < max(s.b - s.a for s in plain)


def test_narrow_lanes_still_sweep_every_cell():
    problem = block_problem(diamond(2), k=2)

    tours = groups_horizontal(problem, pitch=0.6, kick_seconds=0.1, time_limit=0.1)
    walks = [densify(tour, closed=True) for tour in tours]

    assert len(list(problem.uncovered(np.concatenate(walks)).cells())) == 0
