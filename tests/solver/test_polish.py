"""
Polishing must never lengthen a tour, must keep every cell swept, and its
tiles must reach every position of the tour.
"""

import random
import time

import numpy as np

from cgshop2027_pyutils.grid import CellSet

from solver.model import Problem
from solver.polish import _tiles, polish_seconds, polish_tours
from solver.tours import densify


def block_problem() -> Problem:
    mask = np.ones((4, 8), dtype=bool)
    region = CellSet(mask, (0, 0))
    cutter = CellSet(np.ones((1, 1), dtype=bool), (0, 0))

    return Problem(uid="block_1s1", region=region, cutter=cutter, k=1)


def wasteful_tour() -> np.ndarray:
    # a snake over the four rows, with a pointless out and back on the top row
    corners = [
        (0, 0), (7, 0), (7, 1), (0, 1), (0, 2), (7, 2), (7, 3),
        (2, 3), (6, 3), (0, 3),
    ]

    return densify(corners, closed=True)


def test_tiles_reach_every_position():
    tour = wasteful_tour()
    tiles = _tiles(tour, 3, random.Random(1))

    for x, y in tour.tolist():
        assert any(t.x0 <= x < t.x1 and t.y0 <= y < t.y1 for t in tiles)


def test_polish_shortens_and_keeps_coverage():
    problem = block_problem()
    tour = wasteful_tour()

    polished, stats = polish_tours(
        problem, [tour], deadline=time.monotonic() + 5, side=4, window_seconds=1.0
    )

    assert len(polished[0]) < len(tour)
    assert stats.improved >= 1
    assert len(list(problem.uncovered(polished[0]).cells())) == 0


def test_polishing_time_grows_with_the_instance():
    assert polish_seconds(20_000) < polish_seconds(1_000_000) < polish_seconds(5_000_000)
