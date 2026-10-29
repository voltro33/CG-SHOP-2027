"""
Mixed lanes must sweep each part of the region the way it runs, cover every
cell, and never cost more than plain lanes by their own measure.
"""

import numpy as np

from cgshop2027_pyutils.grid import CellSet

from solver.construct.mixed import mixed_segment_ends, segments_mixed
from solver.construct.segments import lane_segments
from solver.model import Problem
from solver.tours import densify

KNOBS = {"tile_lanes": 4, "segment_cost": 3.0, "gap_factor": 0.0, "shift": 0.0, "flip_rounds": 4}


def l_shape() -> Problem:
    """
    A bar three cells high along the bottom and a bar three cells wide up
    the left side, each sixty long, swept by a 3 by 3 cutter.
    """
    mask = np.zeros((60, 60), dtype=bool)
    mask[0:3, :] = True
    mask[:, 0:3] = True

    region = CellSet(mask, (0, 0))
    cutter = CellSet(np.ones((3, 3), dtype=bool), (-1, -1))

    return Problem(uid="l_1s3", region=region, cutter=cutter, k=1)


def sweep_of(left: np.ndarray, right: np.ndarray) -> np.ndarray:
    positions = []
    for a, b in zip(left.tolist(), right.tolist(), strict=True):
        positions.append(densify([tuple(a), tuple(b)]))

    return np.concatenate(positions)


def test_each_bar_is_swept_along_its_length():
    problem = l_shape()

    plain_h = lane_segments(problem, gap_factor=0)
    plain_v = lane_segments(problem.transposed(), gap_factor=0)
    left, right = mixed_segment_ends(problem, **KNOBS)

    # plain lanes cut the bar that runs across them into twenty stubs
    assert min(len(plain_h), len(plain_v)) >= 20

    # one lane per bar, plus a few stubs where the corner tile keeps one direction
    assert len(left) <= 6


def test_mixed_segments_cover_every_cell():
    problem = l_shape()
    left, right = mixed_segment_ends(problem, **KNOBS)

    assert len(problem.uncovered(sweep_of(left, right))) == 0


def test_segments_mixed_returns_a_connected_walk():
    walk = segments_mixed(l_shape(), kick_seconds=0.1, time_limit=1.0)

    # consecutive corners are axis parallel
    for (x0, y0), (x1, y1) in zip(walk, walk[1:]):
        assert x0 == x1 or y0 == y1
