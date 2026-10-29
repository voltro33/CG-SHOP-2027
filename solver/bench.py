"""
A fixed benchmark set, so an idea is judged in minutes instead of overnight.

    python -m solver.bench pick                                # write data/bench.txt once
    python -m solver.bench run best                            # overnight winner per instance
    python -m solver.bench run "segments-h/gap_factor=1/bands-x" --seconds 60

A config is written the way the leaderboard writes it: the method, then
`key=value` params, then an optional partition, joined by `/`. The word
`best` runs each instance with the config that found its stored best.

Every result is verified by the official verifier inside the worker. Scores
compare against the bests stored before the run:

- self score: `sum (best known / ours) ** 2`, one point per instance means
  as good as before, more means better
- pessimistic score: `sum (lower bound / ours) ** 2`, the contest score if a
  team reached every bound

Any strictly better solution is promoted into the best store, like the miner
does. Each run keeps its own job log in `data/bench/`.
"""

import argparse
import csv
import json
import logging
import statistics
import sys
import time
from pathlib import Path
from typing import Any

from cgshop2027_pyutils.io import read_solution

from .jobs import Job, time_limit_for
from .mine import DEFAULT_INSTANCES, BestStore, JobLog, mine
from .report import family, pessimistic_score, write_leaderboard

__all__ = ["bench_jobs", "parse_config", "pick_bench", "score_run"]

DATA = Path("data")
BENCH_LIST = DATA / "bench.txt"

# small enough that a whole run fits in a few minutes
MAX_CELLS = 400_000
PER_STRATUM = 2

# the worst X gaps at full size, where the most room is
WORST_X_LARGE = 3

# a round number no mining round uses, so bench records never look like baseline
BENCH_ROUND = -1


def pick_bench(catalog: list[dict[str, str]], leaderboard: list[dict[str, str]]) -> list[str]:
    """
    Two instances per cutter family and k, spread over the size range, plus
    the worst X gaps above the size cap. Deterministic for fixed inputs.
    """
    strata: dict[tuple[str, str], list[tuple[int, str]]] = {}
    for row in catalog:
        if row["source"] != "instances":
            continue

        cells = int(row["cells"])
        if cells > MAX_CELLS:
            continue

        key = (family(row["instance"]), row["k"])
        strata.setdefault(key, []).append((cells, row["instance"]))

    chosen = []
    for key in sorted(strata):
        members = sorted(strata[key])

        # the instances at one third and two thirds of the size order
        count = len(members)
        picks = []
        for share in range(1, PER_STRATUM + 1):
            index = (count * share) // (PER_STRATUM + 1)
            picks.append(members[index][1])

        for uid in dict.fromkeys(picks):
            chosen.append(uid)

    # worst gaps of the X family among the big instances
    large_x = []
    for row in leaderboard:
        is_x = family(row["instance"]) == "X"
        is_large = int(row["cells"]) > MAX_CELLS
        if is_x and is_large and row["gap_pct"]:
            large_x.append((-float(row["gap_pct"]), row["instance"]))

    for _, uid in sorted(large_x)[:WORST_X_LARGE]:
        chosen.append(uid)

    return chosen


def parse_config(text: str) -> dict[str, Any]:
    """
    `segments-h/gap_factor=1/bands-x` into keyword arguments for `Job`.
    A raw job id like `uid|label|s0|v2` is reduced to its label first.
    """
    if "|" in text:
        text = text.split("|")[1]

    parts = text.split("/")
    method = parts[0]

    params = []
    partition = None
    for part in parts[1:]:
        if "=" in part:
            key, value = part.split("=", 1)
            params.append((key, float(value)))
        else:
            partition = part

    # the seed param also names the job, as in the mining grid
    seed = 0
    for key, value in params:
        if key == "seed":
            seed = int(value)

    return {"method": method, "params": tuple(params), "partition": partition, "seed": seed}


def config_of_best(path: Path) -> dict[str, Any]:
    """
    The exact config that produced a stored best, from its metadata.
    """
    meta = read_solution(path).meta
    stored = meta.get("params")

    if not stored:
        return parse_config(str(meta.get("job", "")))

    params = []
    for key, value in stored["params"]:
        params.append((key, float(value)))

    return {
        "method": stored["method"],
        "params": tuple(params),
        "partition": stored["partition"],
        "seed": int(stored["seed"]),
    }


def bench_jobs(
    uids: list[str],
    catalog: dict[str, dict[str, str]],
    config: str,
    *,
    source: Path,
    best_folder: Path,
    seconds: float | None,
) -> list[Job]:
    jobs = []
    for uid in uids:
        row = catalog[uid]
        cells = int(row["cells"])

        if config == "best":
            settings = config_of_best(best_folder / f"{uid}.solution.json")
        else:
            settings = parse_config(config)

        # by default the same wall clock cap the miner used
        limit = seconds if seconds is not None else time_limit_for(cells)

        job = Job(
            instance=uid,
            source=str(source),
            cells=cells,
            k=int(row["k"]),
            round=BENCH_ROUND,
            time_limit=limit,
            **settings,
        )
        jobs.append(job)

    # biggest first, so the long jobs start early
    jobs.sort(key=lambda job: -job.cells)

    return jobs


def score_run(
    uids: list[str],
    catalog: dict[str, dict[str, str]],
    best_before: dict[str, int],
    ours: dict[str, int],
) -> dict[str, Any]:
    """
    Self score, pessimistic score and gap medians of one run. An instance
    without a verified result scores zero.
    """
    self_score = 0.0
    pessimistic = 0.0
    pessimistic_before = 0.0
    gaps: dict[str, list[float]] = {}
    rows = []

    for uid in uids:
        bound = int(catalog[uid]["lower_bound"])
        before = best_before.get(uid)
        objective = ours.get(uid)

        if before:
            pessimistic_before += pessimistic_score(bound, before) or 0.0

        if objective is None:
            rows.append({"instance": uid, "before": before, "ours": None, "ratio": 0.0})
            continue

        ratio = (before / objective) ** 2 if before else 1.0
        self_score += ratio
        pessimistic += pessimistic_score(bound, objective) or 0.0

        gap = 100.0 * (objective - bound) / bound
        gaps.setdefault(family(uid), []).append(gap)

        rows.append({"instance": uid, "before": before, "ours": objective, "ratio": ratio})

    medians = {}
    for name in sorted(gaps):
        medians[name] = round(statistics.median(gaps[name]), 1)

    return {
        "instances": len(uids),
        "feasible": sum(1 for row in rows if row["ours"] is not None),
        "self_score": round(self_score, 2),
        "pessimistic_score": round(pessimistic, 2),
        "pessimistic_before": round(pessimistic_before, 2),
        "median_gap_by_family": medians,
        "rows": rows,
    }


def read_bench_list(path: Path = BENCH_LIST) -> list[str]:
    uids = []
    for line in path.read_text().splitlines():
        line = line.strip()
        if line and not line.startswith("#"):
            uids.append(line)

    return uids


def _read_csv(path: Path) -> list[dict[str, str]]:
    with path.open() as file:
        return list(csv.DictReader(file))


def _print_report(result: dict[str, Any], config: str) -> None:
    print()
    print(f"config: {config}")
    print(f"{'instance':36} {'before':>8} {'ours':>8} {'ratio':>7}")

    for row in result["rows"]:
        ours = row["ours"] if row["ours"] is not None else "FAIL"
        print(f"{row['instance']:36} {row['before']:>8} {ours:>8} {row['ratio']:>7.3f}")

    summary = {key: value for key, value in result.items() if key != "rows"}
    print(json.dumps(summary, indent=2))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    sub = parser.add_subparsers(dest="command", required=True)

    sub.add_parser("pick", help="write the benchmark list once")

    run = sub.add_parser("run", help="run one config on every benchmark instance")
    run.add_argument("config")
    run.add_argument("--seconds", type=float, default=None)
    run.add_argument("--workers", type=int, default=15)
    run.add_argument("--instances", type=Path, default=DEFAULT_INSTANCES)
    run.add_argument("--only", default="", help="substring filter on instance names")

    args = parser.parse_args(argv)

    catalog_rows = _read_csv(DATA / "catalog.csv")
    catalog = {row["instance"]: row for row in catalog_rows}

    if args.command == "pick":
        if BENCH_LIST.exists():
            print(f"{BENCH_LIST} already exists, delete it first to pick again")
            return 1

        uids = pick_bench(catalog_rows, _read_csv(DATA / "leaderboard.csv"))
        header = "# fixed benchmark set, written once by python -m solver.bench pick\n"
        BENCH_LIST.write_text(header + "\n".join(uids) + "\n")
        print(f"wrote {len(uids)} instances to {BENCH_LIST}")
        return 0

    logging.basicConfig(level=logging.INFO, format="%(asctime)s %(message)s", datefmt="%H:%M:%S")

    uids = read_bench_list()
    if args.only:
        uids = [uid for uid in uids if args.only in uid]

    store = BestStore(DATA / "solutions" / "best", DATA / "solutions" / "candidates")
    best_before = dict(store.objective)

    jobs = bench_jobs(
        uids,
        catalog,
        args.config,
        source=args.instances,
        best_folder=store.folder,
        seconds=args.seconds,
    )

    # a fresh log per run, so no job is skipped as already done
    stamp = time.strftime("%Y%m%d-%H%M%S")
    run_folder = DATA / "bench"
    run_folder.mkdir(parents=True, exist_ok=True)
    job_log = JobLog(run_folder / f"{stamp}.jsonl")

    mine(
        jobs,
        workers=args.workers,
        budget_seconds=24 * 3600,
        store=store,
        job_log=job_log,
        leaderboard=run_folder / f"{stamp}.leaderboard.csv",
        catalog=catalog,
    )

    ours = {}
    for record in job_log.records:
        if record["objective"] is not None:
            ours[record["instance"]] = record["objective"]

    result = score_run(uids, catalog, best_before, ours)
    _print_report(result, args.config)

    # promotions change the main leaderboard, rebuilt from the mining log
    promoted = [uid for uid in uids if store.objective.get(uid) != best_before.get(uid)]
    if promoted:
        mining_log = JobLog(DATA / "jobs.jsonl")
        write_leaderboard(DATA / "leaderboard.csv", catalog, store, mining_log.records)
        print(f"promoted new bests: {', '.join(promoted)}")

    summary_path = run_folder / f"{stamp}.summary.json"
    summary_path.write_text(json.dumps({"config": args.config, **result}, indent=2))

    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
