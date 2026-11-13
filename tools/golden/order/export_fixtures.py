"""
Reference data for the `mow-order` crate, from the Python solver.

    .venv/Scripts/python.exe tools/golden/order/export_fixtures.py

Run from the repo root so `solver` imports. For each case below it writes
`crates/plugins/order/tests/fixtures/<name>.txt`:

    closure closed
    python_nn 1234
    python_ils 1100 seed 0 kick_seconds 3 seconds 4.1 checks 98765 moves 4321
    segments 3
    <lane> <from> <to>
    ...

`python_nn` is the link length of Python's nearest neighbour order and
`python_ils` the one after the pipeline of `walk_segments`: nearest
neighbour, windowed 2-opt, then `improve_order` with the given kick time.
`checks` counts the segments the descent looked at, `moves` the improving
moves it made, both over `seconds`, the time `improve_order` took. They come
from a second, instrumented run, so the timed one pays no counting overhead.
"""

import dataclasses
import sys
import time
from pathlib import Path

import numpy as np

from cgshop2027_pyutils.instance_database import InstanceDatabase

# a script's own folder is on the path, the repo root is not
sys.path.insert(0, str(Path(__file__).resolve().parents[3]))

from solver.construct import ordering
from solver.construct.ordering import improve_order, order_length
from solver.construct.segments import _nearest_neighbor, _two_opt, lane_segments
from solver.mine import DEFAULT_INSTANCES
from solver.model import Problem, points_to_cellset

OUT = Path("crates/plugins/order/tests/fixtures")

KICK_SECONDS = 3.0
SEED = 0

# the windowed 2-opt of walk_segments
WINDOW = 60

# name -> (instance, gap_factor, unit cutter). A unit cutter mows one row per
# lane, which is the only way the challenge set reaches tens of thousands of
# segments.
CASES = {
    "fjr3672_65k_2p4": ("fjr3672_65k_2p4", 2.0, False),
    "iso8005_105k_2s3": ("iso8005_105k_2s3", 2.0, False),
    "srpg5461_22k_1l3": ("srpg5461_22k_1l3", 2.0, False),
    "fna52057_1415k_2x6-gap0": ("fna52057_1415k_2x6", 0.0, False),
    "monalisa100k_389k_1l4-gap0": ("monalisa100k_389k_1l4", 0.0, False),
    "monalisa100k_389k_1l4-unit": ("monalisa100k_389k_1l4", 2.0, True),
    "uniform00800001_708k_1s5-unit-gap0": ("uniform00800001_708k_1s5", 0.0, True),
}


def segments_of(
    problem: Problem, gap_factor: float, unit: bool
) -> list[tuple[int, int, int]]:
    if unit:
        problem = dataclasses.replace(
            problem, cutter=points_to_cellset(np.array([[0, 0]]))
        )

    segments = lane_segments(problem, gap_factor=gap_factor)
    return [(s.y, s.a, s.b) for s in segments]


def pipeline(
    left: np.ndarray, right: np.ndarray, closed: bool
) -> tuple[int, int, float]:
    """
    Nearest neighbour length, final length, and the seconds `improve_order` ran.
    """
    deadline = time.monotonic() + 600.0

    entries, exits = _nearest_neighbor(left, right)
    nn = order_length(
        [tuple(p) for p in entries.tolist()], [tuple(p) for p in exits.tolist()], closed
    )

    entries, exits = _two_opt(entries, exits, deadline=deadline, window=WINDOW)

    started = time.perf_counter()
    entries, exits = improve_order(
        entries,
        exits,
        closed=closed,
        deadline=deadline,
        kick_seconds=KICK_SECONDS,
        seed=SEED,
    )
    seconds = time.perf_counter() - started

    final = order_length(
        [tuple(p) for p in entries.tolist()], [tuple(p) for p in exits.tolist()], closed
    )
    return nn, final, seconds


def counted(left: np.ndarray, right: np.ndarray, closed: bool) -> tuple[int, int]:
    """
    Checks and improving moves of one `improve_order` run, by wrapping the
    two move functions it looks up by name.
    """
    counts = {"checks": 0, "moves": 0}
    two_opt, or_opt = ordering._try_two_opt, ordering._try_or_opt

    def counting_two_opt(*args):
        counts["checks"] += 1
        touched = two_opt(*args)
        counts["moves"] += touched is not None
        return touched

    def counting_or_opt(*args):
        touched = or_opt(*args)
        counts["moves"] += touched is not None
        return touched

    ordering._try_two_opt, ordering._try_or_opt = counting_two_opt, counting_or_opt
    try:
        pipeline(left, right, closed)
    finally:
        ordering._try_two_opt, ordering._try_or_opt = two_opt, or_opt

    return counts["checks"], counts["moves"]


def write_fixture(
    name: str, problem: Problem, segments: list[tuple[int, int, int]]
) -> None:
    closed = problem.k == 1
    left = np.array([(a, y) for y, a, _ in segments], dtype=np.int64)
    right = np.array([(b, y) for y, _, b in segments], dtype=np.int64)

    nn, final, seconds = pipeline(left, right, closed)
    checks, moves = counted(left, right, closed)

    lines = [
        f"closure {'closed' if closed else 'open'}",
        f"python_nn {nn}",
        f"python_ils {final} seed {SEED} kick_seconds {KICK_SECONDS:g} "
        f"seconds {seconds:.3f} checks {checks} moves {moves}",
        f"segments {len(segments)}",
    ]
    lines += [f"{y} {a} {b}" for y, a, b in segments]
    (OUT / f"{name}.txt").write_text("\n".join(lines) + "\n")

    print(
        f"{name}: {len(segments)} segments, nn {nn}, ils {final}, {seconds:.1f} s, {checks} checks"
    )


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    database = InstanceDatabase(str(DEFAULT_INSTANCES))

    for name, (uid, gap_factor, unit) in CASES.items():
        problem = Problem.from_instance(database[uid])
        write_fixture(name, problem, segments_of(problem, gap_factor, unit))

    return 0


if __name__ == "__main__":
    sys.exit(main())
