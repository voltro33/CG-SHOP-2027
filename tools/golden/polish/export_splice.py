"""
Solves a few windows with CP-SAT and writes down what it chose and what
`splice` built from it, so the Rust port can replay the choice through a
fake solver and must build the same tour.

    cd <python checkout>
    PYTHONPATH=. python <repo>/tools/golden/polish/export_splice.py <repo>/crates/plugins/polish/tests/fixtures/splice

Per source, `<name>.tours` holds its tours (one line of corners each) and
`<name>.txt` one line per solved window:

    tour x0 y0 x1 y1 | status objective | arc:uses ... | old new length hash

The arcs are the grid arcs taken at least once, numbered as the model
numbers them. `length` and `hash` describe the new dense tour.

Stored tours are close to optimal in small windows, so the `dented` sources
first get out-and-back dents put into them, and the windows sit on those:
there CP-SAT finds shorter walks and the splice has to reorder.
"""

import random
import sys
from pathlib import Path

import numpy as np
from canonical import cells_text, fnv, load
from ortools.sat.python import cp_model
from solver.tours import compress
from solver.window import (
    Window,
    _piece_steps,
    _required_cells,
    _WindowModel,
    cut_pieces,
)

CANDIDATES = Path("data") / "solutions" / "candidates"

# (fixture name, instance, candidate file or None for the stored best)
SOURCES = (
    ("bck2217_lanes", "bck2217_18k_1d6", "bck2217_18k_1d6_lanes-h_s0_v2"),
    ("bck2217_best", "bck2217_18k_1d6", None),
    ("bck2217_dented", "bck2217_18k_1d6", None),
    ("iso3131_dented", "iso3131_387k_3x4", None),
    (
        "srpg8529_groups",
        "srpg8529_28k_2s2",
        "srpg8529_28k_2s2_groups-h_gap_factor=0.5_s0_v2",
    ),
    (
        "iso3713_groups",
        "iso3713_144k_3l3",
        "iso3713_144k_3l3_groups-h_gap_factor=0.5_s0_v2",
    ),
)

SIDES = (6, 10, 14)
WINDOWS = 8
TRIES = 60
TIME_LIMIT = 1.0


def dented(tours: list[np.ndarray], rng: random.Random) -> list[tuple[int, list[int]]]:
    """
    Puts `WINDOWS` out-and-back dents into the tours, in place of the list
    items. Returns where they are: the tour and the point.
    """
    dents = []
    for _ in range(WINDOWS):
        index = rng.randrange(len(tours))
        positions = tours[index]

        at = rng.randrange(len(positions))
        here = positions[at]
        dx, dy = rng.choice(((1, 0), (-1, 0), (0, 1), (0, -1)))
        dent = np.array([[here[0] + dx, here[1] + dy], here], dtype=np.int64)

        tours[index] = np.concatenate([positions[: at + 1], dent, positions[at + 1 :]])
        dents.append((index, here.tolist()))

    return dents


def corners_text(tours: list[np.ndarray]) -> str:
    lines = [" ".join(f"{x} {y}" for x, y in compress(tour)) for tour in tours]

    return "\n".join(lines) + "\n"


def solve(problem, tours, index: int, window: Window) -> str | None:
    positions = tours[index]

    pieces = cut_pieces(positions, window)
    if not pieces:
        return None

    old_steps = sum(_piece_steps(piece, len(positions)) for piece in pieces)
    required = _required_cells(problem, tours, index, pieces, window)

    model = _WindowModel(problem, window, positions, pieces, required)
    model.add_hint(positions, pieces)
    model.limit_steps(old_steps)

    solver = cp_model.CpSolver()
    solver.parameters.max_time_in_seconds = TIME_LIMIT
    solver.parameters.num_workers = 1
    status = solver.solve(model.model)
    if status not in (cp_model.OPTIMAL, cp_model.FEASIBLE):
        return None

    taken = []
    for arc in range(model.grid_arcs):
        uses = int(solver.value(model.uses[arc]))
        if uses:
            taken.append(f"{arc}:{uses}")

    new_positions = model.splice(solver, positions, pieces)
    new_steps = round(solver.objective_value)

    head = f"{index} {window.x0} {window.y0} {window.x1} {window.y1}"
    verdict = f"{solver.status_name(status)} {new_steps}"
    result = (
        f"{old_steps} {new_steps} {len(new_positions)} {fnv(cells_text(new_positions))}"
    )

    return f"{head} | {verdict} | {' '.join(taken)} | {result}"


def main(out: Path) -> int:
    out.mkdir(parents=True, exist_ok=True)

    for name, uid, candidate in SOURCES:
        path = (
            None
            if candidate is None
            else CANDIDATES / uid / f"{candidate}.solution.json"
        )
        problem, tours, corners = load(uid, path)
        rng = random.Random(name)

        dents = []
        if name.endswith("_dented"):
            dents = dented(tours, rng)
            corners = corners_text(tours)
        (out / f"{name}.tours").write_text(corners)

        lines = []
        for attempt in range(TRIES):
            if len(lines) >= WINDOWS:
                break

            # a window around a dent, or a random position of a random tour
            if attempt < len(dents):
                index, (x, y) = dents[attempt]
            else:
                index = rng.randrange(len(tours))
                x, y = tours[index][rng.randrange(len(tours[index]))].tolist()

            side = rng.choice(SIDES)
            window = Window(
                x - side // 2, y - side // 2, x + side - side // 2, y + side - side // 2
            )

            line = solve(problem, tours, index, window)
            if line is not None:
                lines.append(line)

        (out / f"{name}.txt").write_text("\n".join(lines) + "\n")
        print(name, len(lines), flush=True)

    return 0


if __name__ == "__main__":
    sys.exit(main(Path(sys.argv[1])))
