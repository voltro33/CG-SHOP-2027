"""
Reference data for the `mow-assign` crate, from the Python solver.

    cd <a checkout of init-solver>
    python <this repo>/tools/golden/assign/export_fixtures.py

Run from the root of an `init-solver` checkout, so `solver` imports from the
current folder. The fixtures land in this repo's
`crates/plugins/assign/tests/fixtures/`. For every bench instance it writes

- `walks/<uid>.txt`: the repaired default walk (`build_walk` with
  `segments-h`, a short kick time) as its corners, then the piece bounds
  `split_walk` finds for k = 2 and 3, as first and last position index.
- `groups/<uid>.txt`: the lane segments `groups_horizontal` builds, the
  longest closed tour it returns and how long it took, an open order of the
  segments as entries and exits, and the groups `group_order` cuts it into
  for k = 2 and 3.
- `bands/<uid>.txt`: for each axis and share list, the bands
  `band_problems` returns, as cell box, cell count and a hash of the row
  runs (FNV-1a 64 over the text `y a b\n` of every run, bottom to top, left
  to right).

and once `rebalanced.txt`: share lists, tour lengths and the shares
`_rebalanced` returns, as `repr` floats.
"""

import sys
import time
from pathlib import Path

import numpy as np

from cgshop2027_pyutils.instance_database import InstanceDatabase

# import `solver` from the checkout we run in, not from this repo
sys.path.insert(0, str(Path.cwd()))

from solver.construct.groups import group_order, groups_horizontal
from solver.construct.ordering import improve_order
from solver.construct.segments import _nearest_neighbor, lane_segments
from solver.mine import DEFAULT_INSTANCES
from solver.model import Problem
from solver.partition import _rebalanced, band_problems
from solver.pipeline import SolveConfig, build_walk
from solver.split import split_walk
from solver.tours import compress, route_length

OUT = Path(__file__).resolve().parents[3] / "crates/plugins/assign/tests/fixtures"
BENCH_LIST = Path("data/bench.txt")

WALK_CONFIG = SolveConfig(
    method="segments-h", params={"kick_seconds": 0.2, "time_limit": 1.0}
)

GROUP_KICK = 0.25
GROUP_TIME_LIMIT = 2.0

SHARES = ([1.0, 1.0], [1.0, 1.0, 1.0], [0.7, 1.3], [1.2, 0.5, 1.0], [3.0, 0.01, 1.0])


def bench_uids() -> list[str]:
    lines = BENCH_LIST.read_text().splitlines()
    return [line.strip() for line in lines if line.strip() and not line.startswith("#")]


def fnv1a(text: str) -> int:
    value = 0xCBF29CE484222325
    for byte in text.encode():
        value ^= byte
        value = (value * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return value


def runs_text(problem: Problem) -> str:
    mask = problem.region.mask
    x0, y0 = problem.region.origin

    lines = []
    for row in range(mask.shape[0]):
        padded = np.concatenate(([False], mask[row], [False])).astype(np.int8)
        edges = np.flatnonzero(np.diff(padded))
        for start, stop in zip(edges[::2], edges[1::2], strict=True):
            lines.append(f"{y0 + row} {x0 + start} {x0 + stop - 1}\n")

    return "".join(lines)


def write_walk(problem: Problem, path: Path) -> float:
    positions = build_walk(problem, WALK_CONFIG)

    steps = np.abs(np.diff(positions, axis=0)).sum(axis=1)
    assert (steps == 1).all(), "the walk must be unit steps for indices to match"

    corners = compress(positions)
    lines = [f"positions {len(positions)}", f"corners {len(corners)}"]
    lines += [f"{x} {y}" for x, y in corners]

    started = time.perf_counter()
    for k in (2, 3):
        pieces = split_walk(positions, k)
        bounds = []
        first = 0
        for piece in pieces:
            bounds += [first, first + len(piece) - 1]
            first += len(piece)
        lines.append(f"split {k} " + " ".join(map(str, bounds)))
    seconds = time.perf_counter() - started

    lines.append(f"python_seconds {seconds:.4f}")
    path.write_text("\n".join(lines) + "\n")

    return seconds


def write_groups(problem: Problem, path: Path) -> None:
    segments = lane_segments(problem, gap_factor=0.0)

    started = time.perf_counter()
    tours = groups_horizontal(
        problem, kick_seconds=GROUP_KICK, time_limit=GROUP_TIME_LIMIT
    )
    seconds = time.perf_counter() - started
    longest = max(route_length(t, closed=True) for t in tours)

    left = np.array([(s.a, s.y) for s in segments], dtype=np.int64)
    right = np.array([(s.b, s.y) for s in segments], dtype=np.int64)
    entries, exits = _nearest_neighbor(left, right)
    entries, exits = improve_order(
        entries, exits, closed=False, deadline=time.monotonic() + 5, kick_seconds=0.1
    )

    lines = [
        f"k {problem.k}",
        f"python_groups {longest} tours {len(tours)} kick {GROUP_KICK} seconds {seconds:.3f}",
        f"segments {len(segments)}",
    ]
    lines += [f"{s.y} {s.a} {s.b}" for s in segments]

    lines.append(f"order {len(entries)}")
    lines += [
        f"{a} {b} {c} {d}"
        for (a, b), (c, d) in zip(entries.tolist(), exits.tolist(), strict=False)
    ]

    for k in (2, 3):
        groups = group_order(entries, exits, k)
        flat = [str(i) for group in groups for i in group]
        lines.append(f"group_order {k} " + " ".join(flat))

    path.write_text("\n".join(lines) + "\n")


def write_bands(problem: Problem, path: Path) -> float:
    lines = []
    seconds = 0.0

    for axis in ("y", "x"):
        for shares in SHARES:
            started = time.perf_counter()
            bands = band_problems(problem, shares, axis)
            seconds += time.perf_counter() - started

            lines.append(f"case {axis} {len(bands)} " + " ".join(map(repr, shares)))
            for band in bands:
                cells = band.region_cells
                x_min, y_min = cells.min(axis=0).tolist()
                x_max, y_max = cells.max(axis=0).tolist()
                digest = fnv1a(runs_text(band))
                lines.append(
                    f"{x_min} {y_min} {x_max} {y_max} {band.num_cells} {digest}"
                )

    path.write_text("\n".join(lines) + "\n")

    return seconds


def write_rebalanced(path: Path) -> None:
    rng = np.random.default_rng(0)

    lines = []
    for _ in range(50):
        k = int(rng.integers(2, 4))
        shares = [float(s) for s in rng.uniform(0.2, 3.0, k)]
        lengths = [int(n) for n in rng.integers(1, 200_000, k)]
        result = _rebalanced(shares, lengths)

        lines.append(" ".join(map(repr, shares)))
        lines.append(" ".join(map(str, lengths)))
        lines.append(" ".join(map(repr, result)))

    path.write_text("\n".join(lines) + "\n")


def main() -> int:
    for folder in ("walks", "groups", "bands"):
        (OUT / folder).mkdir(parents=True, exist_ok=True)

    only = set(sys.argv[1:])
    database = InstanceDatabase(str(DEFAULT_INSTANCES))
    split_seconds = band_seconds = 0.0

    for uid in bench_uids():
        if only and uid not in only:
            continue

        problem = Problem.from_instance(database[uid])

        split_seconds += write_walk(problem, OUT / "walks" / f"{uid}.txt")
        write_groups(problem, OUT / "groups" / f"{uid}.txt")
        band_seconds += write_bands(problem, OUT / "bands" / f"{uid}.txt")
        print(f"{uid}: {problem.num_cells} cells", flush=True)

    write_rebalanced(OUT / "rebalanced.txt")
    print(f"python split_walk k=2,3 total {split_seconds:.3f} s")
    print(f"python band_problems total {band_seconds:.3f} s")

    return 0


if __name__ == "__main__":
    sys.exit(main())
