"""
The miner must survive dead workers, resume without redoing work, and only
ever promote strictly better solutions.
"""

import csv
import os
from pathlib import Path

import pytest

from solver.jobs import build_jobs
from solver.mine import BestStore, JobLog, JobResult, mine, run_job
from solver.resources import Resources

EXAMPLES = Path("example_instances.zip")
CATALOG = Path("data/catalog.csv")

needs_data = pytest.mark.skipif(
    not EXAMPLES.exists() or not CATALOG.exists(), reason="needs examples and catalog"
)

# the smallest example instance keeps these tests quick
SMALL_INSTANCE = "srpg_246_821_1s3"


def crash_on_lanes(job):
    """
    A runner that kills its own worker process on plain lanes jobs, the way
    running out of memory would. Module level so worker processes can load it.
    """
    if job.method == "lanes-h":
        os._exit(1)

    return run_job(job)


def read_catalog() -> dict[str, dict[str, str]]:
    with CATALOG.open() as file:
        return {row["instance"]: row for row in csv.DictReader(file)}


def run_small_mine(tmp_path: Path, runner=None) -> tuple[BestStore, JobLog]:
    catalog = read_catalog()
    jobs = build_jobs(EXAMPLES, catalog, max_round=1, only=SMALL_INSTANCE)

    store = BestStore(tmp_path / "best", tmp_path / "candidates")
    job_log = JobLog(tmp_path / "jobs.jsonl")

    mine(
        jobs,
        resources=Resources(cores=2, memory_mb=4096),
        budget_seconds=300,
        store=store,
        job_log=job_log,
        leaderboard=tmp_path / "leaderboard.csv",
        catalog=catalog,
        runner=runner,
    )
    return store, job_log


@needs_data
def test_dead_worker_does_not_take_the_other_jobs_down(tmp_path):
    store, job_log = run_small_mine(tmp_path, runner=crash_on_lanes)

    by_label = {}
    for record in job_log.records:
        by_label[record["label"]] = record

    # the crashing job is given up on, every other job still succeeds
    assert "crashed" in by_label["lanes-h"]["error"]
    for label, record in by_label.items():
        if label != "lanes-h":
            assert record["objective"] is not None, (label, record["error"])

    assert SMALL_INSTANCE in store.objective


@needs_data
def test_restart_skips_finished_jobs_and_keeps_bests(tmp_path):
    store, job_log = run_small_mine(tmp_path)
    first_best = store.objective[SMALL_INSTANCE]
    first_count = len(job_log.records)

    store, job_log = run_small_mine(tmp_path)

    assert len(job_log.records) == first_count
    assert store.objective[SMALL_INSTANCE] == first_best


def test_best_store_only_promotes_strictly_better(tmp_path):
    store = BestStore(tmp_path / "best", tmp_path / "candidates")

    def result(objective: int, job: str) -> JobResult:
        # a minimal valid solution file body is enough for the store
        body = (
            '{"content_type":"CGSHOP2027_Solution","instance_uid":"x",'
            '"tours":[{"x":[0],"y":[0]}],"meta":{"job":"' + job + '"}}'
        )
        return JobResult(job, "x", 0, job, objective, 1.0, None, body)

    assert store.offer(result(100, "a"))
    assert not store.offer(result(100, "b"))
    assert not store.offer(result(120, "c"))
    assert store.offer(result(90, "d"))

    assert store.objective["x"] == 90
    assert store.label["x"] == "d"


def test_job_log_ignores_a_torn_last_line(tmp_path):
    path = tmp_path / "jobs.jsonl"
    path.write_text('{"job_id": "a"}\n{"job_id": "b"}\n{"job_id": "c", "obj')

    job_log = JobLog(path)

    assert job_log.done_ids() == {"a", "b"}
