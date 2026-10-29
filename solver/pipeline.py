"""
The solve pipeline, one instance and one configuration at a time.

Default route, one walk for everybody:

    construct  ->  repair  ->  prune walk  ->  split into k  ->  close  ->  prune tours

Partitioned route (`partition="bands-x"` or `"bands-y"`), see `partition.py`:

    cut into k bands  ->  per band: construct, repair, prune, close, prune  ->  rebalance  ->  prune tours

Every stage takes and returns plain data, so stages can be swapped, reordered
or repeated independently.
"""

import time
from collections.abc import Mapping
from dataclasses import dataclass, field
from typing import Any

import numpy as np

from cgshop2027_pyutils.schemas import CGSHOP2027Solution
from cgshop2027_pyutils.verify import SolutionValidator

from .construct import CONSTRUCTORS
from .improve import prune_routes
from .model import Problem
from .partition import solve_in_bands
from .repair import repair_walk
from .split import split_walk
from .tours import Point, close_route, compress, densify, to_cutter_tour

__all__ = ["Attempt", "SolveConfig", "solve"]

PARTITIONS = ("bands-x", "bands-y")


@dataclass(frozen=True)
class SolveConfig:
    """
    Everything that decides how one instance gets solved, so a result can
    always be reproduced from its config.
    """

    method: str
    params: Mapping[str, Any] = field(default_factory=dict)
    partition: str | None = None
    rebalance_rounds: int = 4

    def label(self) -> str:
        parts = [self.method]

        for key in sorted(self.params):
            parts.append(f"{key}={self.params[key]}")

        if self.partition:
            parts.append(self.partition)

        return "/".join(parts)


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
    problem: Problem,
    validator: SolutionValidator,
    config: SolveConfig | str,
    time_limit: float,
) -> Attempt:
    """
    Runs one configuration through the whole pipeline and verifies the
    result. Failures are reported in the attempt rather than raised, so one
    bad method never stops a batch run. A bare method name still works.
    """
    if isinstance(config, str):
        config = SolveConfig(method=config)

    label = config.label()
    started = time.monotonic()
    deadline = started + time_limit

    try:
        if config.partition is None:
            tours = _tours_single_walk(problem, config, deadline)
        else:
            tours = _tours_partitioned(problem, config, deadline)

        solution = CGSHOP2027Solution(
            instance_uid=problem.uid,
            tours=[to_cutter_tour(t) for t in tours],
            meta={"method": label},
        )
    except Exception as error:
        message = f"{type(error).__name__}: {error}"
        return Attempt(label, None, (message,), _since(started))

    # the official verifier has the last word on every solution
    errors = tuple(validator.check_for_errors(solution))

    return Attempt(label, solution, errors, _since(started))


def build_walk(problem: Problem, config: SolveConfig) -> np.ndarray:
    """
    Construct plus repair: unit step positions of an open walk that sweeps
    every region cell.
    """
    constructor = CONSTRUCTORS[config.method]
    walk = constructor(problem, **config.params)

    if not walk:
        msg = f"{config.method} produced an empty walk"
        raise ValueError(msg)

    return repair_walk(problem, densify(walk))


def _tours_single_walk(
    problem: Problem, config: SolveConfig, deadline: float
) -> list[list[Point]]:
    positions = build_walk(problem, config)

    # polish the single walk first so the split balances the pruned work
    walk_deadline = time.monotonic() + (deadline - time.monotonic()) / 2
    (walk,) = prune_routes(
        problem, [compress(positions)], closed=False, deadline=walk_deadline
    )

    positions = densify(walk)
    pieces = split_walk(positions, problem.k)

    tours = []
    for piece in pieces:
        tours.append(close_route(compress(piece)))

    tours = prune_routes(problem, tours, closed=True, deadline=deadline)

    return _pad_with_idle(tours, problem.k, _point(positions[0]))


def _tours_partitioned(
    problem: Problem, config: SolveConfig, deadline: float
) -> list[list[Point]]:
    if config.partition not in PARTITIONS:
        msg = f"unknown partition {config.partition!r}, choose from {PARTITIONS}"
        raise ValueError(msg)

    axis = config.partition[-1]

    # each round solves all k bands, so give one round a slice of the time
    rounds = max(1, config.rebalance_rounds)
    band_seconds = (deadline - time.monotonic()) / (2 * rounds * problem.k)

    def solve_band(band: Problem) -> list[Point]:
        positions = build_walk(band, config)

        band_deadline = time.monotonic() + band_seconds
        closed = close_route(compress(positions))
        (tour,) = prune_routes(band, [closed], closed=True, deadline=band_deadline)

        return tour

    def out_of_time() -> bool:
        return time.monotonic() >= deadline

    tours = solve_in_bands(
        problem, solve_band, axis=axis, rounds=rounds, out_of_time=out_of_time
    )

    # bands overlap a little at their borders, so a joint prune can cut more
    tours = prune_routes(problem, tours, closed=True, deadline=deadline)

    return _pad_with_idle(tours, problem.k, tours[0][0])


def _pad_with_idle(
    tours: list[list[Point]], k: int, home: Point
) -> list[list[Point]]:
    """
    Adds one point tours for cutters that got no work, since the solution
    must always have exactly k tours.
    """
    padded = list(tours)

    while len(padded) < k:
        padded.append([home])

    return padded


def _point(p: np.ndarray) -> Point:
    return (int(p[0]), int(p[1]))


def _since(started: float) -> float:
    return round(time.monotonic() - started, 2)
