"""
Mixed lanes: horizontal lanes where the region runs wide, vertical where it
runs tall.

One sweep direction for the whole region pays for every lane end, turn and
shoulder where the boundary is ragged across the lanes. The diagnosis on the
benchmark puts most of the lost length there. So the region is cut into
square tiles on the lane grid, and each tile sweeps one way or the other:

1. Tiles are `tile_lanes` lanes high and wide, with borders on lane borders
   in both directions, so a lane never crosses a tile border sideways.
2. The cost of a choice is `length + segment_cost * band` summed over the
   segments it makes, where all horizontal tiles of one tile row are cut
   into segments together, so lanes run on across tile borders, and the
   same for vertical tiles of one tile column.
3. The search starts from the cheaper of all horizontal and all vertical,
   then flips single tiles while that lowers the cost. Flipping a tile only
   changes its own tile row and tile column, so each try is cheap and exact.
   The result is never worse than plain lanes by this cost.
4. Both sets of segments are ordered together by the usual order search.
"""

from dataclasses import dataclass

import numpy as np

from cgshop2027_pyutils.grid import CellSet

from ..model import Problem
from ..tours import Point
from .segments import lane_segments, walk_segments

__all__ = ["TileGrid", "mixed_segment_ends", "segments_mixed", "tile_directions"]


@dataclass(frozen=True)
class TileGrid:
    """
    Lane and tile borders of one problem. Horizontal lanes are `band_y` rows
    high, vertical lanes `band_x` columns wide; a tile is `lanes` of each.
    Mask row `first_row` and mask column `first_column` start the first tile,
    both zero or negative.
    """

    band_y: int
    band_x: int
    lanes: int
    first_row: int
    first_column: int
    rows: int
    columns: int

    @property
    def height(self) -> int:
        return self.lanes * self.band_y

    @property
    def width(self) -> int:
        return self.lanes * self.band_x

    def row_slice(self, tile_row: int) -> slice:
        top = self.first_row + tile_row * self.height
        return slice(max(0, top), top + self.height)

    def column_slice(self, tile_column: int) -> slice:
        left = self.first_column + tile_column * self.width
        return slice(max(0, left), left + self.width)


def segments_mixed(
    problem: Problem,
    *,
    tile_lanes: int = 4,
    segment_cost: float = 3.0,
    gap_factor: float = 0.0,
    shift: float = 0.0,
    flip_rounds: int = 4,
    kick_seconds: float | None = None,
    seed: int = 0,
    anneal: float = 0.0,
    time_limit: float = 10.0,
) -> list[Point]:
    """
    An open walk over horizontal and vertical lane segments.
    """
    left, right = mixed_segment_ends(
        problem,
        tile_lanes=int(tile_lanes),
        segment_cost=segment_cost,
        gap_factor=gap_factor,
        shift=shift,
        flip_rounds=int(flip_rounds),
    )
    if len(left) == 0:
        return []

    return walk_segments(
        left,
        right,
        closed=problem.k == 1,
        kick_seconds=kick_seconds,
        seed=int(seed),
        anneal=anneal,
        time_limit=time_limit,
    )


def mixed_segment_ends(
    problem: Problem,
    *,
    tile_lanes: int,
    segment_cost: float,
    gap_factor: float,
    shift: float,
    flip_rounds: int,
) -> tuple[np.ndarray, np.ndarray]:
    """
    The two end points of every segment, horizontal and vertical together,
    in the coordinates of the problem.
    """
    grid = tile_grid(problem, tile_lanes, shift)
    vertical = tile_directions(problem, grid, segment_cost=segment_cost, flip_rounds=flip_rounds)

    # every region cell gets the direction of its tile
    mask = problem.region.mask
    cell_vertical = _cell_values(vertical, grid, mask.shape)

    ends_left = []
    ends_right = []

    horizontal_part = _restricted(problem, mask & ~cell_vertical)
    if horizontal_part is not None:
        left, right = _horizontal_ends(horizontal_part, grid_start_y(problem, grid), grid.band_y, gap_factor)
        ends_left.append(left)
        ends_right.append(right)

    vertical_part = _restricted(problem, mask & cell_vertical)
    if vertical_part is not None:
        # vertical lanes are horizontal lanes of the mirrored problem
        mirrored = vertical_part.transposed()
        left, right = _horizontal_ends(mirrored, grid_start_x(problem, grid), grid.band_x, gap_factor)
        ends_left.append(left[:, ::-1])
        ends_right.append(right[:, ::-1])

    if not ends_left:
        empty = np.zeros((0, 2), dtype=np.int64)
        return empty, empty

    return np.concatenate(ends_left), np.concatenate(ends_right)


def tile_grid(problem: Problem, tile_lanes: int, shift: float) -> TileGrid:
    offsets = problem.offsets
    band_y = int(offsets[:, 1].max() - offsets[:, 1].min()) + 1
    band_x = int(offsets[:, 0].max() - offsets[:, 0].min()) + 1

    # a shift moves lane borders down and left by a fraction of a lane
    first_row = -int(shift * band_y)
    first_column = -int(shift * band_x)

    height, width = problem.region.mask.shape
    rows = -(-(height - first_row) // (tile_lanes * band_y))
    columns = -(-(width - first_column) // (tile_lanes * band_x))

    return TileGrid(band_y, band_x, tile_lanes, first_row, first_column, rows, columns)


def grid_start_y(problem: Problem, grid: TileGrid) -> int:
    return problem.region.origin[1] + grid.first_row


def grid_start_x(problem: Problem, grid: TileGrid) -> int:
    return problem.region.origin[0] + grid.first_column


def tile_directions(
    problem: Problem, grid: TileGrid, *, segment_cost: float, flip_rounds: int
) -> np.ndarray:
    """
    A `(tile rows, tile columns)` bool array, True where a tile sweeps
    vertically, from a hill climb over single tile flips.
    """
    mask = problem.region.mask
    occupied = _occupied_tiles(mask, grid)

    def row_cost(tile_row: int, vertical: np.ndarray) -> float:
        # the horizontal cells of one tile row, cut into lanes together
        rows = grid.row_slice(tile_row)
        columns_vertical = _spread(vertical[tile_row], grid.first_column, grid.width, mask.shape[1])
        strip = mask[rows] & ~columns_vertical[None, :]
        return _strip_cost(problem, strip, rows.start, 0, segment_cost, grid, along_x=True)

    def column_cost(tile_column: int, vertical: np.ndarray) -> float:
        columns = grid.column_slice(tile_column)
        rows_vertical = _spread(vertical[:, tile_column], grid.first_row, grid.height, mask.shape[0])
        strip = mask[:, columns] & rows_vertical[:, None]
        return _strip_cost(problem, strip, 0, columns.start, segment_cost, grid, along_x=False)

    all_h = np.zeros((grid.rows, grid.columns), dtype=bool)
    all_v = occupied.copy()

    costs = {}
    for name, start in (("h", all_h), ("v", all_v)):
        rows_total = [row_cost(r, start) for r in range(grid.rows)]
        columns_total = [column_cost(c, start) for c in range(grid.columns)]
        costs[name] = (rows_total, columns_total)

    total_h = sum(costs["h"][0]) + sum(costs["h"][1])
    total_v = sum(costs["v"][0]) + sum(costs["v"][1])

    if total_v < total_h:
        vertical = all_v
        row_costs, column_costs = costs["v"]
    else:
        vertical = all_h
        row_costs, column_costs = costs["h"]

    tiles = [tuple(index) for index in np.argwhere(occupied).tolist()]

    for _ in range(flip_rounds):
        improved = False

        for tile_row, tile_column in tiles:
            flipped = vertical.copy()
            flipped[tile_row, tile_column] = not vertical[tile_row, tile_column]

            new_row = row_cost(tile_row, flipped)
            new_column = column_cost(tile_column, flipped)
            old = row_costs[tile_row] + column_costs[tile_column]

            # keep the flip only if it pays, by a little more than rounding
            if new_row + new_column < old - 1e-9:
                vertical = flipped
                row_costs[tile_row] = new_row
                column_costs[tile_column] = new_column
                improved = True

        if not improved:
            break

    return vertical


def _strip_cost(
    problem: Problem,
    strip: np.ndarray,
    top: int,
    left: int,
    segment_cost: float,
    grid: TileGrid,
    *,
    along_x: bool,
) -> float:
    """
    `length + segment_cost * band` over the segments of the cells in
    `strip`, a piece of the mask starting at mask row `top`, column `left`.
    """
    if not strip.any():
        return 0.0

    x0, y0 = problem.region.origin
    region = CellSet(strip.copy(), (x0 + left, y0 + top))
    part = Problem(uid=problem.uid, region=region, cutter=problem.cutter, k=problem.k)

    if along_x:
        band = grid.band_y
        left_ends, right_ends = _horizontal_ends(part, grid_start_y(problem, grid), band, 0.0)
    else:
        band = grid.band_x
        left_ends, right_ends = _horizontal_ends(part.transposed(), grid_start_x(problem, grid), band, 0.0)

    length = float(np.abs(right_ends - left_ends).sum())

    return length + segment_cost * band * len(left_ends)


def _horizontal_ends(
    problem: Problem, grid_start: int, band: int, gap_factor: float
) -> tuple[np.ndarray, np.ndarray]:
    """
    Horizontal lane segments of `problem` with lane borders every `band`
    rows from `grid_start`, as left and right end points.
    """
    mask = problem.region.mask
    lowest_row = problem.region.origin[1] + int(np.flatnonzero(mask.any(axis=1))[0])

    # lane_segments starts its lanes at the lowest row, an offset moves them onto the grid
    offset = (lowest_row - grid_start) % band

    segments = lane_segments(problem, offset=offset, gap_factor=gap_factor)

    left = np.array([(s.a, s.y) for s in segments], dtype=np.int64).reshape(-1, 2)
    right = np.array([(s.b, s.y) for s in segments], dtype=np.int64).reshape(-1, 2)

    return left, right


def _occupied_tiles(mask: np.ndarray, grid: TileGrid) -> np.ndarray:
    occupied = np.zeros((grid.rows, grid.columns), dtype=bool)

    for tile_row in range(grid.rows):
        rows = grid.row_slice(tile_row)
        for tile_column in range(grid.columns):
            columns = grid.column_slice(tile_column)
            occupied[tile_row, tile_column] = bool(mask[rows, columns].any())

    return occupied


def _spread(values: np.ndarray, first: int, size: int, length: int) -> np.ndarray:
    """
    Per tile values along one axis, repeated out to one value per mask index.
    """
    index = (np.arange(length) - first) // size

    return values[index]


def _cell_values(tiles: np.ndarray, grid: TileGrid, shape: tuple[int, int]) -> np.ndarray:
    rows = _spread(np.arange(grid.rows), grid.first_row, grid.height, shape[0])
    columns = _spread(np.arange(grid.columns), grid.first_column, grid.width, shape[1])

    return tiles[rows[:, None], columns[None, :]]


def _restricted(problem: Problem, mask: np.ndarray) -> Problem | None:
    """
    The same problem with only the region cells in `mask`, or None if empty.
    """
    if not mask.any():
        return None

    rows = np.flatnonzero(mask.any(axis=1))
    columns = np.flatnonzero(mask.any(axis=0))
    top, bottom = int(rows[0]), int(rows[-1]) + 1
    left, right = int(columns[0]), int(columns[-1]) + 1

    x0, y0 = problem.region.origin
    cropped = mask[top:bottom, left:right].copy()
    region = CellSet(cropped, (x0 + left, y0 + top))

    return Problem(uid=problem.uid, region=region, cutter=problem.cutter, k=problem.k)
