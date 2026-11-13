"""
Reference data for the `mow-decompose` crate, from the Python solver.

    .venv/Scripts/python.exe tools/golden/decompose/export_fixtures.py

Run from the repo root so `solver` imports. For every bench instance it writes

- `instances/<uid>.txt`: the instance as the Rust tests load it, `k`, the
  cutter cells relative to its center, and the region as row runs.
- `segments/<uid>.txt`: one block per case (decomposer label), the segment
  count, then one `lane from to` line per segment in Python's order.

The Rust fixture test compares its segments with these exactly.
"""

import sys
import time
from pathlib import Path

from cgshop2027_pyutils.instance_database import InstanceDatabase

# a script's own folder is on the path, the repo root is not
sys.path.insert(0, str(Path(__file__).resolve().parents[3]))

from solver.construct.lanes import _lane_span, _row_ranges
from solver.construct.segments import _row_runs, lane_segments
from solver.mine import DEFAULT_INSTANCES
from solver.model import Problem

OUT = Path("crates/plugins/decompose/tests/fixtures")
BENCH_LIST = Path("data/bench.txt")

# label suffix -> lane_segments knobs, labels as the Rust decomposers write them
SEGMENT_KNOBS = {
    "": {},
    "/gap_factor=0.0": {"gap_factor": 0.0},
    "/gap_factor=4.0": {"gap_factor": 4.0},
    "/offset=1": {"offset": 1},
    "/shift=0.5": {"shift": 0.5},
}


def bench_uids() -> list[str]:
    lines = BENCH_LIST.read_text().splitlines()
    return [line.strip() for line in lines if line.strip() and not line.startswith("#")]


def plain_lanes(problem: Problem) -> list[tuple[int, int, int]]:
    """
    The lanes of `lanes_horizontal` as `(y, a, b)`, before they are joined
    into a walk.
    """
    extents = problem.row_extents()
    dy_low, dy_high = min(extents), max(extents)
    band = dy_high - dy_low + 1

    rows = _row_ranges(problem)
    y_first = min(rows) - dy_low
    y_last = max(rows) - dy_high

    lanes = []
    for y in range(y_first, max(y_first, y_last + band - 1) + 1, band):
        span = _lane_span(rows, extents, y)
        if span is not None:
            lanes.append((y, span[0], span[1]))

    return lanes


def cases(problem: Problem) -> dict[str, list[tuple[int, int, int]]]:
    result = {}

    for suffix, mirrored in (("h", problem), ("v", problem.transposed())):
        result[f"lanes-{suffix}"] = plain_lanes(mirrored)

        for knobs_label, knobs in SEGMENT_KNOBS.items():
            segments = lane_segments(mirrored, **knobs)
            result[f"segments-{suffix}{knobs_label}"] = [
                (s.y, s.a, s.b) for s in segments
            ]

    return result


def write_instance(problem: Problem, path: Path) -> None:
    offsets = problem.offsets.tolist()
    runs = [
        (y, int(first), int(last))
        for y, row in sorted(_row_runs(problem).items())
        for first, last in row.tolist()
    ]

    lines = [f"k {problem.k}", f"cutter {len(offsets)}"]
    lines += [f"{dx} {dy}" for dx, dy in offsets]
    lines.append(f"runs {len(runs)}")
    lines += [f"{y} {a} {b}" for y, a, b in runs]

    path.write_text("\n".join(lines) + "\n")


def write_segments(
    all_cases: dict[str, list[tuple[int, int, int]]], path: Path
) -> None:
    lines = []
    for label, segments in all_cases.items():
        lines.append(f"case {label} {len(segments)}")
        lines += [f"{lane} {a} {b}" for lane, a, b in segments]

    path.write_text("\n".join(lines) + "\n")


def time_largest(problem: Problem) -> None:
    started = time.perf_counter()
    lane_segments(problem)
    lane_segments(problem.transposed())
    seconds = time.perf_counter() - started

    print(f"python lane_segments both axes on {problem.uid}: {seconds * 1000:.1f} ms")


def main() -> int:
    (OUT / "instances").mkdir(parents=True, exist_ok=True)
    (OUT / "segments").mkdir(parents=True, exist_ok=True)

    database = InstanceDatabase(str(DEFAULT_INSTANCES))
    largest = None

    for uid in bench_uids():
        problem = Problem.from_instance(database[uid])

        write_instance(problem, OUT / "instances" / f"{uid}.txt")
        write_segments(cases(problem), OUT / "segments" / f"{uid}.txt")
        print(f"{uid}: {problem.num_cells} cells")

        if largest is None or problem.num_cells > largest.num_cells:
            largest = problem

    if largest is not None:
        time_largest(largest)

    return 0


if __name__ == "__main__":
    sys.exit(main())
