"""
The breakdown must add up to the tour length exactly, and put known waste
in the right part.
"""

import numpy as np
import pytest

from cgshop2027_pyutils.grid import CellSet
from cgshop2027_pyutils.schemas import CGSHOP2027Solution, CutterTour

from solver.diagnose import diagnose, lane_gain
from solver.model import Problem


def one_row_problem() -> Problem:
    """
    A region one cell high and six wide, swept by a 2 by 2 cutter whose
    top row always hangs outside the region.
    """
    region = CellSet(np.ones((1, 6), dtype=bool), (0, 0))
    cutter = CellSet(np.ones((2, 2), dtype=bool), (0, 0))

    return Problem(uid="row_1s2", region=region, cutter=cutter, k=1)


def there_and_back() -> CGSHOP2027Solution:
    # right from 0 to 4 sweeps the row, the way back sweeps nothing
    tour = CutterTour(x=[0, 4], y=[0, 0])

    return CGSHOP2027Solution(instance_uid="row_1s2", tours=[tour])


def test_one_lane_overshoots_half_of_every_step():
    result = diagnose(one_row_problem(), there_and_back())

    # every step to the right sweeps one region cell and one outside cell
    assert result.total_length == 8
    assert result.productive == pytest.approx(2.0)
    assert result.overshoot == pytest.approx(2.0)

    # every step back is dead
    assert result.dead == pytest.approx(4.0)
    assert result.dead_steps == 4

    assert result.near_resweep == 0.0
    assert result.far_resweep == 0.0


def test_parts_add_up_and_every_cell_is_counted_once():
    result = diagnose(one_row_problem(), there_and_back())

    summed = sum(result.shares().values())
    assert summed == pytest.approx(1.0)

    # two cells under the first footprint, four from steps
    assert result.start_cells == 2
    assert result.new_cells == 4
    assert result.lost == pytest.approx(6.0)


def test_lane_overheads_of_one_short_lane():
    result = diagnose(one_row_problem(), there_and_back())

    # the lane is four steps and the span is two, so it is all lane end
    assert result.lanes == 1
    assert result.overshoot_at_lane_ends == pytest.approx(2.0)

    # four dead steps in a row is more than a span and less than four spans
    assert result.dead_short == 0.0
    assert result.dead_travel == 0.0


def test_incomplete_coverage_is_refused():
    # stopping at 3 never reaches the last cell
    short = CGSHOP2027Solution(instance_uid="row_1s2", tours=[CutterTour(x=[0, 3], y=[0, 0])])

    with pytest.raises(AssertionError):
        diagnose(one_row_problem(), short)


def test_lane_gain_counts_rows_of_an_x_cutter():
    # an X three wide touches three rows but sweeps five fresh cells per step
    mask = np.array([[1, 0, 1], [0, 1, 0], [1, 0, 1]], dtype=bool)
    cutter = CellSet(mask, (-1, -1))
    region = CellSet(np.ones((5, 5), dtype=bool), (0, 0))
    problem = Problem(uid="x_1x3", region=region, cutter=cutter, k=1)

    assert lane_gain(problem) == 3
    assert problem.step_gain() == 5
