"""
The order search may only reorder and flip segments, never lose one, and
must never return a longer order than it was given.
"""

import time

import numpy as np
import pytest

from solver.construct.ordering import improve_order, nearest_ends, order_length


def random_segments(count: int, seed: int) -> tuple[np.ndarray, np.ndarray]:
    """
    Horizontal segments scattered over a grid, in a random starting order.
    """
    rng = np.random.default_rng(seed)

    ys = rng.integers(0, 200, size=count) * 3
    starts = rng.integers(0, 300, size=count)
    lengths = rng.integers(0, 40, size=count)

    entries = np.stack([starts, ys], axis=1)
    exits = np.stack([starts + lengths, ys], axis=1)

    return entries, exits


def as_lists(entries: np.ndarray, exits: np.ndarray):
    ent = [tuple(p) for p in entries.tolist()]
    ext = [tuple(p) for p in exits.tolist()]
    return ent, ext


def unordered_segments(entries: np.ndarray, exits: np.ndarray) -> list:
    """
    Each segment as its sorted pair of ends, so direction does not matter.
    """
    pairs = []
    for a, b in zip(entries.tolist(), exits.tolist(), strict=True):
        pairs.append(tuple(sorted([tuple(a), tuple(b)])))

    return sorted(pairs)


@pytest.mark.parametrize("closed", [False, True])
@pytest.mark.parametrize("seed", [0, 1, 2])
def test_improve_order_keeps_segments_and_never_gets_longer(closed, seed):
    entries, exits = random_segments(150, seed)

    before = order_length(*as_lists(entries, exits), closed)

    new_entries, new_exits = improve_order(
        entries, exits, closed=closed, deadline=time.monotonic() + 5, kick_seconds=1.0
    )

    after = order_length(*as_lists(new_entries, new_exits), closed)

    assert unordered_segments(new_entries, new_exits) == unordered_segments(entries, exits)
    assert after <= before

    # a random order is terrible, any search should do much better
    assert after < 0.8 * before


def test_improve_order_leaves_inputs_untouched():
    entries, exits = random_segments(40, 5)
    entries_copy = entries.copy()
    exits_copy = exits.copy()

    improve_order(entries, exits, closed=True, deadline=time.monotonic() + 2, kick_seconds=0.2)

    assert np.array_equal(entries, entries_copy)
    assert np.array_equal(exits, exits_copy)


def test_tiny_orders_come_back_unchanged():
    entries, exits = random_segments(2, 9)

    new_entries, new_exits = improve_order(
        entries, exits, closed=False, deadline=time.monotonic() + 1
    )

    assert np.array_equal(new_entries, entries)
    assert np.array_equal(new_exits, exits)


def test_nearest_ends_never_lists_a_segment_as_its_own_neighbour():
    entries, exits = random_segments(60, 3)

    neighbours = nearest_ends(entries, exits, count=5)

    for segment, others in enumerate(neighbours):
        assert segment not in others
        assert len(others) == 5
