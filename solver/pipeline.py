"""
The solve pipeline, one instance and one constructor at a time:

    construct  ->  repair  ->  prune walk  ->  split into k  ->  close  ->  prune tours

Every stage takes and returns plain data, so stages can be swapped, reordered
or repeated independently.
"""

import time
from dataclasses import dataclass

import numpy as np

from cgshop2027_pyutils.schemas import CGSHOP2027Solution
from cgshop2027_pyutils.verify import SolutionValidator

from .construct import CONSTRUCTORS
from .improve import prune_routes
from .model import Problem
from .repair import repair_walk
from .split import split_walk
from .tours import close_route, compress, densify, to_cutter_tour

__all__ = ["Attempt", "solve"]


@dataclass(frozen=True)
class Attempt:
    method: str
    solution: CGSHOP2027Solution | None
    errors: tuple[str, ...]
    seconds: float

    @property
    def feasible(self) -> bool:
        return self.solution is not None and not self.errors

    @property
    def objective(self) -> int | None:
        return self.solution.objective if self.feasible else None


def solve(
    problem: Problem, validator: SolutionValidator, method: str, time_limit: float
) -> Attempt:
    """
    Runs one constructor through the whole pipeline and verifies the result.
    Failures are reported in the attempt rather than raised, so one bad
    method never stops a batch run.
    """
    started = time.monotonic()
    try:
        solution = _build(problem, method, started + time_limit)
    except Exception as error:
        return Attempt(
            method, None, (f"{type(error).__name__}: {error}",), _since(started)
        )
    errors = tuple(validator.check_for_errors(solution))
    return Attempt(method, solution, errors, _since(started))


def _build(problem: Problem, method: str, deadline: float) -> CGSHOP2027Solution:
    walk = CONSTRUCTORS[method](problem)
    if not walk:
        msg = f"{method} produced an empty walk"
        raise ValueError(msg)
    positions = repair_walk(problem, densify(walk))

    # Polish the single walk first, so the split below balances the pruned work.
    walk_deadline = time.monotonic() + (deadline - time.monotonic()) / 2
    (walk,) = prune_routes(
        problem, [compress(positions)], closed=False, deadline=walk_deadline
    )

    positions = densify(walk)
    pieces = split_walk(positions, problem.k)
    tours = [close_route(compress(piece)) for piece in pieces]
    tours = prune_routes(problem, tours, closed=True, deadline=deadline)
    tours += [[_point(positions[0])]] * (problem.k - len(tours))  # idle cutters

    return CGSHOP2027Solution(
        instance_uid=problem.uid,
        tours=[to_cutter_tour(t) for t in tours],
        meta={"method": method},
    )


def _point(p: np.ndarray) -> tuple[int, int]:
    return (int(p[0]), int(p[1]))


def _since(started: float) -> float:
    return round(time.monotonic() - started, 2)
