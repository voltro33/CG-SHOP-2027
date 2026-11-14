"""
Prints how long Python takes to build the CP-SAT model of a window on the
largest instances, to compare with `cargo run --release --example timing
-p mow-polish`, which builds the same windows.

    cd <python checkout>
    PYTHONPATH=. python <repo>/tools/golden/polish/time_models.py

The windows are `SIDE` anchors wide, centered on `WINDOWS` positions spread
evenly along the longest tour of the stored best. The time covers cutting
the pieces, the required cells, the model, the hint and the step limit.
"""

import sys
import time

from canonical import load
from solver.polish import _longest
from solver.window import (
    Window,
    _piece_steps,
    _required_cells,
    _WindowModel,
    cut_pieces,
)

INSTANCES = (
    "euronight0070000_8749k_3s12",
    "octa2104_8486k_3l8",
    "uniform00900002_7283k_3p8",
)

SIDE = 20
WINDOWS = 10


def build(problem, tours, index: int, window: Window) -> int:
    positions = tours[index]

    pieces = cut_pieces(positions, window)
    if not pieces:
        return 0

    old_steps = sum(_piece_steps(piece, len(positions)) for piece in pieces)
    required = _required_cells(problem, tours, index, pieces, window)

    model = _WindowModel(problem, window, positions, pieces, required)
    model.add_hint(positions, pieces)
    model.limit_steps(old_steps)

    return len(model.model.Proto().constraints)


def main() -> int:
    for uid in INSTANCES:
        problem, tours, _ = load(uid)
        longest = _longest(tours)
        positions = tours[longest]

        started = time.perf_counter()
        constraints = 0
        for step in range(WINDOWS):
            x, y = positions[step * len(positions) // WINDOWS].tolist()
            window = Window(x - SIDE // 2, y - SIDE // 2, x + SIDE // 2, y + SIDE // 2)
            constraints += build(problem, tours, longest, window)

        elapsed = (time.perf_counter() - started) / WINDOWS
        print(
            f"{uid:32} {elapsed * 1000:8.1f} ms per window, {constraints} constraints"
        )

    return 0


if __name__ == "__main__":
    sys.exit(main())
