"""
Reference breakdowns for the `mow-report` crate, from `diagnose.py`.

    python <this script> <fixture folder> <data folder> [uid ...]

Run from the root of an `init-solver` checkout so `solver` imports. The data
folder holds `solutions/best/` and `instances/challenge_v0/instances/`, read
only. Without uids it takes the instances of `data/bench.txt` below
`MAX_CELLS`, so the Python loop stays within minutes. It writes into the
fixture folder

- `diagnose.json`: every breakdown, as `diagnose.py --json` writes them
- `diagnose.txt`: the table `diagnose.py` prints for them
"""

import contextlib
import io
import json
import sys
from pathlib import Path

# the script's own folder is on the path, the checkout root is not
sys.path.insert(0, str(Path.cwd()))

from solver import diagnose
from solver.bench import read_bench_list
from solver.model import Problem

from cgshop2027_pyutils.instance_database import InstanceDatabase

MAX_CELLS = 100_000


def pick(database: InstanceDatabase, bests: Path) -> list[str]:
    uids = []
    for uid in read_bench_list():
        if not (bests / f"{uid}.solution.json").exists():
            continue

        if Problem.from_instance(database[uid]).num_cells <= MAX_CELLS:
            uids.append(uid)

    return uids


def main() -> int:
    out = Path(sys.argv[1])
    data = Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)

    instances = data / "instances" / "challenge_v0" / "instances"
    bests = data / "solutions" / "best"
    database = InstanceDatabase(str(instances))

    uids = sys.argv[3:] or pick(database, bests)

    # diagnose.main walks the bench list, so it prints just these rows
    diagnose.read_bench_list = lambda: uids

    argv = [
        "--instances",
        str(instances),
        "--best",
        str(bests),
        "--json",
        str(out / "diagnose.json"),
    ]
    printed = io.StringIO()
    with contextlib.redirect_stdout(printed):
        diagnose.main(argv)

    (out / "diagnose.txt").write_text(printed.getvalue())

    results = json.loads((out / "diagnose.json").read_text())
    print(f"diagnose: {len(results)} breakdowns")
    print(" ".join(uids))
    return 0


if __name__ == "__main__":
    sys.exit(main())
