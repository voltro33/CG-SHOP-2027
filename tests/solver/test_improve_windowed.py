"""
The windowed coverage check must give the same answers as the pointwise
one. The windowed path only exists to save memory on huge cutters.
"""

import numpy as np
import pytest

import solver.improve as improve
from solver.improve import VisitCounts
from solver.model import Problem
from tests.solver.test_pipeline import L_SHAPE, make_instance
from tests.solver.test_segments_partition import U_SHAPE

PLUS_CUTTER = [
    (1, 0), (2, 0), (2, 1), (3, 1), (3, 2), (2, 2),
    (2, 3), (1, 3), (1, 2), (0, 2), (0, 1), (1, 1),
]  # fmt: skip


@pytest.mark.parametrize("shape", [U_SHAPE, L_SHAPE])
@pytest.mark.parametrize("cutter", [PLUS_CUTTER, [(0, 0), (3, 0), (3, 3), (0, 3)]])
def test_windowed_matches_pointwise(monkeypatch, shape, cutter):
    center = (1, 1)
    problem = Problem.from_instance(make_instance(shape, cutter=cutter, center=center))

    rng = np.random.default_rng(7)
    x0, y0, x1, y1 = problem.region.bounds

    for _ in range(60):
        # a random set of visited anchors, some outside the region
        visited = rng.integers([x0 - 2, y0 - 2], [x1 + 3, y1 + 3], size=(40, 2))
        counts = VisitCounts(problem, [visited])

        # pretend a random handful of those visits is taken away
        removed = visited[rng.choice(len(visited), size=5, replace=False)]
        counts.add(removed, -1)

        monkeypatch.setattr(improve, "_POINTWISE_LIMIT", 10**12)
        pointwise = counts.all_swept_around(removed)

        monkeypatch.setattr(improve, "_POINTWISE_LIMIT", 0)
        windowed = counts.all_swept_around(removed)

        assert pointwise == windowed
