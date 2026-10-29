import numpy as np
import pytest

from solver.tours import (
    close_route,
    compress,
    connect,
    densify,
    normalize_cycle,
    route_length,
)


def test_connect_inserts_horizontal_first_bend():
    assert connect([(0, 0), (3, 2)]) == [(0, 0), (3, 0), (3, 2)]


def test_connect_drops_repeats():
    assert connect([(0, 0), (0, 0), (0, 4)]) == [(0, 0), (0, 4)]


def test_densify_open_includes_both_ends():
    dense = densify([(0, 0), (2, 0), (2, 1)])
    assert dense.tolist() == [[0, 0], [1, 0], [2, 0], [2, 1]]


def test_densify_closed_returns_without_repeating_start():
    dense = densify([(0, 0), (1, 0), (1, 1), (0, 1)], closed=True)
    assert len(dense) == route_length([(0, 0), (1, 0), (1, 1), (0, 1)], closed=True)
    assert dense[0].tolist() == [0, 0]


def test_densify_rejects_diagonal():
    with pytest.raises(ValueError, match="axis-parallel"):
        densify([(0, 0), (1, 1)])


def test_compress_inverts_densify_and_keeps_reversals():
    corners = [(0, 0), (4, 0), (1, 0), (1, 3)]
    assert compress(densify(corners)) == corners


def test_normalize_cycle_removes_straight_and_repeated_points():
    assert normalize_cycle([(0, 0), (2, 0), (4, 0), (4, 0), (4, 2), (0, 2)]) == [
        (0, 0),
        (4, 0),
        (4, 2),
        (0, 2),
    ]


def test_close_route_adds_bend_for_diagonal_return():
    route = close_route([(0, 0), (3, 0), (3, 2), (1, 2)])
    # Last point (1, 2) to first (0, 0) needs a bend; the result is a rectangle.
    assert route_length(route, closed=True) == 10
    pairs = zip(route, [*route[1:], route[0]], strict=True)
    assert all(a[0] == b[0] or a[1] == b[1] for a, b in pairs)


def test_close_route_keeps_single_point():
    assert close_route([(5, 5)]) == [(5, 5)]


def test_inputs_are_not_mutated():
    corners = [(0, 0), (2, 0), (4, 0), (4, 2)]
    snapshot = list(corners)
    normalize_cycle(corners)
    close_route(corners)
    assert corners == snapshot
    positions = densify(corners)
    copy = positions.copy()
    compress(positions)
    assert np.array_equal(positions, copy)
