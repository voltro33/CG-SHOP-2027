"""
A window re-solve must find known savings, keep the tour one closed walk,
and never lose coverage.
"""

import numpy as np

from cgshop2027_pyutils.grid import CellSet
from solver.model import Problem
from solver.tours import densify
from solver.window import Window, cut_pieces, solve_window


def strip_problem() -> Problem:
    """
    A region of one row, five cells long, and a one cell cutter.
    """
    region = CellSet(np.ones((1, 5), dtype=bool), (0, 0))
    cutter = CellSet(np.ones((1, 1), dtype=bool), (0, 0))

    return Problem(uid="strip_1s1", region=region, cutter=cutter, k=1)


def loop_around(corners) -> np.ndarray:
    return densify(corners, closed=True)


# the tour runs along the row, steps outside at x = 5, and comes back two rows up
STRAIGHT = [(-1, 0), (5, 0), (5, 2), (-1, 2)]

# the same, with a two step dent on the way back
DENTED = [(-1, 0), (5, 0), (5, 2), (3, 2), (3, 1), (2, 1), (2, 2), (-1, 2)]

WINDOW = Window(x0=0, y0=0, x1=5, y1=3)


def is_closed_walk(positions: np.ndarray) -> bool:
    steps = np.abs(np.diff(np.concatenate([positions, positions[:1]]), axis=0)).sum(
        axis=1
    )
    return bool(np.all(steps == 1))


def test_cut_finds_both_passes_through_the_window():
    pieces = cut_pieces(loop_around(STRAIGHT), WINDOW)

    assert len(pieces) == 2


def test_a_tour_inside_the_window_has_nothing_to_keep():
    assert cut_pieces(loop_around(STRAIGHT), Window(-5, -5, 10, 10)) is None


def test_the_dent_is_removed():
    problem = strip_problem()
    tour = loop_around(DENTED)

    result = solve_window(problem, [tour], 0, WINDOW, time_limit=10, workers=1)

    # the way back takes six steps inside the window with the dent, four without
    assert result.old_steps == 4 + 6
    assert result.new_steps == 4 + 4
    assert result.status == "OPTIMAL"

    new_tour = result.positions
    assert len(new_tour) == len(tour) - 2
    assert is_closed_walk(new_tour)
    assert len(problem.uncovered(new_tour)) == 0


def test_an_optimal_window_stays_as_long():
    problem = strip_problem()
    tour = loop_around(STRAIGHT)

    result = solve_window(problem, [tour], 0, WINDOW, time_limit=10, workers=1)

    assert result.saved == 0
    assert len(result.positions) == len(tour)
    assert len(problem.uncovered(result.positions)) == 0
