"""
What the mining runs taught us, from the job log, so the next run spends its
hours where bests come from.

    python -m solver.mine_report
    python -m solver.mine_report --json data/mine_report.json

Points are pessimistic score, `(lower bound / length) ** 2` per instance, the
same unit as the report. Four questions:

- rounds: CPU hours per round, how often a job beat everything logged before
  it for its instance, and the points those improvements bought per CPU hour
- configs: how many instances each config wins, and the points at stake, the
  score lost if that config had never run and the next best config stood
- strata: which configs win per cutter family and size band
- time caps: the share of jobs that ran into their wall clock cap, where a
  longer cap might still help, and where jobs finish early

A config is a label plus its seed, since extra seeds are their own jobs.
"""

import argparse
import csv
import json
import statistics
import sys
from collections import defaultdict
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

from .jobs import time_limit_for
from .report import family, pessimistic_score

__all__ = [
    "ConfigStats",
    "RoundStats",
    "config_of",
    "config_stats",
    "round_stats",
    "strata_winners",
    "time_cap_shares",
]

DATA = Path("data")

# a job counts as capped when it ran this close to its time limit
CAP_SHARE = 0.95

SIZE_BANDS = ((0, 50_000, "<50k"), (50_000, 400_000, "50k-400k"), (400_000, 2_000_000, "400k-2M"), (2_000_000, 10**12, ">2M"))

TOP_PER_STRATUM = 3


@dataclass(frozen=True)
class RoundStats:
    round: int
    jobs: int
    cpu_hours: float
    improvements: int
    points: float

    @property
    def points_per_cpu_hour(self) -> float:
        return self.points / self.cpu_hours if self.cpu_hours else 0.0


@dataclass(frozen=True)
class ConfigStats:
    config: str
    jobs: int
    cpu_hours: float
    wins: int
    sole_wins: int
    points_at_stake: float


def config_of(record: dict[str, Any]) -> str:
    """
    `label|sN` from a job id `instance|label|sN|vM` or an older `instance|label|sN`.
    """
    parts = record["job_id"].split("|")

    return f"{parts[1]}|{parts[2]}"


def round_stats(records: list[dict[str, Any]], bounds: dict[str, int]) -> list[RoundStats]:
    """
    Walks the log in the order jobs finished. A job improves when it is
    shorter than every earlier result for its instance; the first result of
    an instance sets the start and buys nothing.
    """
    running_best: dict[str, int] = {}
    per_round: dict[int, dict[str, float]] = defaultdict(lambda: {"jobs": 0, "seconds": 0.0, "improvements": 0, "points": 0.0})

    for record in records:
        totals = per_round[record["round"]]
        totals["jobs"] += 1
        totals["seconds"] += record["seconds"]

        objective = record["objective"]
        if objective is None:
            continue

        uid = record["instance"]
        before = running_best.get(uid)

        if before is not None and objective < before:
            bound = bounds[uid]
            gained = pessimistic_score(bound, objective) - pessimistic_score(bound, before)
            totals["improvements"] += 1
            totals["points"] += gained

        if before is None or objective < before:
            running_best[uid] = objective

    result = []
    for number in sorted(per_round):
        totals = per_round[number]
        stats = RoundStats(
            round=number,
            jobs=int(totals["jobs"]),
            cpu_hours=totals["seconds"] / 3600,
            improvements=int(totals["improvements"]),
            points=totals["points"],
        )
        result.append(stats)

    return result


def config_stats(records: list[dict[str, Any]], bounds: dict[str, int]) -> list[ConfigStats]:
    """
    Per config: wins, wins nobody else ties, and the points lost on the
    instances it wins if it had never run. Most points at stake first.
    """
    # the best length per instance and config
    best: dict[str, dict[str, int]] = defaultdict(dict)
    jobs: dict[str, int] = defaultdict(int)
    seconds: dict[str, float] = defaultdict(float)

    for record in records:
        config = config_of(record)
        jobs[config] += 1
        seconds[config] += record["seconds"]

        objective = record["objective"]
        if objective is None:
            continue

        mine = best[record["instance"]]
        if config not in mine or objective < mine[config]:
            mine[config] = objective

    wins: dict[str, int] = defaultdict(int)
    sole_wins: dict[str, int] = defaultdict(int)
    at_stake: dict[str, float] = defaultdict(float)

    for uid, by_config in best.items():
        ordered = sorted(by_config.values())
        shortest = ordered[0]
        winners = [config for config, value in by_config.items() if value == shortest]

        for config in winners:
            wins[config] += 1

        if len(winners) > 1 or len(ordered) < 2:
            continue

        # only a sole winner costs anything when it is taken away
        sole = winners[0]
        sole_wins[sole] += 1
        bound = bounds[uid]
        at_stake[sole] += pessimistic_score(bound, shortest) - pessimistic_score(bound, ordered[1])

    result = []
    for config in jobs:
        stats = ConfigStats(
            config=config,
            jobs=jobs[config],
            cpu_hours=seconds[config] / 3600,
            wins=wins[config],
            sole_wins=sole_wins[config],
            points_at_stake=at_stake[config],
        )
        result.append(stats)

    result.sort(key=lambda stats: (-stats.points_at_stake, -stats.wins))

    return result


def size_band(cells: int) -> str:
    for low, high, name in SIZE_BANDS:
        if low <= cells < high:
            return name

    return SIZE_BANDS[-1][2]


def strata_winners(
    records: list[dict[str, Any]], cells: dict[str, int]
) -> dict[tuple[str, str], list[tuple[str, int]]]:
    """
    Per (family, size band), the configs that win most instances there.
    """
    best: dict[str, dict[str, int]] = defaultdict(dict)
    for record in records:
        if record["objective"] is None:
            continue

        mine = best[record["instance"]]
        config = config_of(record)
        if config not in mine or record["objective"] < mine[config]:
            mine[config] = record["objective"]

    counts: dict[tuple[str, str], dict[str, int]] = defaultdict(lambda: defaultdict(int))
    for uid, by_config in best.items():
        shortest = min(by_config.values())
        stratum = (family(uid), size_band(cells[uid]))

        for config, value in by_config.items():
            if value == shortest:
                counts[stratum][config] += 1

    result = {}
    for stratum in sorted(counts):
        ranked = sorted(counts[stratum].items(), key=lambda item: -item[1])
        result[stratum] = ranked[:TOP_PER_STRATUM]

    return result


def time_cap_shares(records: list[dict[str, Any]], cells: dict[str, int]) -> dict[str, dict[str, float]]:
    """
    Per size band: the share of jobs that ran into their cap, and the median
    run time as a share of the cap.
    """
    capped: dict[str, list[bool]] = defaultdict(list)
    used: dict[str, list[float]] = defaultdict(list)

    for record in records:
        size = cells[record["instance"]]
        limit = time_limit_for(size)
        band = size_band(size)

        capped[band].append(record["seconds"] >= CAP_SHARE * limit)
        used[band].append(record["seconds"] / limit)

    result = {}
    for _, _, band in SIZE_BANDS:
        if band not in capped:
            continue

        result[band] = {
            "jobs": len(capped[band]),
            "capped_share": sum(capped[band]) / len(capped[band]),
            "median_used_share": statistics.median(used[band]),
        }

    return result


def read_records(path: Path) -> list[dict[str, Any]]:
    records = []
    for line in path.read_text().splitlines():
        try:
            records.append(json.loads(line))
        except json.JSONDecodeError:
            # a torn last line from a killed run
            continue

    return records


def _read_catalog(path: Path) -> list[dict[str, str]]:
    with path.open() as file:
        return [row for row in csv.DictReader(file) if row["source"] == "instances"]


def _print_rounds(rounds: list[RoundStats]) -> None:
    print("rounds, in the order jobs finished")
    print(f"  {'round':>5} {'jobs':>6} {'cpu h':>7} {'better':>7} {'points':>7} {'pts/cpu h':>10}")

    for stats in rounds:
        print(
            f"  {stats.round:>5} {stats.jobs:>6} {stats.cpu_hours:>7.1f} {stats.improvements:>7} "
            f"{stats.points:>7.2f} {stats.points_per_cpu_hour:>10.3f}"
        )


def _print_configs(configs: list[ConfigStats]) -> None:
    never = [stats for stats in configs if stats.wins == 0]
    never_hours = sum(stats.cpu_hours for stats in never)
    total_hours = sum(stats.cpu_hours for stats in configs)

    print()
    print(f"configs: {len(configs)}, of which {len(never)} never won, using {never_hours:.1f} of {total_hours:.1f} cpu hours")
    print(f"  {'config':58} {'jobs':>5} {'cpu h':>6} {'wins':>5} {'sole':>5} {'at stake':>9}")

    for stats in configs:
        print(
            f"  {stats.config:58} {stats.jobs:>5} {stats.cpu_hours:>6.1f} {stats.wins:>5} "
            f"{stats.sole_wins:>5} {stats.points_at_stake:>9.3f}"
        )


def _print_strata(strata: dict[tuple[str, str], list[tuple[str, int]]]) -> None:
    print()
    print("most wins per family and size")

    for (name, band), ranked in strata.items():
        listed = ", ".join(f"{config} {count}" for config, count in ranked)
        print(f"  {name:15} {band:9} {listed}")


def _print_time_caps(caps: dict[str, dict[str, float]]) -> None:
    print()
    print("time caps per size band")

    for band, values in caps.items():
        print(
            f"  {band:9} {values['jobs']:>6} jobs, {values['capped_share']:6.1%} capped, "
            f"median run uses {values['median_used_share']:6.1%} of its cap"
        )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("--log", type=Path, default=DATA / "jobs.jsonl")
    parser.add_argument("--catalog", type=Path, default=DATA / "catalog.csv")
    parser.add_argument("--json", type=Path, default=None, help="also write everything here")
    args = parser.parse_args(argv)

    catalog = _read_catalog(args.catalog)
    bounds = {row["instance"]: int(row["lower_bound"]) for row in catalog}
    cells = {row["instance"]: int(row["cells"]) for row in catalog}

    # only challenge instances count, the log may also hold example runs
    records = [record for record in read_records(args.log) if record["instance"] in bounds]

    rounds = round_stats(records, bounds)
    configs = config_stats(records, bounds)
    strata = strata_winners(records, cells)
    caps = time_cap_shares(records, cells)

    _print_rounds(rounds)
    _print_configs(configs)
    _print_strata(strata)
    _print_time_caps(caps)

    if args.json:
        everything = {
            "rounds": [asdict(stats) for stats in rounds],
            "configs": [asdict(stats) for stats in configs],
            "strata": {f"{name}|{band}": ranked for (name, band), ranked in strata.items()},
            "time_caps": caps,
        }
        args.json.write_text(json.dumps(everything, indent=2))

    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
