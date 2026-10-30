"""
Coverage repair: patch a walk that misses some cells by adding short
out-and-back detours. Any constructor can be sloppy at the edges and rely on
this to become feasible.
"""

import numpy as np

from .model import Problem
from .tours import Point, connect, densify

__all__ = ["repair_tours", "repair_walk"]


def repair_walk(problem: Problem, positions: np.ndarray) -> np.ndarray:
    """
    Returns a new walk that sweeps every region cell. For each missed cell, the
    walk position closest to it gets a detour to the anchor in the cell's
    neighborhood closest to that position.
    """
    walk = np.asarray(positions, dtype=np.int64)
    missing = np.array(list(problem.uncovered(walk).cells()), dtype=np.int64)
    if len(missing) == 0:
        return walk
    offsets = problem.offsets
    detours: dict[int, list[Point]] = {}
    handled: set[Point] = set()
    for cell in missing:
        if (int(cell[0]), int(cell[1])) in handled:
            continue
        walk_index = int(np.argmin(np.abs(walk - cell).sum(axis=1)))
        candidates = cell - offsets  # anchors whose footprint holds this cell
        best = candidates[np.argmin(np.abs(candidates - walk[walk_index]).sum(axis=1))]
        anchor = (int(best[0]), int(best[1]))
        detours.setdefault(walk_index, []).append(anchor)
        handled.update((anchor[0] + dx, anchor[1] + dy) for dx, dy in offsets.tolist())
    return _insert_detours(walk, detours)


def repair_tours(problem: Problem, tours: list[np.ndarray]) -> list[np.ndarray]:
    """
    The same for several closed tours together: a cell counts as swept if any
    tour sweeps it, and each detour goes to the tour that passes closest.
    Detours go out and back, so every tour stays closed.
    """
    walks = [np.asarray(tour, dtype=np.int64) for tour in tours]
    joined = np.concatenate(walks)
    missing = np.array(list(problem.uncovered(joined).cells()), dtype=np.int64)
    if len(missing) == 0:
        return walks

    # which tour and which position every joined position comes from
    owner = np.concatenate([np.full(len(walk), index) for index, walk in enumerate(walks)])
    position = np.concatenate([np.arange(len(walk)) for walk in walks])

    offsets = problem.offsets
    detours: list[dict[int, list[Point]]] = [{} for _ in walks]
    handled: set[Point] = set()
    for cell in missing:
        if (int(cell[0]), int(cell[1])) in handled:
            continue

        nearest = int(np.argmin(np.abs(joined - cell).sum(axis=1)))
        candidates = cell - offsets
        best = candidates[np.argmin(np.abs(candidates - joined[nearest]).sum(axis=1))]
        anchor = (int(best[0]), int(best[1]))

        tour_detours = detours[int(owner[nearest])]
        tour_detours.setdefault(int(position[nearest]), []).append(anchor)
        handled.update((anchor[0] + dx, anchor[1] + dy) for dx, dy in offsets.tolist())

    result = []
    for walk, tour_detours in zip(walks, detours, strict=True):
        result.append(_insert_detours(walk, tour_detours) if tour_detours else walk)

    return result


def _insert_detours(walk: np.ndarray, detours: dict[int, list[Point]]) -> np.ndarray:
    chunks = []
    previous = 0
    for index in sorted(detours):
        chunks.append(walk[previous : index + 1])
        home = (int(walk[index][0]), int(walk[index][1]))
        trip = connect([home, *detours[index], home])
        chunks.append(densify(trip)[1:])
        previous = index + 1
    chunks.append(walk[previous:])
    return np.concatenate([c for c in chunks if len(c)])
