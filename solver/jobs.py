"""
The job grid for mining: which configs run on which instance, in what order.

Jobs come in rounds, and every instance finishes a round before anyone
starts the next one, so the cheap safe configs land first:

    round 0   feasibility    segments, both orientations, default knobs
    round 1   baseline       plain lanes, cover-tsp on small instances,
                             band partitions when k > 1
    round 2   gap sweep      segments with other gap factors
    round 3   shift sweep    segments with lanes shifted by a quarter, half
                             and three quarters of a lane height
    round 4   cross          partitions combined with gap and shift

Rounds 0 and 1 together are the Phase 4 baseline in the leaderboard.
Within a round the biggest instances go first, so the long jobs start early
and the short ones fill the gaps at the end.
"""

from dataclasses import dataclass
from pathlib import Path

from cgshop2027_pyutils.instance_database import InstanceDatabase

__all__ = ["BASELINE_ROUNDS", "Job", "build_jobs"]

BASELINE_ROUNDS = (0, 1)

# cover-tsp uses a quadratic nearest neighbour, keep it to small instances
COVER_TSP_MAX_CELLS = 60_000

GAP_FACTORS = (0.0, 1.0, 4.0, 8.0)
SHIFTS = (0.25, 0.5, 0.75)


@dataclass(frozen=True)
class Job:
    instance: str
    source: str
    cells: int
    k: int
    round: int
    method: str
    params: tuple[tuple[str, float], ...] = ()
    partition: str | None = None
    time_limit: float = 120.0
    seed: int = 0

    @property
    def label(self) -> str:
        parts = [self.method]

        for key, value in self.params:
            parts.append(f"{key}={value:g}")

        if self.partition:
            parts.append(self.partition)

        return "/".join(parts)

    @property
    def job_id(self) -> str:
        return f"{self.instance}|{self.label}|s{self.seed}"


def time_limit_for(cells: int) -> float:
    """
    A wall clock cap per job. The local search usually converges well
    before this, so it mostly guards against runaway jobs.
    """
    if cells > 1_000_000:
        return 600.0

    return 120.0


def configs_for(k: int, cells: int, round_number: int) -> list[dict]:
    """
    The configs of one round for one instance, as keyword dicts for `Job`.
    """
    orientations = ("segments-h", "segments-v")
    partitions = ("bands-x", "bands-y") if k > 1 else ()

    configs: list[dict] = []

    if round_number == 0:
        for method in orientations:
            configs.append({"method": method})

    elif round_number == 1:
        configs.append({"method": "lanes-h"})
        configs.append({"method": "lanes-v"})

        if cells <= COVER_TSP_MAX_CELLS:
            configs.append({"method": "cover-tsp"})

        for method in orientations:
            for partition in partitions:
                configs.append({"method": method, "partition": partition})

    elif round_number == 2:
        for method in orientations:
            for gap in GAP_FACTORS:
                configs.append({"method": method, "params": (("gap_factor", gap),)})

    elif round_number == 3:
        for method in orientations:
            for shift in SHIFTS:
                configs.append({"method": method, "params": (("shift", shift),)})

    elif round_number == 4:
        for method in orientations:
            for partition in partitions:
                for gap in (0.0, 4.0):
                    for shift in (0.0, 0.5):
                        params = (("gap_factor", gap), ("shift", shift))
                        configs.append(
                            {"method": method, "params": params, "partition": partition}
                        )

    return configs


LAST_ROUND = 4


def build_jobs(
    source: Path,
    catalog: dict[str, dict[str, str]],
    *,
    max_round: int = LAST_ROUND,
    only: str = "",
) -> list[Job]:
    """
    Every job for every instance in `source`, in run order.
    """
    instances = []
    for instance in InstanceDatabase(str(source)):
        uid = instance.instance_uid

        if only and only not in uid:
            continue

        # the catalog holds the sizes, so we do not rasterize anything here
        row = catalog.get(uid)
        if row is None:
            cells = 0
        else:
            cells = int(row["cells"])

        instances.append((uid, cells, instance.number_of_cutters))

    # biggest first inside every round
    instances.sort(key=lambda item: -item[1])

    jobs: list[Job] = []
    for round_number in range(min(max_round, LAST_ROUND) + 1):
        for uid, cells, k in instances:
            for config in configs_for(k, cells, round_number):
                job = Job(
                    instance=uid,
                    source=str(source),
                    cells=cells,
                    k=k,
                    round=round_number,
                    time_limit=time_limit_for(cells),
                    **config,
                )
                jobs.append(job)

    return jobs
