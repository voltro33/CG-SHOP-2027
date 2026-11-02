"""
Group first, then close: k closed tours straight from the lane segments.

The default pipeline orders all segments as one open walk and cuts it into
k pieces afterwards. Each piece then has to drive from its end back to its
start, a return trip the order search never saw: on the benchmark it is 3
to 13% of every tour when k is 2 or 3, against at most 1.3% for k = 1, where
the order search optimizes the closed tour itself.

Here the cut comes before the closing:

1. Order all segments as one open walk, as before.
2. Cut that order into k consecutive groups, pricing every group with its
   return trip, so the longest closed group is as short as possible.
3. Order each group again as a closed tour, which turns its return trip
   into ordinary links the search can shorten.
"""

import time

import numpy as np

from ..model import Problem
from ..tours import Point, close_route, connect
from .ordering import improve_order
from .segments import _nearest_neighbor, _two_opt, auto_kick_seconds, lane_segments

__all__ = ["group_order", "groups_horizontal", "groups_vertical"]


def groups_horizontal(
    problem: Problem,
    *,
    gap_factor: float = 0.0,
    shift: float = 0.0,
    kick_seconds: float | None = None,
    seed: int = 0,
    time_limit: float = 10.0,
) -> list[list[Point]]:
    """
    Up to k closed tours over horizontal lane segments.
    """
    segments = lane_segments(problem, gap_factor=gap_factor, shift=shift)
    if not segments:
        return []

    left = np.array([(s.a, s.y) for s in segments], dtype=np.int64)
    right = np.array([(s.b, s.y) for s in segments], dtype=np.int64)

    if kick_seconds is None:
        kick_seconds = auto_kick_seconds(len(left))

    deadline = time.monotonic() + time_limit + 2 * kick_seconds
    entries, exits = _nearest_neighbor(left, right)
    entries, exits = _two_opt(entries, exits, deadline=deadline, window=60)

    entries, exits = improve_order(
        entries,
        exits,
        closed=problem.k == 1,
        deadline=deadline,
        kick_seconds=kick_seconds,
        seed=seed,
    )

    groups = []
    for first, last in group_order(entries, exits, problem.k):
        groups.append((entries[first : last + 1], exits[first : last + 1]))

    # the kick time is shared, each group gets its share, once before
    # rebalancing and once after
    groups = _close_groups(groups, len(entries), kick_seconds / 2, deadline, seed)
    groups = rebalance(groups, deadline)
    groups = _close_groups(groups, len(entries), kick_seconds / 2, deadline, seed + 1)

    tours = []
    for group_entries, group_exits in groups:
        tours.append(_closed_tour(group_entries, group_exits))

    return tours


def rebalance(
    groups: list[tuple[np.ndarray, np.ndarray]], deadline: float
) -> list[tuple[np.ndarray, np.ndarray]]:
    """
    Moves single segments out of the longest closed group into the cheapest
    slot of another group, as long as that shortens the longest group.
    Returns new groups.
    """
    groups = [(entries.copy(), exits.copy()) for entries, exits in groups]
    if len(groups) < 2:
        return groups

    for _ in range(MAX_MOVES):
        if time.monotonic() >= deadline:
            break

        move = _best_move(groups)
        if move is None:
            break

        groups = _apply_move(groups, move)

    return groups


# how many single segment moves one rebalance may make
MAX_MOVES = 500

# segments of the longest group priced at once, bounds memory
MOVE_CHUNK = 256


def _close_groups(groups, total: int, kick_seconds: float, deadline: float, seed: int):
    result = []
    for group_entries, group_exits in groups:
        group_kick = kick_seconds * len(group_entries) / total
        closed = improve_order(
            group_entries,
            group_exits,
            closed=True,
            deadline=deadline,
            kick_seconds=group_kick,
            seed=seed,
        )
        result.append(closed)

    return result


def _closed_length(entries: np.ndarray, exits: np.ndarray) -> int:
    across = np.abs(exits - entries).sum()
    links = np.abs(np.roll(entries, -1, axis=0) - exits).sum()

    return int(across + links)


def _best_move(groups):
    """
    The single segment move that shortens the longest group the most, as
    (from group, position, to group, slot, flipped), or None.
    """
    lengths = [_closed_length(entries, exits) for entries, exits in groups]
    longest = int(np.argmax(lengths))
    current = lengths[longest]

    entries, exits = groups[longest]
    if len(entries) < 2:
        return None

    # what taking each segment out saves, linking its neighbours directly
    before = np.roll(exits, 1, axis=0)
    after = np.roll(entries, -1, axis=0)
    across = np.abs(exits - entries).sum(axis=1)
    saving = (
        np.abs(entries - before).sum(axis=1)
        + across
        + np.abs(after - exits).sum(axis=1)
        - np.abs(after - before).sum(axis=1)
    )

    best = None
    best_peak = current
    for target, (target_entries, target_exits) in enumerate(groups):
        if target == longest:
            continue

        # slot j sits between the exit of segment j and the entry of j + 1
        slot_from = target_exits
        slot_to = np.roll(target_entries, -1, axis=0)
        slot_link = np.abs(slot_to - slot_from).sum(axis=1)

        others = [length for index, length in enumerate(lengths) if index not in (longest, target)]
        floor = max(others) if others else 0

        for start in range(0, len(entries), MOVE_CHUNK):
            chunk = slice(start, start + MOVE_CHUNK)
            a = entries[chunk][:, None, :]
            b = exits[chunk][:, None, :]

            # driven as is, or flipped
            straight = np.abs(a - slot_from).sum(axis=2) + np.abs(slot_to - b).sum(axis=2)
            flipped = np.abs(b - slot_from).sum(axis=2) + np.abs(slot_to - a).sum(axis=2)
            cheaper = np.minimum(straight, flipped) - slot_link

            slot = np.argmin(cheaper, axis=1)
            added = cheaper[np.arange(len(slot)), slot] + across[chunk]

            shrunk = current - saving[chunk]
            grown = lengths[target] + added
            peak = np.maximum(np.maximum(shrunk, grown), floor)

            pick = int(np.argmin(peak))
            if peak[pick] < best_peak:
                best_peak = int(peak[pick])
                position = start + pick
                is_flipped = bool(flipped[pick, slot[pick]] < straight[pick, slot[pick]])
                best = (longest, position, target, int(slot[pick]), is_flipped)

    return best


def _apply_move(groups, move):
    source, position, target, slot, flipped = move

    entries, exits = groups[source]
    entry = entries[position]
    exit_ = exits[position]
    if flipped:
        entry, exit_ = exit_, entry

    result = list(groups)
    result[source] = (np.delete(entries, position, axis=0), np.delete(exits, position, axis=0))

    target_entries, target_exits = groups[target]
    result[target] = (
        np.insert(target_entries, slot + 1, entry, axis=0),
        np.insert(target_exits, slot + 1, exit_, axis=0),
    )

    return result


def groups_vertical(problem: Problem, **params) -> list[list[Point]]:
    """
    The same over vertical lanes, by solving the mirrored problem.
    """
    tours = groups_horizontal(problem.transposed(), **params)

    # mirror back across the diagonal
    return [close_route([(y, x) for x, y in tour]) for tour in tours]


def group_order(entries: np.ndarray, exits: np.ndarray, k: int) -> list[tuple[int, int]]:
    """
    Cuts an order of segments into at most k consecutive groups, as first
    and last index, so the longest group, closed back to its first entry,
    is as short as the greedy finds.
    """
    n = len(entries)
    if k <= 1 or n <= 1:
        return [(0, n - 1)]

    # driven length from the first entry to every entry and exit, in order
    across = np.abs(exits - entries).sum(axis=1)
    links = np.concatenate(([0], np.abs(entries[1:] - exits[:-1]).sum(axis=1)))
    at_exit = np.cumsum(links + across)
    at_entry = at_exit - across

    low = 0
    high = _closed_cost(entries, exits, at_entry, at_exit, first=0, last=n - 1)
    best = _greedy(entries, exits, at_entry, at_exit, k=k, target=high)

    while low < high:
        target = (low + high) // 2
        groups = _greedy(entries, exits, at_entry, at_exit, k=k, target=target)
        if groups is None:
            low = target + 1
        else:
            best = groups
            high = target

    return best


def _closed_cost(entries, exits, at_entry, at_exit, *, first: int, last: int) -> int:
    inside = int(at_exit[last] - at_entry[first])
    home = int(np.abs(entries[first] - exits[last]).sum())

    return inside + home


def _greedy(
    entries, exits, at_entry, at_exit, *, k: int, target: int
) -> list[tuple[int, int]] | None:
    """
    Groups as long as `target` allows, or None if k groups are not enough.
    """
    n = len(entries)
    groups = []
    first = 0

    while first < n:
        if len(groups) == k:
            return None

        # closed cost of every group starting at `first`
        inside = at_exit[first:] - at_entry[first]
        home = np.abs(exits[first:] - entries[first]).sum(axis=1)
        fits = np.flatnonzero(inside + home <= target)

        # a single segment always forms a group, even above the target
        last = first + (int(fits[-1]) if len(fits) else 0)
        groups.append((first, last))
        first = last + 1

    return groups


def _closed_tour(entries: np.ndarray, exits: np.ndarray) -> list[Point]:
    corners: list[Point] = []
    for entry, exit_ in zip(entries.tolist(), exits.tolist(), strict=True):
        corners.append((entry[0], entry[1]))
        corners.append((exit_[0], exit_[1]))

    return close_route(connect(corners))
