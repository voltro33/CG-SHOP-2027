"""
Where the waste of a tour sits, window by window, and how much a perfect
re-solve of each window could save at most.

    python -m solver.window_map --best ../CG-SHOP-2027/data/solutions/best \
        --instances ../CG-SHOP-2027/data/instances/challenge_v0/instances

The anchor positions of all tours are cut into square windows of `side`
anchors. For one window:

- length: the unit steps whose anchor lies in the window
- need: the region cells that only anchors in this window sweep; any
  re-solve that keeps the tour outside the window must cover them from inside
- room: `length - need / gain`, the length above what those cells cost at the
  best rate. A perfect re-solve saves at most this, and less, since it must
  also link the pieces where the tour enters and leaves
- pieces: how often the tours enter the window, which sets the model size

Every cell has at most one window it needs, so over one tiling the rooms add
up to `total length - exclusive cells / gain`, the ceiling of re-solving every
window on its own.
"""

import argparse
import json
import statistics
import sys
from dataclasses import asdict, dataclass
from pathlib import Path

import numpy as np

from cgshop2027_pyutils.instance_database import InstanceDatabase
from cgshop2027_pyutils.io import read_solution
from cgshop2027_pyutils.schemas import CGSHOP2027Solution

from .model import Problem
from .report import family
from .tours import densify

__all__ = ["WindowMap", "WindowSummary", "summarize", "window_map"]

DATA = Path("data")
DEFAULT_INSTANCES = DATA / "instances" / "challenge_v0" / "instances"

# window sides in cutter spans
DEFAULT_SIDES = (2, 4, 8)

# the worst windows, as shares of all windows the tours touch
WORST_SHARES = (0.10, 0.25)

NO_WINDOW = np.iinfo(np.int64).max


@dataclass(frozen=True)
class WindowMap:
    """
    Per window arrays of one tiling, all of shape `(rows, columns)`.
    """

    side: int
    gain: int
    total_length: int
    length: np.ndarray
    need: np.ndarray
    pieces: np.ndarray

    @property
    def room(self) -> np.ndarray:
        return self.length - self.need / self.gain


@dataclass(frozen=True)
class WindowSummary:
    instance: str
    family: str
    side_spans: int
    side: int
    windows: int
    room_share: float
    worst_10_share: float
    worst_25_share: float
    worst_10_pieces: float
    worst_10_room: float


def window_map(problem: Problem, solution: CGSHOP2027Solution, side: int) -> WindowMap:
    gain = problem.step_gain()

    tours = []
    for tour in solution.tours:
        corners = list(zip(tour.x, tour.y))
        tours.append(densify(corners, closed=True))

    everything = np.concatenate(tours)
    low = everything.min(axis=0)
    high = everything.max(axis=0)

    columns = int((high[0] - low[0]) // side) + 1
    rows = int((high[1] - low[1]) // side) + 1

    length = np.zeros(rows * columns, dtype=np.int64)
    pieces = np.zeros(rows * columns, dtype=np.int64)

    # the window of every visited anchor, on a grid over all anchors
    width, height = (high - low + 1).tolist()
    lowest_id = np.full((height, width), NO_WINDOW, dtype=np.int64)
    highest_id = np.full((height, width), -1, dtype=np.int64)

    for positions in tours:
        ids = _window_ids(positions, low, side, columns)
        np.add.at(length, ids, 1)

        # a piece starts where the tour comes in from another window
        entering = ids != np.roll(ids, 1)
        if not entering.any():
            entering[0] = True
        np.add.at(pieces, ids[entering], 1)

        xs = positions[:, 0] - low[0]
        ys = positions[:, 1] - low[1]
        lowest_id[ys, xs] = ids
        highest_id[ys, xs] = ids

    need = _exclusive_cells(problem, low, lowest_id, highest_id, rows * columns)

    return WindowMap(
        side=side,
        gain=gain,
        total_length=int(sum(len(positions) for positions in tours)),
        length=length.reshape(rows, columns),
        need=need.reshape(rows, columns),
        pieces=pieces.reshape(rows, columns),
    )


def _window_ids(positions: np.ndarray, low: np.ndarray, side: int, columns: int) -> np.ndarray:
    window_x = (positions[:, 0] - low[0]) // side
    window_y = (positions[:, 1] - low[1]) // side

    return window_y * columns + window_x


def _exclusive_cells(
    problem: Problem,
    low: np.ndarray,
    lowest_id: np.ndarray,
    highest_id: np.ndarray,
    windows: int,
) -> np.ndarray:
    """
    Per window, the region cells whose every sweeping anchor lies in it. A
    cell's lowest and highest window id over the anchors that sweep it are
    equal exactly then.
    """
    offsets = problem.offsets
    offset_low = offsets.min(axis=0)
    offset_high = offsets.max(axis=0)

    # cells are anchors plus offsets, so their frame is the anchor grid grown by the cutter
    height, width = lowest_id.shape
    cell_width = width + int(offset_high[0] - offset_low[0])
    cell_height = height + int(offset_high[1] - offset_low[1])
    cell_low = low + offset_low

    cell_lowest = np.full((cell_height, cell_width), NO_WINDOW, dtype=np.int64)
    cell_highest = np.full((cell_height, cell_width), -1, dtype=np.int64)

    for dx, dy in (offsets - offset_low).tolist():
        rows = slice(dy, dy + height)
        columns = slice(dx, dx + width)
        np.minimum(cell_lowest[rows, columns], lowest_id, out=cell_lowest[rows, columns])
        np.maximum(cell_highest[rows, columns], highest_id, out=cell_highest[rows, columns])

    # region cells, moved into the cell frame
    region = problem.region_cells - cell_low
    ids_low = cell_lowest[region[:, 1], region[:, 0]]
    ids_high = cell_highest[region[:, 1], region[:, 0]]

    exclusive = (ids_low == ids_high) & (ids_low != NO_WINDOW)

    return np.bincount(ids_low[exclusive], minlength=windows)


def summarize(problem: Problem, result: WindowMap, side_spans: int) -> WindowSummary:
    touched = result.length > 0
    room = result.room[touched]
    pieces = result.pieces[touched]

    # windows ordered by room, most first
    order = np.argsort(-room)
    positive_room = float(np.clip(room, 0, None).sum())

    shares = []
    for share in WORST_SHARES:
        count = max(1, round(share * len(room)))
        worst = np.clip(room[order[:count]], 0, None).sum()
        shares.append(float(worst / positive_room) if positive_room else 0.0)

    worst_count = max(1, round(WORST_SHARES[0] * len(room)))
    worst = order[:worst_count]

    return WindowSummary(
        instance=problem.uid,
        family=family(problem.uid),
        side_spans=side_spans,
        side=result.side,
        windows=int(touched.sum()),
        room_share=float(room.sum() / result.total_length),
        worst_10_share=shares[0],
        worst_25_share=shares[1],
        worst_10_pieces=float(statistics.median(pieces[worst].tolist())),
        worst_10_room=float(statistics.median(room[worst].tolist())),
    )


def cutter_span(problem: Problem) -> int:
    offsets = problem.offsets

    return int(offsets.max() - offsets.min()) + 1


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("--instances", type=Path, default=DEFAULT_INSTANCES)
    parser.add_argument("--best", type=Path, default=DATA / "solutions" / "best")
    parser.add_argument("--sides", default=",".join(str(s) for s in DEFAULT_SIDES))
    parser.add_argument("--only", default="")
    parser.add_argument("--json", type=Path, default=None, help="also write every summary here")
    args = parser.parse_args(argv)

    from .bench import read_bench_list

    sides = [int(text) for text in args.sides.split(",")]
    uids = [uid for uid in read_bench_list() if args.only in uid]
    database = InstanceDatabase(str(args.instances))

    print(f"{'instance':30} {'side':>4} {'windows':>7} {'room':>6} {'top10':>6} {'top25':>6} {'pieces':>6} {'room/w':>7}")

    results = []
    for uid in uids:
        problem = Problem.from_instance(database[uid])
        solution = read_solution(args.best / f"{uid}.solution.json")

        for spans in sides:
            side = spans * cutter_span(problem)
            summary = summarize(problem, window_map(problem, solution, side), spans)
            results.append(summary)

            print(
                f"{uid:30} {spans:>4} {summary.windows:>7} {summary.room_share:6.1%} "
                f"{summary.worst_10_share:6.1%} {summary.worst_25_share:6.1%} "
                f"{summary.worst_10_pieces:>6.0f} {summary.worst_10_room:>7.0f}",
                flush=True,
            )

    if args.json:
        args.json.write_text(json.dumps([asdict(result) for result in results], indent=2))

    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
