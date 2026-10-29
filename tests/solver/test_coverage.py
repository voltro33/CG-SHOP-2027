"""
Cross-check: our coverage model (`Problem.uncovered`) against the official
verifier (`SolutionValidator.uncovered_cells`).

The official verifier is the source of truth. If these tests ever disagree,
the bug is in `solver/`, not in the verifier.

We compare the exact number of uncovered cells, not just "feasible or not",
because random tours almost never cover everything, and a count catches
off-by-one mistakes that a yes/no answer would hide.
"""

import random
from pathlib import Path

import numpy as np
import pytest

from cgshop2027_pyutils.instance_database import InstanceDatabase
from cgshop2027_pyutils.schemas import CGSHOP2027Solution, CutterTour
from cgshop2027_pyutils.verify import SolutionValidator
from cgshop2027_pyutils.zip import ZipSolutionIterator
from solver.model import Problem
from solver.tours import close_route, densify

EXAMPLE_INSTANCES = Path("example_instances.zip")
EXAMPLE_SOLUTIONS = Path("example_solutions.zip")

RANDOM_TOURS = 200
MAX_CELLS_FOR_RANDOM_TOURS = 20_000

needs_examples = pytest.mark.skipif(
    not EXAMPLE_INSTANCES.exists() or not EXAMPLE_SOLUTIONS.exists(),
    reason="example zips are not in the repo root",
)


def our_uncovered_count(problem: Problem, solution: CGSHOP2027Solution) -> int:
    """
    Uncovered cells according to our model: every lattice point of every
    tour (closed), dilated by the cutter.
    """
    all_positions = []
    for tour in solution.tours:
        corners = list(zip(tour.x, tour.y, strict=True))
        positions = densify(corners, closed=True)
        all_positions.append(positions)

    stacked = np.concatenate(all_positions)

    return len(problem.uncovered(stacked))


def random_tour(
    rng: random.Random, low: tuple[int, int], high: tuple[int, int]
) -> CutterTour:
    """
    A random closed tour inside the box `low..high`: random corners, made
    axis-parallel by `close_route`.
    """
    corners = []
    for _ in range(rng.randint(1, 8)):
        x = rng.randint(low[0], high[0])
        y = rng.randint(low[1], high[1])
        corners.append((x, y))

    # close_route adds bends so every step is axis parallel
    route = close_route(corners)

    xs = [point[0] for point in route]
    ys = [point[1] for point in route]

    return CutterTour(x=xs, y=ys)


@needs_examples
def test_shipped_example_solutions_verify_and_agree():
    """
    Phase 0 acceptance: every shipped solution passes the official verifier,
    and our model agrees that nothing is left uncovered.
    """
    database = InstanceDatabase(str(EXAMPLE_INSTANCES))

    checked = 0
    for solution in ZipSolutionIterator(str(EXAMPLE_SOLUTIONS)):
        instance = database[solution.instance_uid]

        errors = SolutionValidator(instance).check_for_errors(solution)
        assert errors == [], f"{solution.instance_uid}: {errors}"

        problem = Problem.from_instance(instance)
        assert our_uncovered_count(problem, solution) == 0

        checked += 1

    assert checked > 0


@needs_examples
def test_random_tours_match_official_uncovered_count():
    """
    Phase 2 acceptance: on random tours over small example instances, our
    uncovered-cell count equals the official one, every single time.
    """
    database = InstanceDatabase(str(EXAMPLE_INSTANCES))

    small_instances = []
    for instance in database:
        problem = Problem.from_instance(instance)
        if problem.num_cells <= MAX_CELLS_FOR_RANDOM_TOURS:
            small_instances.append((instance, problem))

    assert small_instances, "expected at least one small example instance"

    # fixed seed so a failure can be replayed exactly
    rng = random.Random(2027)
    mismatches = []

    for trial in range(RANDOM_TOURS):
        instance, problem = small_instances[trial % len(small_instances)]

        # reach a little outside the region since tours may leave it
        x0, y0, x1, y1 = problem.region.bounds
        low = (x0 - 3, y0 - 3)
        high = (x1 + 3, y1 + 3)

        tours = []
        for _ in range(instance.number_of_cutters):
            tours.append(random_tour(rng, low, high))

        solution = CGSHOP2027Solution(instance_uid=instance.instance_uid, tours=tours)

        official = len(SolutionValidator(instance).uncovered_cells(solution))
        ours = our_uncovered_count(problem, solution)

        if official != ours:
            mismatches.append((instance.instance_uid, trial, official, ours))

    assert mismatches == []
