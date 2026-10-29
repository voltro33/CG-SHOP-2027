"""
End-to-end: every constructor, through the whole pipeline, must give a
solution that the official verifier accepts and that respects the lower bound.
"""

import time

import numpy as np
import pytest

from cgshop2027_pyutils.schemas import CGSHOP2027Instance
from cgshop2027_pyutils.verify import SolutionValidator
from solver.bounds import area_bound, extent_bound, lower_bound
from solver.construct import CONSTRUCTORS
from solver.construct.cover_tsp import greedy_cover
from solver.improve import prune_routes
from solver.model import Problem
from solver.pipeline import solve
from solver.repair import repair_walk
from solver.split import closed_cost, split_walk
from solver.tours import compress, densify, route_length

# -- small hand-made instances ---------------------------------------------


def make_instance(
    outer: list[tuple[int, int]],
    *,
    holes: tuple[list[tuple[int, int]], ...] = (),
    cutter: list[tuple[int, int]] = ((0, 0), (2, 0), (2, 2), (0, 2)),
    center: tuple[int, int] = (1, 1),
    k: int = 1,
    uid: str = "test",
) -> CGSHOP2027Instance:
    def ring(points):
        return {"x": [p[0] for p in points], "y": [p[1] for p in points]}

    return CGSHOP2027Instance(
        instance_uid=uid,
        region_to_cover={
            "outer_boundary": ring(outer),
            "inner_boundaries": [ring(h) for h in holes],
        },
        cutter=ring(list(cutter)),
        cutter_center=center,
        number_of_cutters=k,
    )


SQUARE = [(0, 0), (12, 0), (12, 10), (0, 10)]
L_SHAPE = [(0, 0), (14, 0), (14, 4), (4, 4), (4, 12), (0, 12)]
HOLE = [(4, 4), (4, 6), (8, 6), (8, 4)]  # clockwise
PLUS = [
    (1, 0),
    (2, 0),
    (2, 1),
    (3, 1),
    (3, 2),
    (2, 2),
    (2, 3),
    (1, 3),
    (1, 2),
    (0, 2),
    (0, 1),
    (1, 1),
]


CASES = {
    "square-2x2-k1": make_instance(SQUARE),
    "square-2x2-k3": make_instance(SQUARE, k=3),
    "lshape-plus-k2": make_instance(L_SHAPE, cutter=PLUS, center=(1, 1), k=2),
    "hole-2x2-k2": make_instance(SQUARE, holes=(HOLE,), k=2),
}


@pytest.fixture(params=sorted(CASES))
def instance(request) -> CGSHOP2027Instance:
    return CASES[request.param]


@pytest.fixture
def problem(instance) -> Problem:
    return Problem.from_instance(instance)


@pytest.mark.parametrize("method", sorted(CONSTRUCTORS))
def test_pipeline_is_feasible_and_above_bound(instance, problem, method):
    attempt = solve(problem, SolutionValidator(instance), method, time_limit=5)
    assert attempt.errors == ()
    assert len(attempt.solution.tours) == instance.number_of_cutters
    assert attempt.objective >= lower_bound(problem)


@pytest.mark.parametrize("method", sorted(CONSTRUCTORS))
def test_constructor_plus_repair_covers_region(problem, method):
    walk = repair_walk(problem, densify(CONSTRUCTORS[method](problem)))
    assert len(problem.uncovered(walk)) == 0


def test_greedy_cover_footprints_cover_region(problem):
    anchors = greedy_cover(problem)
    assert len(problem.uncovered(anchors)) == 0


def test_repair_fixes_a_walk_that_covers_nothing(problem):
    lonely = np.array([[-50, -50]])
    assert len(problem.uncovered(repair_walk(problem, lonely))) == 0


@pytest.mark.parametrize("k", [1, 2, 3, 5])
def test_split_keeps_every_position_once(k):
    walk = densify([(0, 0), (20, 0), (20, 3), (0, 3), (0, 6), (20, 6)])
    pieces = split_walk(walk, k)
    assert 1 <= len(pieces) <= k
    assert np.array_equal(np.concatenate(pieces), walk)


def test_split_balances_pieces():
    walk = densify([(0, 0), (30, 0), (30, 2), (0, 2)])
    pieces = split_walk(walk, 2)
    costs = [closed_cost(p, 0, len(p) - 1) for p in pieces]
    single = closed_cost(walk, 0, len(walk) - 1)
    assert max(costs) < single
    assert max(costs) - min(costs) <= 4


def test_prune_never_breaks_coverage_or_grows(problem):
    walk = repair_walk(problem, densify(CONSTRUCTORS["cover-tsp"](problem)))
    before = compress(walk)
    (after,) = prune_routes(
        problem, [before], closed=False, deadline=time.monotonic() + 5
    )
    assert route_length(after, closed=False) <= route_length(before, closed=False)
    assert len(problem.uncovered(densify(after))) == 0


def test_bounds_on_known_square(problem):
    assert area_bound(problem) >= 0
    assert extent_bound(problem) >= 0
    if problem.k > 1:
        assert extent_bound(problem) == 0
