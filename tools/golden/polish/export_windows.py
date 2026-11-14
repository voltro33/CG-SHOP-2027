"""
Writes, for many windows on stored bests, what `solver/window.py` makes of
them: the pieces, the required cells and the CP-SAT model in the canonical
form of `canonical.py`, so the Rust port can prove it builds the same.

    cd <python checkout>
    PYTHONPATH=. python <repo>/tools/golden/polish/export_windows.py <repo>/crates/plugins/polish/tests/fixtures/windows

The tours go to `<uid>.tours`, one line of corners each, so the test does
not depend on the best store staying as it was. Then one line per window in
`<uid>.txt`:

    tour x0 y0 x1 y1 | pieces | old required required_hash | vars constraints model_hash

`pieces` is `none` when the tour never leaves the window, else `first:last`
pairs (maybe none). The last two groups are `-` when there is nothing to
model, the model group also for windows above `MODEL_NODES` anchors.
"""

import random
import sys
from pathlib import Path

import numpy as np
from canonical import canonical, cells_text, fnv, load
from solver.window import (
    Window,
    _piece_steps,
    _required_cells,
    _WindowModel,
    cut_pieces,
)

INSTANCES = (
    "ics39603_9468_1l5",
    "bck2217_18k_1d6",
    "pcb1173_41k_1x11",
    "srpg8529_28k_2s2",
    "octa29860_37k_2p4",
    "isoa4725_44k_2l4",
    "iso3713_144k_3l3",
    "bgf4475_195k_3l3",
    "iso3131_387k_3x4",
)

SIDES = (1, 2, 3, 5, 8, 13, 20, 32)
RANDOM_WINDOWS = 30
MODEL_NODES = 1600


def windows_for(positions: np.ndarray, rng: random.Random) -> list[Window]:
    x_min, y_min = positions.min(axis=0).tolist()
    x_max, y_max = positions.max(axis=0).tolist()

    windows = []
    for _ in range(RANDOM_WINDOWS):
        side = rng.choice(SIDES)
        x0 = rng.randint(x_min - side, x_max)
        y0 = rng.randint(y_min - side, y_max)
        windows.append(Window(x0, y0, x0 + side, y0 + side))

    # across the border of the tour's box, at its outermost positions
    for axis in (0, 1):
        for extreme in (positions[:, axis].argmin(), positions[:, axis].argmax()):
            x, y = positions[extreme].tolist()
            windows.append(Window(x - 5, y - 5, x + 6, y + 6))

    # a corner of the box, a thin strip, the whole tour, more than it, nothing
    windows.append(Window(x_min - 4, y_min - 4, x_min + 6, y_min + 6))
    windows.append(Window(x_min, y_min, x_min + 1, y_min + 12))
    windows.append(Window(x_min, y_min, x_max + 1, y_max + 1))
    windows.append(Window(x_min - 5, y_min - 5, x_max + 9, y_max + 9))
    windows.append(Window(x_max + 50, y_max + 50, x_max + 70, y_max + 70))

    return windows


def describe(problem, tours, index: int, window: Window) -> str:
    positions = tours[index]
    head = f"{index} {window.x0} {window.y0} {window.x1} {window.y1}"

    pieces = cut_pieces(positions, window)
    if pieces is None:
        return f"{head} | none | - | -"

    text = " ".join(f"{piece.first}:{piece.last}" for piece in pieces)
    if not pieces:
        return f"{head} | {text} | - | -"

    old_steps = sum(_piece_steps(piece, len(positions)) for piece in pieces)
    required = _required_cells(problem, tours, index, pieces, window)
    cells = f"{old_steps} {len(required)} {fnv(cells_text(required))}"

    if window.width * window.height > MODEL_NODES:
        return f"{head} | {text} | {cells} | -"

    model = _WindowModel(problem, window, positions, pieces, required)
    model.add_hint(positions, pieces)
    model.limit_steps(old_steps)

    proto = model.model.Proto()
    shape = f"{len(proto.variables)} {len(proto.constraints)}"

    return f"{head} | {text} | {cells} | {shape} {fnv(canonical(proto))}"


def main(out: Path) -> int:
    out.mkdir(parents=True, exist_ok=True)

    for uid in INSTANCES:
        problem, tours, corners = load(uid)
        (out / f"{uid}.tours").write_text(corners)

        rng = random.Random(uid)

        lines = [
            describe(problem, tours, index, window)
            for index, positions in enumerate(tours)
            for window in windows_for(positions, rng)
        ]

        (out / f"{uid}.txt").write_text("\n".join(lines) + "\n")
        print(uid, len(lines), flush=True)

    return 0


if __name__ == "__main__":
    sys.exit(main(Path(sys.argv[1])))
