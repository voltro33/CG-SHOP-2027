"""
Reference data for the `mow-improve` crate, from the Python solver.

    python tools/golden/improve/export_fixtures.py --out <crate>/tests/fixtures/pipeline [uid ...]

Run from the root of an `init-solver` checkout so `solver` imports (see
`tools/golden/README.md`); `--out` is this repository's
`crates/plugins/improve/tests/fixtures/pipeline/`. Without uids it runs the 31 bench
instances of `data/bench.txt`.

For every instance it writes `<uid>.txt`, one block per list of corners:
a line `<name> <count>` and then one line with the corners as `x y x y ...`.

- `walk`: `segments-h` with default knobs (no kicks, so it runs fast), the
  walk `build_walk` repairs;
- `repaired`: `compress(repair_walk(densify(walk)))`;
- `pruned`: `prune_routes([repaired], closed=False)` with no deadline;
- `tours N` then `tour` blocks: the closed pieces `_tours_single_walk` makes
  from `pruned`; `closed N` then `closed` blocks: their joint closed prune;
- `groups N` then `group` blocks: the `groups-h` tours as built;
  `grepaired N` then `grepaired` blocks: `close_route(compress(w))` for the
  walks `repair_tours` returns; `gpruned N` then `gpruned` blocks: their
  closed prune.

It prints Python's time for each step, for the comparison with Rust.
"""

import argparse
import math
import sys
import time
from pathlib import Path

from cgshop2027_pyutils.instance_database import InstanceDatabase

sys.path.insert(0, str(Path.cwd()))

from solver.construct import CONSTRUCTORS, GROUP_CONSTRUCTORS
from solver.improve import prune_routes
from solver.mine import DEFAULT_INSTANCES
from solver.model import Problem
from solver.repair import repair_tours, repair_walk
from solver.split import split_walk
from solver.tours import close_route, compress, densify

BENCH_LIST = Path("data/bench.txt")
NO_KICKS = {"kick_seconds": 0.0}


def bench_uids() -> list[str]:
    lines = BENCH_LIST.read_text().splitlines()
    return [line.strip() for line in lines if line.strip() and not line.startswith("#")]


def block(name: str, corners) -> list[str]:
    flat = " ".join(f"{int(x)} {int(y)}" for x, y in corners)
    return [f"{name} {len(corners)}", flat]


def blocks(name: str, item: str, routes) -> list[str]:
    lines = [f"{name} {len(routes)}"]
    for route in routes:
        lines += block(item, route)
    return lines


class Clock:
    def __init__(self) -> None:
        self.times: dict[str, float] = {}

    def run(self, name, fn, *args, **kwargs):
        started = time.perf_counter()
        result = fn(*args, **kwargs)
        self.times[name] = time.perf_counter() - started
        return result


def export(problem: Problem, out: Path) -> dict[str, float]:
    clock = Clock()
    inf = math.inf

    walk = CONSTRUCTORS["segments-h"](problem, **NO_KICKS)
    positions = clock.run("repair_walk", repair_walk, problem, densify(walk))
    repaired = compress(positions)

    (pruned,) = clock.run(
        "prune_open", prune_routes, problem, [repaired], closed=False, deadline=inf
    )
    pieces = split_walk(densify(pruned), problem.k)
    tours = [close_route(compress(piece)) for piece in pieces]
    closed = clock.run(
        "prune_closed", prune_routes, problem, tours, closed=True, deadline=inf
    )

    groups = GROUP_CONSTRUCTORS["groups-h"](problem, **NO_KICKS)
    dense = [densify(tour, closed=True) for tour in groups]
    walks = clock.run("repair_tours", repair_tours, problem, dense)
    grepaired = [close_route(compress(w)) for w in walks]
    gpruned = clock.run(
        "prune_groups", prune_routes, problem, grepaired, closed=True, deadline=inf
    )

    lines = block("walk", walk) + block("repaired", repaired) + block("pruned", pruned)
    lines += blocks("tours", "tour", tours) + blocks("closed", "closed", closed)
    lines += blocks("groups", "group", groups)
    lines += blocks("grepaired", "grepaired", grepaired)
    lines += blocks("gpruned", "gpruned", gpruned)
    out.write_text("\n".join(lines) + "\n")

    return clock.times


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("uids", nargs="*")
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()

    args.out.mkdir(parents=True, exist_ok=True)
    database = InstanceDatabase(str(DEFAULT_INSTANCES))

    for uid in args.uids or bench_uids():
        problem = Problem.from_instance(database[uid])
        times = export(problem, args.out / f"{uid}.txt")
        shown = ", ".join(f"{name} {seconds:.2f} s" for name, seconds in times.items())
        print(f"{uid}: {shown}", flush=True)

    return 0


if __name__ == "__main__":
    sys.exit(main())
