"""
Reference data for the `mow-cli` crate, from the Python solver.

    python <this script> <fixture folder>

Run from the root of an `init-solver` checkout with the data linked, so
`solver` and `data/` resolve. It writes into the fixture folder

- `grid.txt`: Python's whole job grid (`jobs.build_jobs`) for the bench
  instances, in run order. A header line `instance <uid> <cells> <k>` per
  instance, then one line per job: `<round> <time_limit> <job_id>`.
- `lanes/<uid>.txt`: the corners of the plain lane snake of `lanes.py`
  before `connect` adds the links, one block per axis: `axis <h|v> <count>`
  then `x y` lines.

The Rust tests rebuild the grid from the instance lines and compare every
job, and drive the snake over the Rust lanes and compare the corners.
"""

import csv
import sys
from pathlib import Path

# the script's own folder is on the path, the checkout root is not
sys.path.insert(0, str(Path.cwd()))

from solver.construct import lanes
from solver.jobs import build_jobs
from solver.mine import DEFAULT_INSTANCES
from solver.model import Problem

from cgshop2027_pyutils.instance_database import InstanceDatabase

BENCH = Path("data/bench.txt")
CATALOG = Path("data/catalog.csv")

# small enough that the lanes fixture stays a few kilobytes
LANE_UIDS = ("fjs3649_17k_1x7", "iso5009_26k_1x6", "dka1376_23k_1p7")


def bench_uids() -> list[str]:
    lines = BENCH.read_text().splitlines()

    return [line.strip() for line in lines if line.strip() and not line.startswith("#")]


def write_grid(out: Path, uids: list[str]) -> None:
    with CATALOG.open() as file:
        catalog = {row["instance"]: row for row in csv.DictReader(file)}

    jobs = [
        job for job in build_jobs(DEFAULT_INSTANCES, catalog) if job.instance in uids
    ]

    lines = []
    for uid in sorted(uids):
        row = catalog[uid]
        lines.append(f"instance {uid} {row['cells']} {row['k']}")

    lines += [f"{job.round} {job.time_limit:g} {job.job_id}" for job in jobs]

    (out / "grid.txt").write_text("\n".join(lines) + "\n")
    print(f"grid: {len(jobs)} jobs")


def write_lanes(out: Path) -> None:
    # without the links, the walk is just the lane corners in snake order
    lanes.connect = list

    database = InstanceDatabase(str(DEFAULT_INSTANCES))
    folder = out / "lanes"
    folder.mkdir(parents=True, exist_ok=True)

    for uid in LANE_UIDS:
        problem = Problem.from_instance(database[uid])

        lines = []
        for axis, build in (("h", lanes.lanes_horizontal), ("v", lanes.lanes_vertical)):
            corners = build(problem)
            lines.append(f"axis {axis} {len(corners)}")
            lines += [f"{x} {y}" for x, y in corners]

        (folder / f"{uid}.txt").write_text("\n".join(lines) + "\n")
        print(f"lanes: {uid}")


def main() -> int:
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)

    write_grid(out, bench_uids())
    write_lanes(out)

    return 0


if __name__ == "__main__":
    sys.exit(main())
