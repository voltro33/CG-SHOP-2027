"""
Polish finished solutions window by window, the improvement half of the
search: mining only builds tours from scratch, this works on the best one.

    python -m solver.polish --seconds 300
    python -m solver.polish --seconds 120 --side 16 --only _1s

The longest tour is cut into square windows of anchors, laid as tiles with a
random offset. The windows are re-solved with CP-SAT (`window.py`) in random
order, and every shorter walk is spliced in at once. When another tour
becomes the longest, the tiles move to that tour. Each pass draws a new
offset, so window borders do not stay in the same place.

Every polished solution is checked by the official verifier before it can
enter the best store, and the store only takes strictly better ones.
"""

import argparse
import json
import random
import time
from concurrent.futures import ProcessPoolExecutor, as_completed
from dataclasses import dataclass
from pathlib import Path

import numpy as np

from cgshop2027_pyutils.instance_database import InstanceDatabase
from cgshop2027_pyutils.io import read_solution
from cgshop2027_pyutils.schemas import CGSHOP2027Solution
from cgshop2027_pyutils.verify import SolutionValidator

from .bench import _print_report, _read_csv, read_bench_list, score_run
from .mine import DEFAULT_INSTANCES, BestStore, JobLog, JobResult
from .model import Problem
from .report import write_leaderboard
from .resources import system_resources
from .tours import compress, densify, to_cutter_tour
from .window import Window, solve_window

__all__ = ["PolishStats", "polish_tours"]

DATA = Path("data")

# window side in anchors, the pilot proved most 20 x 20 windows optimal in seconds
DEFAULT_SIDE = 20
DEFAULT_WINDOW_SECONDS = 3.0

# a round number no mining round uses
POLISH_ROUND = -2


@dataclass(frozen=True)
class PolishStats:
    windows: int
    improved: int
    saved_steps: int
    passes: int


def polish_tours(
    problem: Problem,
    tours: list[np.ndarray],
    *,
    deadline: float,
    side: int = DEFAULT_SIDE,
    window_seconds: float = DEFAULT_WINDOW_SECONDS,
    workers: int = 1,
    seed: int = 0,
) -> tuple[list[np.ndarray], PolishStats]:
    """
    Shortens the longest of the dense closed `tours` until the deadline.
    Returns new tours, never longer than the old ones.
    """
    rng = random.Random(seed)
    tours = list(tours)

    windows = 0
    improved = 0
    saved = 0
    passes = 0

    while time.monotonic() < deadline:
        passes += 1
        longest = _longest(tours)
        tiles = _tiles(tours[longest], side, rng)

        for window in tiles:
            if time.monotonic() >= deadline:
                break

            seconds_left = deadline - time.monotonic()
            limit = min(window_seconds, seconds_left)
            result = solve_window(
                problem, tours, longest, window, time_limit=limit, workers=workers
            )
            if result is None:
                continue

            windows += 1
            if result.positions is None or result.saved <= 0:
                continue

            tours = list(tours)
            tours[longest] = result.positions
            improved += 1
            saved += result.saved

            # once another tour is the longest, this one no longer sets the objective
            if _longest(tours) != longest:
                break

    stats = PolishStats(windows=windows, improved=improved, saved_steps=saved, passes=passes)

    return tours, stats


def _longest(tours: list[np.ndarray]) -> int:
    lengths = [len(tour) for tour in tours]

    return int(np.argmax(lengths))


def _tiles(positions: np.ndarray, side: int, rng: random.Random) -> list[Window]:
    """
    Square windows over the bounding box of a tour, offset at random, in
    random order. Windows the tour never enters are left out.
    """
    low = positions.min(axis=0) - rng.randrange(side)
    columns = (positions[:, 0] - low[0]) // side
    rows = (positions[:, 1] - low[1]) // side

    visited = sorted(set(zip(columns.tolist(), rows.tolist(), strict=True)))
    rng.shuffle(visited)

    tiles = []
    for column, row in visited:
        x0 = int(low[0]) + column * side
        y0 = int(low[1]) + row * side
        tiles.append(Window(x0, y0, x0 + side, y0 + side))

    return tiles


# one instance per worker process


@dataclass(frozen=True)
class PolishJob:
    instance: str
    source: str
    best_path: str
    seconds: float
    side: int
    window_seconds: float
    seed: int


def run_polish(job: PolishJob) -> tuple[JobResult, PolishStats | None]:
    """
    Worker entry point. Never raises, a failure comes back as an error.
    """
    started = time.monotonic()
    job_id = f"{job.instance}|polish/side={job.side}|s{job.seed}"

    try:
        instance = InstanceDatabase(job.source)[job.instance]
        problem = Problem.from_instance(instance)
        best = read_solution(Path(job.best_path))

        tours = []
        for tour in best.tours:
            tours.append(densify(list(zip(tour.x, tour.y, strict=True)), closed=True))

        deadline = started + job.seconds
        polished, stats = polish_tours(
            problem,
            tours,
            deadline=deadline,
            side=job.side,
            window_seconds=job.window_seconds,
            seed=job.seed,
        )

        cutter_tours = [to_cutter_tour(compress(tour)) for tour in polished]
        solution = CGSHOP2027Solution(
            instance_uid=job.instance,
            tours=cutter_tours,
            meta={"job": job_id, "polished_from": best.meta.get("job", "?")},
        )

        errors = list(SolutionValidator(instance).check_for_errors(solution))
    except Exception as error:
        seconds = round(time.monotonic() - started, 2)
        failed = JobResult(job_id, job.instance, POLISH_ROUND, "polish", None, seconds, repr(error))
        return failed, None

    seconds = round(time.monotonic() - started, 2)
    if errors:
        failed = JobResult(job_id, job.instance, POLISH_ROUND, "polish", None, seconds, str(errors[0])[:300])
        return failed, stats

    result = JobResult(
        job_id,
        job.instance,
        POLISH_ROUND,
        "polish",
        solution.objective,
        seconds,
        None,
        solution.model_dump_json(),
    )

    return result, stats


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("--seconds", type=float, default=300.0, help="polishing time per instance")
    parser.add_argument("--side", type=int, default=DEFAULT_SIDE)
    parser.add_argument("--window-seconds", type=float, default=DEFAULT_WINDOW_SECONDS)
    parser.add_argument("--cores", type=int, default=None, help="instances at once, default all but one")
    parser.add_argument("--seed", type=int, default=0)
    parser.add_argument("--only", default="", help="substring filter on instance names")
    parser.add_argument("--instances", type=Path, default=DEFAULT_INSTANCES)
    args = parser.parse_args(argv)

    catalog = {row["instance"]: row for row in _read_csv(DATA / "catalog.csv")}
    uids = [uid for uid in read_bench_list() if args.only in uid]

    store = BestStore(DATA / "solutions" / "best", DATA / "solutions" / "candidates")
    best_before = {uid: store.objective[uid] for uid in uids if uid in store.objective}

    cores = args.cores or system_resources().cores.capacity

    jobs = []
    for uid in uids:
        if uid not in best_before:
            continue

        job = PolishJob(
            instance=uid,
            source=str(args.instances),
            best_path=str(store.folder / f"{uid}.solution.json"),
            seconds=args.seconds,
            side=args.side,
            window_seconds=args.window_seconds,
            seed=args.seed,
        )
        jobs.append(job)

    ours: dict[str, int] = {}
    promoted = []
    cpu_seconds = 0.0
    totals = {"windows": 0, "improved": 0, "saved_steps": 0}

    with ProcessPoolExecutor(max_workers=cores) as pool:
        futures = [pool.submit(run_polish, job) for job in jobs]

        for future in as_completed(futures):
            result, stats = future.result()
            cpu_seconds += result.seconds

            if stats is not None:
                totals["windows"] += stats.windows
                totals["improved"] += stats.improved
                totals["saved_steps"] += stats.saved_steps

            before = best_before[result.instance]
            if result.objective is None:
                print(f"FAIL {result.instance:36} {result.error}", flush=True)
                continue

            ours[result.instance] = result.objective
            if store.offer(result):
                promoted.append(result.instance)

            windows = stats.windows if stats else 0
            improved = stats.improved if stats else 0
            print(
                f"{result.instance:36} {before:>8} {result.objective:>8}"
                f"  windows {windows:>5}  improved {improved:>4}  {result.seconds:>7.1f}s",
                flush=True,
            )

    report = score_run(uids, catalog, best_before, ours)
    _print_report(report, f"polish/side={args.side}/window={args.window_seconds}s")

    # the report rounds, a polish gain is small enough to need the exact sum
    points = 0.0
    for uid, objective in ours.items():
        bound = int(catalog[uid]["lower_bound"])
        points += (bound / objective) ** 2 - (bound / best_before[uid]) ** 2

    cpu_hours = cpu_seconds / 3600
    summary = {
        **totals,
        "cpu_hours": round(cpu_hours, 2),
        "points": round(points, 3),
        "points_per_cpu_hour": round(points / cpu_hours, 3) if cpu_hours else None,
    }
    print(json.dumps(summary, indent=2))

    # promotions change the main leaderboard, rebuilt from the mining log
    if promoted:
        mining_log = JobLog(DATA / "jobs.jsonl")
        write_leaderboard(DATA / "leaderboard.csv", catalog, store, mining_log.records)
        print("promoted new bests: " + ", ".join(promoted))

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
