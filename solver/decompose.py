"""
Cells that follow the region's shape, so each can get its own sweep.

This is the grid version of a boustrophedon decomposition. The region is cut
into horizontal lanes one cutter height high, from its lowest row up. In each
lane, the region's cells form a few intervals, the lane pieces. Two pieces in
neighbouring lanes touch when their intervals overlap. A cell is a chain of
pieces stacked on top of each other with no split or merge between them: the
piece below touches only this piece above, and the piece above touches only
the one below. Where the region splits around a hole or a notch, or two arms
join, a new cell starts.

Ragged edges would still give many tiny cells, so cells with fewer than
`min_lanes` lanes are merged into the neighbouring cell they share the most
border with, smallest first.

    python -m solver.decompose                # cell counts on the benchmark
"""

import argparse
import sys
from dataclasses import dataclass
from pathlib import Path

import numpy as np

from cgshop2027_pyutils.instance_database import InstanceDatabase

from .model import Problem

__all__ = ["Cells", "decompose", "lane_pieces"]

DATA = Path("data")
DEFAULT_INSTANCES = DATA / "instances" / "challenge_v0" / "instances"

DEFAULT_MIN_LANES = 3


@dataclass(frozen=True)
class Cells:
    """
    `labels` has the shape of the region mask: the cell of every region
    cell, -1 outside the region. Cells are numbered 0 to `count - 1`, and
    `pieces` holds how many lane pieces each cell is made of.
    """

    labels: np.ndarray
    count: int
    pieces: np.ndarray

    def mask(self, cell: int) -> np.ndarray:
        return self.labels == cell


@dataclass(frozen=True)
class _Piece:
    lane: int
    first: int
    last: int


def lane_pieces(mask: np.ndarray, band: int) -> list[list[_Piece]]:
    """
    Per lane of `band` mask rows, from the lowest occupied row, the column
    intervals where any row of the lane has region cells.
    """
    occupied = np.flatnonzero(mask.any(axis=1))
    if len(occupied) == 0:
        return []

    lowest = int(occupied[0])
    lane_count = (mask.shape[0] - lowest + band - 1) // band

    result = []
    for lane in range(lane_count):
        top = lowest + lane * band
        columns = mask[top : top + band].any(axis=0)

        # pad so every interval has a start and an end
        changes = np.diff(np.concatenate(([0], columns.astype(np.int8), [0])))
        starts = np.flatnonzero(changes == 1)
        ends = np.flatnonzero(changes == -1) - 1

        pieces = []
        for first, last in zip(starts.tolist(), ends.tolist()):
            pieces.append(_Piece(lane, first, last))
        result.append(pieces)

    return result


def decompose(problem: Problem, *, min_lanes: int = DEFAULT_MIN_LANES) -> Cells:
    mask = problem.region.mask
    band = _band(problem)

    lanes = lane_pieces(mask, band)
    pieces = [piece for lane in lanes for piece in lane]
    if not pieces:
        return Cells(np.full(mask.shape, -1, dtype=np.int64), 0, np.zeros(0, dtype=np.int64))

    # number the pieces in lane order
    first_index = [0]
    for lane in lanes:
        first_index.append(first_index[-1] + len(lane))

    above, below, overlap = _touching(lanes, first_index)

    parent = list(range(len(pieces)))

    # a chain continues where both pieces touch only each other
    for index in range(len(pieces)):
        if len(above[index]) == 1:
            upper = above[index][0]
            if len(below[upper]) == 1:
                _union(parent, index, upper)

    _merge_small(parent, pieces, overlap, min_lanes)

    piece_cell = _relabel(parent)
    labels = _label_mask(mask, lanes, first_index, piece_cell, band)

    count = int(max(piece_cell) + 1)
    piece_counts = np.bincount(np.array(piece_cell, dtype=np.int64), minlength=count)

    return Cells(labels, count, piece_counts)


def _band(problem: Problem) -> int:
    offsets = problem.offsets

    return int(offsets[:, 1].max() - offsets[:, 1].min()) + 1


def _touching(lanes: list[list[_Piece]], first_index: list[int]):
    """
    Per piece, the pieces it overlaps in the lane above and below, and the
    overlap width of every touching pair.
    """
    total = first_index[-1]
    above: list[list[int]] = [[] for _ in range(total)]
    below: list[list[int]] = [[] for _ in range(total)]
    overlap: dict[tuple[int, int], int] = {}

    for lane in range(len(lanes) - 1):
        lower_pieces = lanes[lane]
        upper_pieces = lanes[lane + 1]

        # both lists are sorted by column, so one sweep finds every overlap
        i = 0
        j = 0
        while i < len(lower_pieces) and j < len(upper_pieces):
            lower = lower_pieces[i]
            upper = upper_pieces[j]

            width = min(lower.last, upper.last) - max(lower.first, upper.first) + 1
            if width > 0:
                a = first_index[lane] + i
                b = first_index[lane + 1] + j
                above[a].append(b)
                below[b].append(a)
                overlap[(a, b)] = width

            if lower.last < upper.last:
                i += 1
            else:
                j += 1

    return above, below, overlap


def _find(parent: list[int], index: int) -> int:
    root = index
    while parent[root] != root:
        root = parent[root]

    # point the whole path at the root
    while parent[index] != root:
        parent[index], index = root, parent[index]

    return root


def _union(parent: list[int], a: int, b: int) -> None:
    root_a = _find(parent, a)
    root_b = _find(parent, b)
    if root_a != root_b:
        parent[max(root_a, root_b)] = min(root_a, root_b)


def _merge_small(
    parent: list[int],
    pieces: list[_Piece],
    overlap: dict[tuple[int, int], int],
    min_lanes: int,
) -> None:
    """
    Merges every cell with fewer than `min_lanes` lanes into the neighbour
    it shares the widest border with, smallest cells first, until none is
    left or a small cell has no neighbour.
    """
    while True:
        roots = [_find(parent, index) for index in range(len(pieces))]

        sizes: dict[int, int] = {}
        for root in roots:
            sizes[root] = sizes.get(root, 0) + 1

        # borders between different cells
        borders: dict[int, dict[int, int]] = {}
        for (a, b), width in overlap.items():
            cell_a = roots[a]
            cell_b = roots[b]
            if cell_a == cell_b:
                continue

            borders.setdefault(cell_a, {})
            borders.setdefault(cell_b, {})
            borders[cell_a][cell_b] = borders[cell_a].get(cell_b, 0) + width
            borders[cell_b][cell_a] = borders[cell_b].get(cell_a, 0) + width

        small = [cell for cell, size in sizes.items() if size < min_lanes and cell in borders]
        if not small:
            return

        small.sort(key=lambda cell: sizes[cell])

        merged = set()
        for cell in small:
            if cell in merged:
                continue

            neighbours = {other: width for other, width in borders[cell].items() if other not in merged}
            if not neighbours:
                continue

            target = max(neighbours, key=lambda other: neighbours[other])
            _union(parent, cell, target)

            # both changed this pass; their borders are stale until the next one
            merged.add(cell)
            merged.add(target)


def _relabel(parent: list[int]) -> list[int]:
    numbers: dict[int, int] = {}
    result = []
    for index in range(len(parent)):
        root = _find(parent, index)
        if root not in numbers:
            numbers[root] = len(numbers)
        result.append(numbers[root])

    return result


def _label_mask(
    mask: np.ndarray,
    lanes: list[list[_Piece]],
    first_index: list[int],
    piece_cell: list[int],
    band: int,
) -> np.ndarray:
    labels = np.full(mask.shape, -1, dtype=np.int64)
    lowest = int(np.flatnonzero(mask.any(axis=1))[0])

    for lane, pieces in enumerate(lanes):
        top = lowest + lane * band
        bottom = min(top + band, mask.shape[0])

        for offset, piece in enumerate(pieces):
            cell = piece_cell[first_index[lane] + offset]
            window = mask[top:bottom, piece.first : piece.last + 1]
            labels[top:bottom, piece.first : piece.last + 1][window] = cell

    return labels


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("--instances", type=Path, default=DEFAULT_INSTANCES)
    parser.add_argument("--min-lanes", type=int, default=DEFAULT_MIN_LANES)
    parser.add_argument("--only", default="")
    args = parser.parse_args(argv)

    from .bench import read_bench_list

    database = InstanceDatabase(str(args.instances))
    uids = [uid for uid in read_bench_list() if args.only in uid]

    print(f"{'instance':30} {'lanes':>6} {'chains':>7} {'cells':>6} {'big share':>9}")
    for uid in uids:
        problem = Problem.from_instance(database[uid])

        raw = decompose(problem, min_lanes=1)
        cells = decompose(problem, min_lanes=args.min_lanes)

        # share of region cells in cells of at least ten lane pieces
        sizes = np.bincount(cells.labels[cells.labels >= 0], minlength=cells.count)
        big = cells.pieces >= 10
        share = sizes[big].sum() / sizes.sum()

        lane_count = (problem.region.mask.shape[0] + _band(problem) - 1) // _band(problem)
        print(f"{uid:30} {lane_count:>6} {raw.count:>7} {cells.count:>6} {share:>9.1%}", flush=True)

    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
