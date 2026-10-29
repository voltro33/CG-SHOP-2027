"""
Lane segments skip gaps, and band partitions split the region cleanly.
"""

import numpy as np
import pytest

from cgshop2027_pyutils.verify import SolutionValidator
from solver.construct.segments import lane_segments, order_segments
from solver.model import Problem
from solver.partition import band_problems
from solver.pipeline import SolveConfig, solve

from tests.solver.test_pipeline import make_instance

# a U shape, two arms 4 wide with a 12 wide empty middle
U_SHAPE = [(0, 0), (20, 0), (20, 16), (16, 16), (16, 4), (4, 4), (4, 16), (0, 16)]

# a 3 by 3 square cutter centered on its middle cell
CUTTER = [(0, 0), (3, 0), (3, 3), (0, 3)]


def u_problem(k: int = 1) -> Problem:
    instance = make_instance(U_SHAPE, cutter=CUTTER, k=k, uid="u")
    return Problem.from_instance(instance)


def test_lanes_split_at_the_gap_of_a_u():
    problem = u_problem()

    # gap 0 means never bridge a gap
    segments = lane_segments(problem, max_gap=0)

    rows_with_two_pieces = 0
    for y in {s.y for s in segments}:
        pieces = [s for s in segments if s.y == y]
        if len(pieces) == 2:
            rows_with_two_pieces += 1

    assert rows_with_two_pieces >= 3


def test_huge_gap_gives_one_segment_per_lane():
    problem = u_problem()

    segments = lane_segments(problem, max_gap=10_000)

    lanes = [s.y for s in segments]
    assert len(lanes) == len(set(lanes))


def test_order_keeps_every_segment_once():
    problem = u_problem()
    segments = lane_segments(problem, max_gap=0)

    entries, exits = order_segments(segments, deadline=float("inf"))

    # each segment shows up once, in one of its two directions
    driven = set()
    for entry, exit_ in zip(entries.tolist(), exits.tolist(), strict=True):
        ends = tuple(sorted([tuple(entry), tuple(exit_)]))
        driven.add(ends)

    expected = set()
    for s in segments:
        expected.add(tuple(sorted([(s.a, s.y), (s.b, s.y)])))

    assert driven == expected


@pytest.mark.parametrize("axis", ["x", "y"])
@pytest.mark.parametrize("k", [2, 3])
def test_bands_are_disjoint_and_cover_the_region(axis, k):
    problem = u_problem(k)

    bands = band_problems(problem, [1.0] * k, axis)

    all_cells = []
    for band in bands:
        all_cells.extend(band.region.cells())

    # no cell twice, and every cell somewhere
    assert len(all_cells) == len(set(all_cells))
    assert set(all_cells) == set(problem.region.cells())


def test_equal_shares_give_roughly_equal_bands():
    problem = u_problem(2)

    bands = band_problems(problem, [1.0, 1.0], "x")
    sizes = [b.num_cells for b in bands]

    # bands are cut on whole columns, so allow one column of slack
    column_height = 16
    assert abs(sizes[0] - sizes[1]) <= 2 * column_height


@pytest.mark.parametrize("partition", ["bands-x", "bands-y"])
@pytest.mark.parametrize("k", [1, 2, 3])
def test_partitioned_solve_is_feasible(partition, k):
    instance = make_instance(U_SHAPE, cutter=CUTTER, k=k, uid="u")
    problem = Problem.from_instance(instance)
    config = SolveConfig(method="segments-h", partition=partition)

    attempt = solve(problem, SolutionValidator(instance), config, time_limit=5)

    assert attempt.feasible, attempt.errors
    assert len(attempt.solution.tours) == k


def test_segments_beat_plain_lanes_on_a_u():
    instance = make_instance(U_SHAPE, cutter=CUTTER, uid="u")
    problem = Problem.from_instance(instance)
    validator = SolutionValidator(instance)

    lanes = solve(problem, validator, "lanes-h", time_limit=5)
    segments = solve(
        problem, validator, SolveConfig("segments-h", {"max_gap": 0}), time_limit=5
    )

    assert segments.objective <= lanes.objective
    assert np.isfinite(segments.objective)
