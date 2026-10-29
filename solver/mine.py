"""
Parallel mining: run many `(instance, config)` jobs on all cores, keep every
verified result, and promote the best one per instance.

    python -m solver.mine --hours 8                       # challenge instances
    python -m solver.mine --instances example_instances.zip --hours 0.2

How it stays safe:

- Workers verify their own result with the official verifier, and only a
  verified solution leaves a worker.
- Only the main process touches `best/`, and it writes through a temp file
  plus `os.replace`, so a crash never leaves half a file behind.
- Every finished job is appended to `jobs.jsonl`. A restart skips those jobs
  and starts from the stored bests, so killing the miner loses nothing that
  was verified.
"""

import argparse
import csv
import json
import logging
import os
import sys
import time
from concurrent.futures import FIRST_COMPLETED, Future, ProcessPoolExecutor, wait
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

from cgshop2027_pyutils.instance_database import InstanceDatabase
from cgshop2027_pyutils.io import read_solution
from cgshop2027_pyutils.verify import SolutionValidator

from .jobs import Job, build_jobs
from .model import Problem
from .pipeline import SolveConfig, solve
from .report import write_leaderboard

log = logging.getLogger("mine")

DATA = Path("data")
DEFAULT_INSTANCES = DATA / "instances" / "challenge_v0" / "instances"

# instances this big get a cap on how many run at once, to protect memory
HUGE_CELLS = 3_000_000
MAX_HUGE_IN_FLIGHT = 3

# keep a candidate file only if it is this close to the best so far
CANDIDATE_SLACK = 1.05

LEADERBOARD_EVERY_SECONDS = 60


# worker side


@dataclass(frozen=True)
class JobResult:
    job_id: str
    instance: str
    round: int
    label: str
    objective: int | None
    seconds: float
    error: str | None
    solution_json: str | None = field(default=None, repr=False)


# one instance stays loaded per worker, since jobs for it often come in a row
_loaded: dict[str, tuple[Problem, SolutionValidator]] = {}


def _load(source: str, uid: str) -> tuple[Problem, SolutionValidator]:
    if uid not in _loaded:
        _loaded.clear()

        instance = InstanceDatabase(source)[uid]
        _loaded[uid] = (Problem.from_instance(instance), SolutionValidator(instance))

    return _loaded[uid]


def run_job(job: Job) -> JobResult:
    """
    Worker entry point. Never raises: any failure comes back as a result
    with an error, so one bad job cannot take the pool down.
    """
    started = time.monotonic()

    try:
        problem, validator = _load(job.source, job.instance)
        config = SolveConfig(
            method=job.method, params=dict(job.params), partition=job.partition
        )
        attempt = solve(problem, validator, config, job.time_limit)
    except Exception as error:
        return JobResult(
            job.job_id,
            job.instance,
            job.round,
            job.label,
            None,
            round(time.monotonic() - started, 2),
            f"{type(error).__name__}: {error}",
        )

    if not attempt.feasible:
        first_error = attempt.errors[0] if attempt.errors else "infeasible"
        return JobResult(
            job.job_id,
            job.instance,
            job.round,
            job.label,
            None,
            attempt.seconds,
            first_error[:300],
        )

    # record every parameter with the solution so it can be reproduced
    solution = attempt.solution.model_copy(
        update={"meta": {"job": job.job_id, "params": asdict(job)}}
    )

    return JobResult(
        job.job_id,
        job.instance,
        job.round,
        job.label,
        attempt.objective,
        attempt.seconds,
        None,
        solution.model_dump_json(),
    )


# main process side


class BestStore:
    """
    The best verified objective per instance, mirrored to one file each.
    """

    def __init__(self, folder: Path, candidates: Path) -> None:
        self.folder = folder
        self.candidates = candidates
        self.folder.mkdir(parents=True, exist_ok=True)
        self.candidates.mkdir(parents=True, exist_ok=True)

        self.objective: dict[str, int] = {}
        self.label: dict[str, str] = {}

        for path in sorted(folder.glob("*.solution.json")):
            solution = read_solution(path)
            self.objective[solution.instance_uid] = solution.objective
            self.label[solution.instance_uid] = str(solution.meta.get("job", "?"))

    def offer(self, result: JobResult) -> bool:
        """
        Stores the result as a candidate if it is close to the best, and
        promotes it if it is strictly better. Returns True on promotion.
        """
        current = self.objective.get(result.instance)
        is_better = current is None or result.objective < current

        is_close = current is None or result.objective <= current * CANDIDATE_SLACK
        if is_close:
            name = _safe_name(result.job_id) + ".solution.json"
            _atomic_write(self.candidates / result.instance / name, result.solution_json)

        if not is_better:
            return False

        target = self.folder / f"{result.instance}.solution.json"
        _atomic_write(target, result.solution_json)

        self.objective[result.instance] = result.objective
        self.label[result.instance] = result.label
        return True


class JobLog:
    """
    Append only record of finished jobs, one JSON object per line.
    """

    def __init__(self, path: Path) -> None:
        self.path = path
        self.records: list[dict[str, Any]] = []

        if path.exists():
            for line in path.read_text().splitlines():
                # a crash mid write can leave a broken last line, skip it
                try:
                    self.records.append(json.loads(line))
                except json.JSONDecodeError:
                    continue

    def done_ids(self) -> set[str]:
        return {record["job_id"] for record in self.records}

    def append(self, result: JobResult) -> None:
        record = asdict(result)
        del record["solution_json"]

        self.records.append(record)

        with self.path.open("a") as file:
            file.write(json.dumps(record) + "\n")
            file.flush()
            os.fsync(file.fileno())


def mine(
    jobs: list[Job],
    *,
    workers: int,
    budget_seconds: float,
    store: BestStore,
    job_log: JobLog,
    leaderboard: Path,
    catalog: dict[str, dict[str, str]],
) -> None:
    done = job_log.done_ids()
    pending = [job for job in jobs if job.job_id not in done]
    log.info("%d jobs total, %d already done, %d to run", len(jobs), len(done), len(pending))

    deadline = time.monotonic() + budget_seconds
    in_flight: dict[Future, Job] = {}
    last_board = 0.0

    with ProcessPoolExecutor(max_workers=workers) as pool:
        while pending or in_flight:
            # keep every worker busy until time runs out
            while pending and len(in_flight) < workers and time.monotonic() < deadline:
                job = _next_runnable(pending, in_flight.values())
                if job is None:
                    break

                in_flight[pool.submit(run_job, job)] = job

            if not in_flight:
                break

            finished, _ = wait(in_flight, timeout=30, return_when=FIRST_COMPLETED)

            for future in finished:
                job = in_flight.pop(future)
                _handle(future, job, store, job_log)

            if time.monotonic() - last_board > LEADERBOARD_EVERY_SECONDS:
                write_leaderboard(leaderboard, catalog, store, job_log.records)
                last_board = time.monotonic()

            # past the deadline, drop what has not started and let the rest finish
            if time.monotonic() >= deadline and pending:
                log.info("budget used up, %d jobs left unstarted", len(pending))
                pending = []

    write_leaderboard(leaderboard, catalog, store, job_log.records)


def _next_runnable(pending: list[Job], running) -> Job | None:
    """
    Takes the first pending job that does not break the huge instance cap.
    """
    huge_running = sum(1 for job in running if job.cells >= HUGE_CELLS)

    for index, job in enumerate(pending):
        if job.cells >= HUGE_CELLS and huge_running >= MAX_HUGE_IN_FLIGHT:
            continue

        return pending.pop(index)

    return None


def _handle(future: Future, job: Job, store: BestStore, job_log: JobLog) -> None:
    try:
        result = future.result()
    except Exception as error:
        # the worker process itself died, for example out of memory
        result = JobResult(
            job.job_id, job.instance, job.round, job.label, None, 0.0,
            f"worker crashed: {type(error).__name__}: {error}",
        )  # fmt: skip

    promoted = False
    if result.objective is not None:
        promoted = store.offer(result)

    job_log.append(result)

    if result.error:
        log.warning("FAIL %-34s %-40s %s", job.instance, job.label, result.error[:120])
    else:
        mark = "BEST" if promoted else "    "
        log.info(
            "%s %-34s %-40s %8d  %6.1fs",
            mark, job.instance, job.label, result.objective, result.seconds,
        )  # fmt: skip


def _atomic_write(path: Path, text: str) -> None:
    """
    Write to a temp file, then swap it in. OneDrive sometimes holds a file
    open for a moment while syncing, so the swap is retried a few times.
    """
    path.parent.mkdir(parents=True, exist_ok=True)
    temp = path.with_suffix(path.suffix + ".tmp")
    temp.write_text(text)

    for attempt in range(10):
        try:
            os.replace(temp, path)
            return
        except PermissionError:
            time.sleep(0.2 * (attempt + 1))

    os.replace(temp, path)


def _safe_name(job_id: str) -> str:
    allowed = []
    for char in job_id:
        if char.isalnum() or char in "-_.=":
            allowed.append(char)
        else:
            allowed.append("_")

    return "".join(allowed)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--instances", type=Path, default=DEFAULT_INSTANCES)
    parser.add_argument("--catalog", type=Path, default=DATA / "catalog.csv")
    parser.add_argument("--out", type=Path, default=DATA)
    parser.add_argument("--hours", type=float, default=8.0)
    parser.add_argument("--workers", type=int, default=max(1, (os.cpu_count() or 2) - 1))
    parser.add_argument("--max-round", type=int, default=99)
    parser.add_argument("--only", default="", help="substring filter on instance names")
    args = parser.parse_args(argv)

    args.out.mkdir(parents=True, exist_ok=True)
    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s %(message)s",
        datefmt="%H:%M:%S",
        handlers=[
            logging.StreamHandler(sys.stdout),
            logging.FileHandler(args.out / "mine.log"),
        ],
    )

    catalog = _read_catalog(args.catalog)
    jobs = build_jobs(args.instances, catalog, max_round=args.max_round, only=args.only)

    store = BestStore(args.out / "solutions" / "best", args.out / "solutions" / "candidates")
    job_log = JobLog(args.out / "jobs.jsonl")

    mine(
        jobs,
        workers=args.workers,
        budget_seconds=args.hours * 3600,
        store=store,
        job_log=job_log,
        leaderboard=args.out / "leaderboard.csv",
        catalog=catalog,
    )
    return 0


def _read_catalog(path: Path) -> dict[str, dict[str, str]]:
    with path.open() as file:
        return {row["instance"]: row for row in csv.DictReader(file)}


if __name__ == "__main__":
    sys.exit(main())
