"""
Phase 5 acceptance: a lower bound must never exceed a verified solution.

If one ever does, the bound is wrong, not the solution: the solution passed
the official verifier.
"""

from pathlib import Path

import pytest

from cgshop2027_pyutils.instance_database import InstanceDatabase
from cgshop2027_pyutils.io import read_solution
from cgshop2027_pyutils.verify import SolutionValidator
from cgshop2027_pyutils.zip import ZipSolutionIterator
from solver.bounds import lower_bound
from solver.model import Problem

EXAMPLE_INSTANCES = Path("example_instances.zip")
EXAMPLE_SOLUTIONS = Path("example_solutions.zip")

CHALLENGE_INSTANCES = Path("data/instances/challenge_v0/instances")
BEST_SOLUTIONS = Path("data/solutions/best")


@pytest.mark.skipif(not EXAMPLE_SOLUTIONS.exists(), reason="no example zips")
def test_bound_below_every_shipped_example_solution():
    database = InstanceDatabase(str(EXAMPLE_INSTANCES))

    for solution in ZipSolutionIterator(str(EXAMPLE_SOLUTIONS)):
        problem = Problem.from_instance(database[solution.instance_uid])

        assert lower_bound(problem) <= solution.objective, solution.instance_uid


def _stored_best_files() -> list[Path]:
    if not BEST_SOLUTIONS.exists():
        return []

    return sorted(BEST_SOLUTIONS.glob("*.solution.json"))


@pytest.mark.skipif(not _stored_best_files(), reason="no mined solutions yet")
def test_bound_below_every_mined_best_solution():
    databases = []
    for source in (EXAMPLE_INSTANCES, CHALLENGE_INSTANCES):
        if source.exists():
            databases.append(InstanceDatabase(str(source)))

    for path in _stored_best_files():
        solution = read_solution(path)

        # find the instance in whichever source holds it
        instance = None
        for database in databases:
            try:
                instance = database[solution.instance_uid]
                break
            except (KeyError, ValueError):
                continue

        if instance is None:
            continue

        # only verified solutions count as evidence against the bound
        if SolutionValidator(instance).check_for_errors(solution):
            continue

        problem = Problem.from_instance(instance)
        assert lower_bound(problem) <= solution.objective, solution.instance_uid
