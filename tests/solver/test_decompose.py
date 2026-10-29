"""
Cells must start exactly where the region splits or joins, and small cells
must disappear into their neighbours.
"""

import numpy as np

from cgshop2027_pyutils.grid import CellSet

from solver.decompose import decompose
from solver.model import Problem


def problem_from(rows: list[str]) -> Problem:
    """
    A region drawn as text, top row first, `#` for a region cell, with a
    one cell cutter so every row is its own lane.
    """
    mask = np.array([[char == "#" for char in row] for row in reversed(rows)], dtype=bool)
    region = CellSet(mask, (0, 0))
    cutter = CellSet(np.ones((1, 1), dtype=bool), (0, 0))

    return Problem(uid="drawn_1s1", region=region, cutter=cutter, k=1)


L_SHAPE = [
    "##....",
    "##....",
    "##....",
    "######",
    "######",
]

U_SHAPE = [
    "##..##",
    "##..##",
    "##..##",
    "######",
    "######",
]

RING = [
    "######",
    "######",
    "##..##",
    "##..##",
    "######",
    "######",
]


def test_an_l_needs_no_split():
    # the foot narrows into the bar without splitting, one sweep runs through
    assert decompose(problem_from(L_SHAPE), min_lanes=1).count == 1


def test_a_u_splits_into_base_and_two_arms():
    cells = decompose(problem_from(U_SHAPE), min_lanes=1)

    assert cells.count == 3
    assert sorted(cells.pieces.tolist()) == [2, 3, 3]


def test_a_ring_gives_bottom_two_sides_and_top():
    assert decompose(problem_from(RING), min_lanes=1).count == 4


def test_every_region_cell_gets_one_cell():
    problem = problem_from(RING)
    cells = decompose(problem, min_lanes=1)

    mask = problem.region.mask
    assert np.all(cells.labels[mask] >= 0)
    assert np.all(cells.labels[~mask] == -1)


def test_small_cells_merge_into_neighbours():
    # the arms of the u are three lanes high, the base two
    cells = decompose(problem_from(U_SHAPE), min_lanes=3)

    assert cells.count == 2
