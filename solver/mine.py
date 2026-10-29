"""
Parallel mining: run many `(instance, config)` jobs on all cores, keep every
verified result, and promote the best one per instance.

Each job runs in its own worker process, spawned only once the core and
memory semaphores in `resources.py` have room for it. So the number of
workers follows what the machine can hold: many small jobs at once, or a few
big ones.

    python -m solver.mine --hours 8                       # challenge instances
    python -m solver.mine --instances example_instances.zip --hours 0.2

How it stays safe:

- Workers verify their own result with the official verifier, and only a
  verified solution leaves a worker.
- Only the main process touches `best/`, and it writes through a temp file
  plus `os.replace`, so a crash never leaves half a file behind.
- A worker that dies, from running out of memory or anything else, loses
  only its own job, which is logged as failed.
- Every finished job is appended to `jobs.jsonl`. A restart skips those jobs
  and starts from the stored bests, so killing the miner loses nothing that
  was verified.
"""

import argparse
import csv
import json
import logging
import multiprocessing
import multiprocessing.process
import os
import sys
import time
from collections.abc import Callable
from dataclasses import asdict, dataclass, field
from multiprocessing.connection import Connection
from multiprocessing.connection import wait as connection_wait
from pathlib import Path
from typing import Any

from cgshop2027_pyutils.instance_database import InstanceDatabase
from cgshop2027_pyutils.io import read_solution
from cgshop2027_pyutils.verify import SolutionValidator

from .jobs import Job, build_jobs
from .model import Problem
from .pipeline import SolveConfig, solve
from .report import write_leaderboard
from .resources import Demand, Resources, system_resources

log = logging.getLogger("mine")

DATA = Path("data")
DEFAULT_INSTANCES = DATA / "instances" / "challenge_v0" / "instances"

# peak memory of one job grows with the box of the region, about 50 MB plus
# 26 bytes per box cell measured on the largest challenge instances; these
# leave half again as much room
BASE_MEMORY_MB = 100
MEMORY_BYTES_PER_BOX_CELL = 40

# once the head of the queue has waited this long, nothing jumps ahead of it
HEAD_WAIT_SECONDS = 60

# a worker this far over its own time limit is stuck and gets killed
KILL_AFTER_LIMIT = 2.0
KILL_GRACE_SECONDS = 120

# how often the scheduler wakes up when no worker finishes
WAKE_SECONDS = 5

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


def _load(source: str, uid: str) -> tuple[Problem, SolutionValidator]:
    instance = InstanceDatabase(source)[uid]

    return Problem.from_instance(instance), SolutionValidator(instance)


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
    resources: Resources,
    budget_seconds: float,
    store: BestStore,
    job_log: JobLog,
    leaderboard: Path,
    catalog: dict[str, dict[str, str]],
    runner: Callable[[Job], JobResult] | None = None,
) -> None:
    """
    Runs `jobs` in order, each in its own worker process, as many at once as
    the core and memory semaphores of `resources` allow.
    """
    # tests swap in their own runner, it must be a module level function
    if runner is None:
        runner = run_job

    done = job_log.done_ids()
    pending = [job for job in jobs if job.job_id not in done]
    log.info("%d jobs total, %d already done, %d to run", len(jobs), len(done), len(pending))
    log.info("resources: %s", resources.describe())

    context = multiprocessing.get_context("spawn")
    deadline = time.monotonic() + budget_seconds
    running: list[Running] = []
    head_id: str | None = None
    waiting_since: float | None = None
    last_board = 0.0

    try:
        while pending or running:
            # start every job the semaphores have room for
            while pending and time.monotonic() < deadline:
                index = _next_admissible(pending, resources, waiting_since)
                if index is None:
                    break

                job = pending.pop(index)
                running.append(_start(context, runner, job, resources))

            # how long the head of the queue has waited for room
            if pending and pending[0].job_id != head_id:
                head_id = pending[0].job_id
                waiting_since = time.monotonic()

            if not running:
                break

            _wait_for_any(running, timeout=WAKE_SECONDS)

            still_running = []
            for worker in running:
                result = _collect(worker)
                if result is None:
                    still_running.append(worker)
                    continue

                resources.release(worker.demand)
                _handle(result, worker.job, store, job_log)
            running = still_running

            if time.monotonic() - last_board > LEADERBOARD_EVERY_SECONDS:
                write_leaderboard(leaderboard, catalog, store, job_log.records)
                last_board = time.monotonic()

            # past the deadline, drop what has not started and let the rest finish
            if time.monotonic() >= deadline and pending:
                log.info("budget used up, %d jobs left unstarted", len(pending))
                pending = []
    finally:
        for worker in running:
            worker.process.kill()
            worker.process.join()

    write_leaderboard(leaderboard, catalog, store, job_log.records)


@dataclass
class Running:
    """
    One job in its worker process, with the permits it holds.
    """

    job: Job
    process: multiprocessing.process.BaseProcess
    connection: Connection
    demand: Demand
    started: float


def job_demand(job: Job) -> Demand:
    """
    One core, and the peak memory measured for jobs of this size.
    """
    # the box holds every cell, so the cells stand in when no box is known
    box_cells = max(job.box_cells, job.cells)
    memory_mb = BASE_MEMORY_MB + box_cells * MEMORY_BYTES_PER_BOX_CELL // 2**20

    return Demand(cores=1, memory_mb=memory_mb)


def _fitted_demand(job: Job, resources: Resources) -> Demand:
    return resources.fit(job_demand(job))


def _next_admissible(
    pending: list[Job], resources: Resources, waiting_since: float | None
) -> int | None:
    """
    Takes the permits of the first pending job that fits and returns its
    index. Once the head of the queue has waited too long, only the head may
    start, so a big job is never starved by a stream of small ones.
    """
    head_starving = (
        waiting_since is not None and time.monotonic() - waiting_since > HEAD_WAIT_SECONDS
    )
    candidates = range(1) if head_starving else range(len(pending))

    for index in candidates:
        if resources.try_acquire(_fitted_demand(pending[index], resources)):
            return index

    return None


def _start(context, runner: Callable[[Job], JobResult], job: Job, resources: Resources) -> Running:
    receiver, sender = context.Pipe(duplex=False)

    process = context.Process(target=_worker_main, args=(runner, job, sender), daemon=True)
    process.start()

    # the child holds its own copy of the sending end
    sender.close()

    return Running(job, process, receiver, _fitted_demand(job, resources), time.monotonic())


def _worker_main(runner: Callable[[Job], JobResult], job: Job, sender: Connection) -> None:
    result = runner(job)
    sender.send(result)
    sender.close()


def _wait_for_any(running: list[Running], timeout: float) -> None:
    handles = []
    for worker in running:
        handles.append(worker.connection)
        handles.append(worker.process.sentinel)

    connection_wait(handles, timeout=timeout)


def _collect(worker: Running) -> JobResult | None:
    """
    The result of a finished worker, a failure if it died or ran far over
    its time, or None while it is still working.
    """
    job = worker.job

    # read before joining, a big solution would block the child otherwise
    if _has_data(worker.connection):
        try:
            result = worker.connection.recv()
        except EOFError:
            result = None

        worker.process.join()
        worker.connection.close()

        if result is not None:
            return result

        return _failure(job, "worker died before sending its result")

    if not worker.process.is_alive():
        worker.process.join()
        worker.connection.close()

        # the process itself died, for example out of memory
        code = worker.process.exitcode
        return _failure(job, f"worker crashed with exit code {code}")

    elapsed = time.monotonic() - worker.started
    if elapsed > job.time_limit * KILL_AFTER_LIMIT + KILL_GRACE_SECONDS:
        worker.process.kill()
        worker.process.join()
        worker.connection.close()

        return _failure(job, f"worker killed after {elapsed:.0f}s, over its time limit")

    return None


def _has_data(connection: Connection) -> bool:
    # on windows the pipe of a dead child raises instead of reading empty
    try:
        return connection.poll()
    except (BrokenPipeError, EOFError):
        return False


def _failure(job: Job, message: str) -> JobResult:
    return JobResult(job.job_id, job.instance, job.round, job.label, None, 0.0, message)


def _handle(result: JobResult, job: Job, store: BestStore, job_log: JobLog) -> None:
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
    parser.add_argument("--cores", type=int, default=None, help="core permits, default all but one")
    parser.add_argument("--memory-gb", type=float, default=None, help="memory permits, default most of what is free")
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

    memory_mb = None if args.memory_gb is None else int(args.memory_gb * 1024)
    resources = system_resources(cores=args.cores, memory_mb=memory_mb)

    mine(
        jobs,
        resources=resources,
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
