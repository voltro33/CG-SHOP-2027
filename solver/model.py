"""
The problem as the solver sees it: a region of cells, a cutter given as cell
offsets from its center, and the number of cutters.

Vocabulary used throughout the solver:

- *cell*: a unit square `(x, y)` of the grid, as in `cgshop2027_pyutils.grid`.
- *anchor* / *position*: a lattice point where the cutter's center stands.
- *footprint* of an anchor `p`: the cells `p + o` for every cutter offset `o`.
- *neighborhood* of a cell `c`: the anchors whose footprint contains `c`,
  i.e. `c - o` for every offset `o`.
- *walk*: an open sequence of axis-parallel corner points; *positions* are the
  unit-step points a walk passes through.
"""

from dataclasses import dataclass

import numpy as np

from cgshop2027_pyutils.grid import CellSet, dilate, rasterize, rasterize_ring
from cgshop2027_pyutils.schemas import CGSHOP2027Instance

__all__ = ["Problem", "points_to_cellset"]


@dataclass(frozen=True)
class Problem:
    """
    Instance-derived data that every solver component needs, computed once.
    """

    uid: str
    region: CellSet
    cutter: CellSet  # cutter cells relative to the cutter's center
    k: int

    @classmethod
    def from_instance(cls, instance: CGSHOP2027Instance) -> "Problem":
        center_x, center_y = instance.cutter_center
        return cls(
            uid=instance.instance_uid,
            region=rasterize(instance.region_to_cover),
            cutter=rasterize_ring(instance.cutter).translated(-center_x, -center_y),
            k=instance.number_of_cutters,
        )

    # -- basic quantities ---------------------------------------------------

    @property
    def num_cells(self) -> int:
        return len(self.region)

    @property
    def offsets(self) -> np.ndarray:
        """
        The cutter's cells relative to its center, as an `(m, 2)` int array.
        """
        return np.array(list(self.cutter.cells()), dtype=np.int64)

    @property
    def region_cells(self) -> np.ndarray:
        """
        The region's cells as an `(n, 2)` int array.
        """
        return np.array(list(self.region.cells()), dtype=np.int64)

    def step_gain(self) -> int:
        """
        The most cells one unit step of the cutter can newly sweep: the size of
        `C \\ (C + e)` maximized over the four unit directions `e`.
        """
        cells = {tuple(o) for o in self.offsets.tolist()}
        return max(
            sum((x - dx, y - dy) not in cells for x, y in cells)
            for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))
        )

    def row_extents(self) -> dict[int, tuple[int, int]]:
        """
        For each row offset `dy` of the cutter, the smallest and largest `dx`.
        """
        extents: dict[int, tuple[int, int]] = {}
        for dx, dy in self.offsets.tolist():
            low, high = extents.get(dy, (dx, dx))
            extents[dy] = (min(low, dx), max(high, dx))
        return extents

    def anchor_frame(self) -> tuple[tuple[int, int], tuple[int, int]]:
        """
        `(origin, shape)` of the smallest grid holding every anchor whose
        footprint can touch the region; `shape` is `(height, width)`.
        """
        rx0, ry0, rx1, ry1 = self.region.bounds
        cx0, cy0, cx1, cy1 = self.cutter.bounds
        origin = (rx0 - cx1, ry0 - cy1)
        shape = ((ry1 - cy0) - origin[1] + 1, (rx1 - cx0) - origin[0] + 1)
        return origin, shape

    # -- coverage -----------------------------------------------------------

    def swept(self, positions: np.ndarray) -> CellSet:
        """
        The cells swept by the cutter standing at each of `positions`.
        """
        return dilate(points_to_cellset(positions), self.cutter)

    def uncovered(self, positions: np.ndarray) -> CellSet:
        """
        The region cells that no position in `positions` sweeps.
        """
        return self.region.difference(self.swept(positions))

    # -- symmetry -----------------------------------------------------------

    def transposed(self) -> "Problem":
        """
        The same problem mirrored across the diagonal `x = y`, so a solver
        written for horizontal sweeps also yields vertical ones.
        """
        return Problem(
            uid=self.uid,
            region=_transpose(self.region),
            cutter=_transpose(self.cutter),
            k=self.k,
        )


def _transpose(cells: CellSet) -> CellSet:
    return CellSet(cells.mask.T.copy(), (cells.origin[1], cells.origin[0]))


def points_to_cellset(points: np.ndarray) -> CellSet:
    """
    A `CellSet` holding exactly the given `(n, 2)` lattice points.
    """
    pts = np.asarray(points, dtype=np.int64).reshape(-1, 2)
    if len(pts) == 0:
        return CellSet(np.zeros((1, 1), dtype=bool), (0, 0))
    x0, y0 = pts.min(axis=0)
    x1, y1 = pts.max(axis=0)
    mask = np.zeros((y1 - y0 + 1, x1 - x0 + 1), dtype=bool)
    mask[pts[:, 1] - y0, pts[:, 0] - x0] = True
    return CellSet(mask, (int(x0), int(y0)))
