"""
Reference data for the `mow-report` crate, from the Python reporting tools.

    python <this script> <fixture folder> <data folder>

Run from the root of an `init-solver` checkout so `solver` imports. The data
folder holds the inputs, read only: `catalog.csv`, `leaderboard.csv`,
`jobs.jsonl`, `solutions/best/` and `instances/challenge_v0/instances/`. It
writes into the fixture folder

- `catalog.csv`: `catalog.py` run afresh on the challenge instances
- `leaderboard.csv`: `report.write_leaderboard` from the stored bests, the
  job log and the catalog
- `summary.json`: `report.summary` of those rows
- `mine_report.txt` and `mine_report.json`: what `mine_report.py` prints for
  the job log, and its `--json` file
- `bench_pick.txt`: `bench.pick_bench` from the catalog and `leaderboard.csv`
- `inputs.txt`: how many job records and stored bests went in, so a test
  can tell when the data moved on
"""

import contextlib
import csv
import io
import json
import sys
from dataclasses import asdict
from pathlib import Path

# the script's own folder is on the path, the checkout root is not
sys.path.insert(0, str(Path.cwd()))

from solver import catalog, mine_report
from solver.bench import pick_bench
from solver.mine import BestStore, JobLog
from solver.report import leaderboard_rows, summary, write_leaderboard


def read_csv(path: Path) -> list[dict[str, str]]:
    with path.open() as file:
        return list(csv.DictReader(file))


def write_catalog(out: Path, data: Path) -> None:
    instances = data / "instances" / "challenge_v0" / "instances"
    rows = catalog.catalog([instances], workers=8)

    with (out / "catalog.csv").open("w", newline="") as file:
        fields = list(asdict(rows[0]))
        writer = csv.DictWriter(file, fieldnames=fields, lineterminator="\n")
        writer.writeheader()
        for row in rows:
            writer.writerow(asdict(row))

    print(f"catalog: {len(rows)} rows")


def write_leaderboard_and_summary(out: Path, data: Path) -> tuple[int, int]:
    catalog_rows = read_csv(data / "catalog.csv")
    by_uid = {row["instance"]: row for row in catalog_rows}
    challenge = [row for row in catalog_rows if row["source"] == "instances"]

    best = data / "solutions" / "best"
    store = BestStore(best, data / "solutions" / "candidates")
    records = JobLog(data / "jobs.jsonl").records

    write_leaderboard(out / "leaderboard.csv", by_uid, store, records)

    rows = leaderboard_rows(by_uid, store.objective, store.label, records)
    text = json.dumps(summary(rows, len(challenge)), indent=2)
    (out / "summary.json").write_text(text + "\n")

    print(f"leaderboard: {len(rows)} rows")
    return len(records), len(store.objective)


def write_mine_report(out: Path, data: Path) -> None:
    argv = [
        "--log",
        str(data / "jobs.jsonl"),
        "--catalog",
        str(data / "catalog.csv"),
        "--json",
        str(out / "mine_report.json"),
    ]

    printed = io.StringIO()
    with contextlib.redirect_stdout(printed):
        mine_report.main(argv)

    (out / "mine_report.txt").write_text(printed.getvalue())
    print("mine report written")


def write_bench_pick(out: Path, data: Path) -> None:
    catalog_rows = read_csv(data / "catalog.csv")
    leaderboard = read_csv(data / "leaderboard.csv")

    uids = pick_bench(catalog_rows, leaderboard)
    (out / "bench_pick.txt").write_text("\n".join(uids) + "\n")
    print(f"bench pick: {len(uids)} instances")


def main() -> int:
    out = Path(sys.argv[1])
    data = Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)

    write_catalog(out, data)
    records, bests = write_leaderboard_and_summary(out, data)
    write_mine_report(out, data)
    write_bench_pick(out, data)

    (out / "inputs.txt").write_text(f"records {records}\nbests {bests}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
