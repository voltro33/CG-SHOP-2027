"""
Harder repair cases for the `mow-improve` crate: the default pipeline's walks
and tours already sweep almost everything, so here they are damaged first.

    python tools/golden/improve/export_damaged.py --out <crate>/tests/fixtures/damaged [uid ...]

Run from the root of an `init-solver` checkout (see `tools/golden/README.md`).
For every bench instance it writes `<uid>.txt` in the format of
`export_fixtures.py`:

- `walk`: the `segments-h` walk with every `SKIP`th inner corner dropped and
  the rest joined by `connect`, so whole stretches of lanes go missing;
- `repaired`: `compress(repair_walk(densify(walk)))`;
- `groups N` / `group`: the `groups-h` tours damaged the same way and closed
  with `close_route`;
- `grepaired N` / `grepaired`: `close_route(compress(w))` for the walks
  `repair_tours` returns.
"""

import argparse
import sys
import time
from pathlib import Path

from cgshop2027_pyutils.instance_database import InstanceDatabase

sys.path.insert(0, str(Path.cwd()))

from solver.construct import CONSTRUCTORS, GROUP_CONSTRUCTORS
from solver.mine import DEFAULT_INSTANCES
from solver.model import Problem
from solver.repair import repair_tours, repair_walk
from solver.tours import close_route, compress, connect, densify

BENCH_LIST = Path("data/bench.txt")
NO_KICKS = {"kick_seconds": 0.0}
SKIP = 9


def bench_uids() -> list[str]:
    lines = BENCH_LIST.read_text().splitlines()
    return [line.strip() for line in lines if line.strip() and not line.startswith("#")]


def damaged(corners):
    kept = [
        p
        for i, p in enumerate(corners)
        if i % SKIP != SKIP - 1 or i == len(corners) - 1
    ]
    return connect(kept)


def block(name: str, corners) -> list[str]:
    flat = " ".join(f"{int(x)} {int(y)}" for x, y in corners)
    return [f"{name} {len(corners)}", flat]


def blocks(name: str, item: str, routes) -> list[str]:
    lines = [f"{name} {len(routes)}"]
    for route in routes:
        lines += block(item, route)
    return lines


def export(problem: Problem, out: Path) -> float:
    walk = damaged(CONSTRUCTORS["segments-h"](problem, **NO_KICKS))
    groups = [
        close_route(damaged(t))
        for t in GROUP_CONSTRUCTORS["groups-h"](problem, **NO_KICKS)
    ]

    started = time.perf_counter()
    repaired = compress(repair_walk(problem, densify(walk)))
    walks = repair_tours(problem, [densify(t, closed=True) for t in groups])
    seconds = time.perf_counter() - started

    grepaired = [close_route(compress(w)) for w in walks]

    lines = block("walk", walk) + block("repaired", repaired)
    lines += blocks("groups", "group", groups) + blocks(
        "grepaired", "grepaired", grepaired
    )
    out.write_text("\n".join(lines) + "\n")

    return seconds


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("uids", nargs="*")
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()

    args.out.mkdir(parents=True, exist_ok=True)
    database = InstanceDatabase(str(DEFAULT_INSTANCES))

    for uid in args.uids or bench_uids():
        problem = Problem.from_instance(database[uid])
        seconds = export(problem, args.out / f"{uid}.txt")
        print(f"{uid}: repair_walk + repair_tours {seconds:.2f} s", flush=True)

    return 0


if __name__ == "__main__":
    sys.exit(main())
