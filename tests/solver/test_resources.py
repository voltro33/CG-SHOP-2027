"""
The semaphores must never hand out more than they hold, and the scheduler
must only start jobs whose whole demand fits.
"""

import time

import pytest

from solver.jobs import Job
from solver.mine import HEAD_WAIT_SECONDS, _next_admissible, job_demand
from solver.resources import CountingSemaphore, Demand, Resources, system_resources


def make_job(uid: str, cells: int) -> Job:
    return Job(instance=uid, source="x", cells=cells, k=1, round=0, method="segments-h")


def test_semaphore_never_goes_below_zero():
    semaphore = CountingSemaphore("memory", 10)

    assert semaphore.try_acquire(6)
    assert not semaphore.try_acquire(5)
    assert semaphore.try_acquire(4)
    assert semaphore.free == 0

    semaphore.release(10)
    assert semaphore.free == 10


def test_semaphore_rejects_impossible_requests_and_extra_releases():
    semaphore = CountingSemaphore("core", 4)

    with pytest.raises(ValueError, match="only 4 exist"):
        semaphore.try_acquire(5)

    with pytest.raises(ValueError, match="more core permits than were taken"):
        semaphore.release(1)


def test_demand_is_taken_from_both_or_from_neither():
    resources = Resources(cores=4, memory_mb=1000)

    assert resources.try_acquire(Demand(cores=1, memory_mb=900))

    # cores are free but memory is not, so no cores may be kept either
    assert not resources.try_acquire(Demand(cores=1, memory_mb=200))
    assert resources.cores.free == 3
    assert resources.memory.free == 100


def test_a_demand_bigger_than_the_machine_is_cut_to_fit():
    resources = Resources(cores=2, memory_mb=1000)

    fitted = resources.fit(Demand(cores=1, memory_mb=5000))

    assert fitted == Demand(cores=1, memory_mb=1000)
    assert resources.try_acquire(fitted)


def test_system_resources_are_sized_from_the_machine():
    resources = system_resources()

    assert resources.cores.capacity >= 1
    assert resources.memory.capacity >= 1


def test_small_jobs_fill_in_while_a_big_one_waits():
    big = make_job("big", 9_000_000)
    small = make_job("small", 10_000)
    # one megabyte short of running both side by side
    memory_mb = job_demand(big).memory_mb + job_demand(small).memory_mb - 1

    resources = Resources(cores=4, memory_mb=memory_mb)
    assert resources.try_acquire(job_demand(small))

    # the big job does not fit next to the running small one, the next small one does
    index = _next_admissible([big, small], resources, waiting_since=time.monotonic())

    assert index == 1


def test_a_starving_head_blocks_everything_behind_it():
    big = make_job("big", 9_000_000)
    small = make_job("small", 10_000)
    # one megabyte short of running both side by side
    memory_mb = job_demand(big).memory_mb + job_demand(small).memory_mb - 1

    resources = Resources(cores=4, memory_mb=memory_mb)
    assert resources.try_acquire(job_demand(small))

    long_ago = time.monotonic() - HEAD_WAIT_SECONDS - 1
    index = _next_admissible([big, small], resources, waiting_since=long_ago)

    assert index is None
    assert resources.memory.free == job_demand(big).memory_mb - 1
