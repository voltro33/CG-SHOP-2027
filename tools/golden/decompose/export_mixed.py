"""
Reference data for the `MixedLanes` decomposer in `mow-decompose`, from the
Python solver's `construct/mixed.py`.

Run it from a checkout of `init-solver` (so `solver` and `data/` resolve),
giving this script by its path in the Rust repo:

    cd <init-solver checkout>
    python <rust repo>/tools/golden/decompose/export_mixed.py

It writes `crates/plugins/decompose/tests/fixtures/mixed/<uid>.txt` next to
this script's repo for every bench instance: one block per case (decomposer
label), `case <label> <count>`, then one `<h|v> lane from to` line per
segment in Python's order. The instances themselves come from
`export_fixtures.py` (`fixtures/instances/`).
"""

import sys
import time
from pathlib import Path

import numpy as np

# the init-solver checkout we run in, not this script's folder, holds `solver`
sys.path.insert(0, str(Path.cwd()))

from solver.construct.mixed import (
    _cell_values,
    _horizontal_ends,
    _restricted,
    grid_start_x,
    grid_start_y,
    mixed_segment_ends,
    tile_directions,
    tile_grid,
)
from solver.mine import DEFAULT_INSTANCES
from solver.model import Problem

from cgshop2027_pyutils.instance_database import InstanceDatabase

REPO = Path(__file__).resolve().parents[3]
OUT = REPO / "crates/plugins/decompose/tests/fixtures/mixed"
BENCH_LIST = Path("data/bench.txt")

DEFAULTS = {
    "tile_lanes": 4,
    "segment_cost": 3.0,
    "gap_factor": 0.0,
    "shift": 0.0,
    "flip_rounds": 4,
}

# label suffix -> knobs, labels as the Rust decomposer writes them: only the
# knobs that differ from the defaults, so `gap_factor=0.0` (the default) is
# the first case and `gap_factor=0, tile_lanes=2, segment_cost=4` the second
CASES = {
    "": {},
    "/segment_cost=4.0/tile_lanes=2": {"tile_lanes": 2, "segment_cost": 4.0},
    "/segment_cost=4.0/tile_lanes=16": {"tile_lanes": 16, "segment_cost": 4.0},
    "/gap_factor=2.0": {"gap_factor": 2.0},
    "/shift=0.5": {"shift": 0.5},
}


def bench_uids() -> list[str]:
    lines = BENCH_LIST.read_text().splitlines()
    return [line.strip() for line in lines if line.strip() and not line.startswith("#")]


def split_segments(problem: Problem, knobs: dict) -> list[tuple[str, int, int, int]]:
    """
    `mixed_segment_ends` again, step by step, so each segment keeps its axis
    (a single anchor segment has equal ends and cannot tell). Checked against
    the real function below.
    """
    grid = tile_grid(problem, knobs["tile_lanes"], knobs["shift"])
    vertical = tile_directions(
        problem,
        grid,
        segment_cost=knobs["segment_cost"],
        flip_rounds=knobs["flip_rounds"],
    )

    mask = problem.region.mask
    cell_vertical = _cell_values(vertical, grid, mask.shape)
    segments = []

    horizontal_part = _restricted(problem, mask & ~cell_vertical)
    if horizontal_part is not None:
        left, right = _horizontal_ends(
            horizontal_part,
            grid_start_y(problem, grid),
            grid.band_y,
            knobs["gap_factor"],
        )
        segments += [
            ("h", y, a, b)
            for (a, y), (b, _) in zip(left.tolist(), right.tolist(), strict=False)
        ]

    vertical_part = _restricted(problem, mask & cell_vertical)
    if vertical_part is not None:
        left, right = _horizontal_ends(
            vertical_part.transposed(),
            grid_start_x(problem, grid),
            grid.band_x,
            knobs["gap_factor"],
        )
        segments += [
            ("v", x, a, b)
            for (a, x), (b, _) in zip(left.tolist(), right.tolist(), strict=False)
        ]

    return segments


def check_against_python(problem: Problem, knobs: dict, segments: list) -> None:
    left, right = mixed_segment_ends(problem, **knobs)

    def end(axis: str, lane: int, t: int) -> tuple[int, int]:
        return (t, lane) if axis == "h" else (lane, t)

    want_left = [end(s[0], s[1], s[2]) for s in segments]
    want_right = [end(s[0], s[1], s[3]) for s in segments]

    assert np.array_equal(left.reshape(-1, 2), np.array(want_left).reshape(-1, 2))
    assert np.array_equal(right.reshape(-1, 2), np.array(want_right).reshape(-1, 2))


def write_cases(problem: Problem, path: Path) -> None:
    lines = []

    for suffix, changed in CASES.items():
        knobs = {**DEFAULTS, **changed}
        segments = split_segments(problem, knobs)
        check_against_python(problem, knobs, segments)

        lines.append(f"case segments-mixed{suffix} {len(segments)}")
        lines += [f"{axis} {lane} {a} {b}" for axis, lane, a, b in segments]

    path.write_text("\n".join(lines) + "\n")


def time_largest(problem: Problem) -> None:
    started = time.perf_counter()
    mixed_segment_ends(problem, **DEFAULTS)
    seconds = time.perf_counter() - started

    print(
        f"python mixed_segment_ends (defaults) on {problem.uid}: {seconds * 1000:.1f} ms"
    )


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)

    database = InstanceDatabase(str(DEFAULT_INSTANCES))
    largest = None

    for uid in bench_uids():
        problem = Problem.from_instance(database[uid])

        write_cases(problem, OUT / f"{uid}.txt")
        print(f"{uid}: {problem.num_cells} cells")

        if largest is None or problem.num_cells > largest.num_cells:
            largest = problem

    if largest is not None:
        time_largest(largest)

    return 0


if __name__ == "__main__":
    sys.exit(main())
