"""
Groups must price their way home, rebalancing must never make the longest
group longer, and the k tours together must still sweep every cell.
"""

import numpy as np

from cgshop2027_pyutils.grid import CellSet

from solver.construct.groups import (
    _closed_length,
    group_order,
    groups_horizontal,
    rebalance,
)
from solver.model import Problem
from solver.repair import repair_tours
from solver.tours import densify


def segments_in_a_row(count: int) -> tuple[np.ndarray, np.ndarray]:
    """
    Unit long segments one apart along the x axis, driven left to right.
    """
    entries = np.array([(2 * index, 0) for index in range(count)], dtype=np.int64)
    exits = entries + np.array([1, 0])

    return entries, exits


def test_groups_cover_the_order_once():
    entries, exits = segments_in_a_row(10)
    groups = group_order(entries, exits, 3)

    covered = []
    for first, last in groups:
        covered.extend(range(first, last + 1))

    assert covered == list(range(10))
    assert len(groups) <= 3


def test_one_cutter_keeps_one_group():
    entries, exits = segments_in_a_row(5)

    assert group_order(entries, exits, 1) == [(0, 4)]


def test_two_groups_split_a_row_in_the_middle():
    # every group drives out and back, so halves are the balanced cut
    entries, exits = segments_in_a_row(8)

    assert group_order(entries, exits, 2) == [(0, 3), (4, 7)]


def test_rebalance_never_lengthens_the_longest_group():
    entries, exits = segments_in_a_row(12)
    groups = [(entries[:10], exits[:10]), (entries[10:], exits[10:])]
    before = max(_closed_length(e, x) for e, x in groups)

    balanced = rebalance(groups, deadline=float("inf"))
    after = max(_closed_length(e, x) for e, x in balanced)

    assert after < before
    assert sum(len(e) for e, _ in balanced) == 12


def test_repair_reaches_cells_no_tour_sweeps():
    mask = np.ones((3, 6), dtype=bool)
    region = CellSet(mask, (0, 0))
    cutter = CellSet(np.ones((1, 1), dtype=bool), (0, 0))
    problem = Problem(uid="strip_2s1", region=region, cutter=cutter, k=2)

    # two tours over the bottom row only, the top two rows are missed
    left = densify([(0, 0), (2, 0)], closed=True)
    right = densify([(3, 0), (5, 0)], closed=True)

    repaired = repair_tours(problem, [left, right])

    assert len(list(problem.uncovered(np.concatenate(repaired)).cells())) == 0
    assert len(repaired) == 2


def test_two_cutters_give_two_closed_tours_over_everything():
    mask = np.ones((4, 8), dtype=bool)
    region = CellSet(mask, (0, 0))
    cutter = CellSet(np.ones((1, 1), dtype=bool), (0, 0))
    problem = Problem(uid="block_2s1", region=region, cutter=cutter, k=2)

    tours = groups_horizontal(problem, kick_seconds=0.1, time_limit=0.1)
    walks = [densify(tour, closed=True) for tour in tours]

    assert len(tours) == 2
    assert len(list(problem.uncovered(np.concatenate(walks)).cells())) == 0
