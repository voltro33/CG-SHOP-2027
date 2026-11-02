"""
The window map must count every step once, and give a window only the cells
nothing outside it sweeps.
"""

import numpy as np
import pytest

from cgshop2027_pyutils.grid import CellSet
from cgshop2027_pyutils.schemas import CGSHOP2027Solution, CutterTour
from solver.model import Problem
from solver.window_map import summarize, window_map


def one_row_problem() -> Problem:
    """
    A region one cell high and six wide, swept by a 2 by 2 cutter.
    """
    region = CellSet(np.ones((1, 6), dtype=bool), (0, 0))
    cutter = CellSet(np.ones((2, 2), dtype=bool), (0, 0))

    return Problem(uid="row_1s2", region=region, cutter=cutter, k=1)


def there_and_back() -> CGSHOP2027Solution:
    # anchors 0 to 4 and back, eight unit steps
    tour = CutterTour(x=[0, 4], y=[0, 0])

    return CGSHOP2027Solution(instance_uid="row_1s2", tours=[tour])


def test_one_window_needs_every_cell():
    result = window_map(one_row_problem(), there_and_back(), side=100)

    assert result.length.sum() == 8
    assert result.need.sum() == 6
    assert result.pieces.sum() == 1

    # eight steps against six cells at two per step
    assert result.room.sum() == pytest.approx(5.0)


def test_cells_swept_from_two_windows_belong_to_neither():
    result = window_map(one_row_problem(), there_and_back(), side=1)

    # one window per anchor; the turning points are visited once, the rest twice
    assert result.length.ravel().tolist() == [1, 2, 2, 2, 1]
    assert result.pieces.ravel().tolist() == [1, 2, 2, 2, 1]

    # only the two end cells have a single sweeping anchor
    assert result.need.ravel().tolist() == [1, 0, 0, 0, 1]
    assert result.room.sum() == pytest.approx(7.0)


def test_summary_shares_are_shares():
    problem = one_row_problem()
    summary = summarize(
        problem, window_map(problem, there_and_back(), side=1), side_spans=1
    )

    assert summary.windows == 5
    assert summary.room_share == pytest.approx(7 / 8)
    assert 0.0 < summary.worst_10_share <= summary.worst_25_share <= 1.0
