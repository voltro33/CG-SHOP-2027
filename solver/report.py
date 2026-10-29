"""
Leaderboard and summary numbers for the morning report.

    python -m solver.report                    # print the summary from data/

`leaderboard.csv` columns:

- `baseline`: best objective from the Phase 4 baseline rounds (see `jobs.py`)
- `best`: best verified objective so far, `best_config` the config that found it
- `gap_pct`: `(best - lower_bound) / lower_bound` in percent
- `vs_baseline_pct`: how much shorter than the baseline, in percent
- `score_gain`: `(baseline / best) ** 2`, the factor by which the contest
  score on this instance grew since the baseline, if nobody else moved
- `family`: cutter family from the instance name suffix, `s` square,
  `l` diamond, `d` clipped square, `p` plus, `x` X shape
- `pessimistic_score`: `(lower_bound / best) ** 2`, the contest score on this
  instance if some team reached the bound; the real score can only be higher
"""

import csv
import io
import json
import os
import re
import statistics
import sys
import time
from pathlib import Path
from typing import Any

from .jobs import BASELINE_ROUNDS

__all__ = [
    "family",
    "leaderboard_rows",
    "median_gap_by_family",
    "pessimistic_score",
    "summary",
    "write_leaderboard",
]

# names end in k, cutter letter and cutter size, sometimes with a variant letter
NAME_SUFFIX = re.compile(r"_(\d)([a-z])(\d+)[a-z]*$")

FAMILY_NAMES = {
    "s": "square",
    "l": "diamond",
    "d": "clipped square",
    "p": "plus",
    "x": "X",
}


def family(uid: str) -> str:
    """
    The cutter family of an instance, read from its name.
    """
    match = NAME_SUFFIX.search(uid)
    if match is None:
        return "unknown"

    return FAMILY_NAMES.get(match.group(2), "unknown")


def pessimistic_score(bound: int, objective: int) -> float | None:
    """
    `(bound / objective) ** 2`, the score if a team reached the lower bound.
    """
    if not bound or not objective:
        return None

    return (bound / objective) ** 2


def leaderboard_rows(
    catalog: dict[str, dict[str, str]],
    best_objective: dict[str, int],
    best_label: dict[str, str],
    records: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    # the best objective each instance reached in the baseline rounds
    baseline: dict[str, int] = {}
    for record in records:
        if record["objective"] is None or record["round"] not in BASELINE_ROUNDS:
            continue

        uid = record["instance"]
        if uid not in baseline or record["objective"] < baseline[uid]:
            baseline[uid] = record["objective"]

    rows = []
    for uid in sorted(best_objective):
        info = catalog.get(uid, {})
        best = best_objective[uid]
        bound = int(info.get("lower_bound", 0) or 0)
        base = baseline.get(uid)

        row = {
            "instance": uid,
            "k": info.get("k"),
            "cells": info.get("cells"),
            "cutter": info.get("cutter_kind"),
            "family": family(uid),
            "lower_bound": bound,
            "baseline": base,
            "best": best,
            "best_config": best_label.get(uid),
            "gap_pct": _pct(best, bound) if bound else None,
            "vs_baseline_pct": _pct(best, base) if base else None,
            "score_gain": round((base / best) ** 2, 4) if base else None,
            "pessimistic_score": _round(pessimistic_score(bound, best), 4),
        }
        rows.append(row)

    return rows


def write_leaderboard(path: Path, catalog, store, records) -> None:
    rows = leaderboard_rows(catalog, store.objective, store.label, records)
    if not rows:
        return

    buffer = io.StringIO()
    writer = csv.DictWriter(buffer, fieldnames=list(rows[0]), lineterminator="\n")
    writer.writeheader()
    writer.writerows(rows)

    # a spreadsheet holding the file open must not crash the miner
    temp = path.with_suffix(".csv.tmp")
    temp.write_text(buffer.getvalue())
    for attempt in range(10):
        try:
            os.replace(temp, path)
            return
        except PermissionError:
            time.sleep(0.2 * (attempt + 1))


def summary(rows: list[dict[str, Any]], total_instances: int) -> dict[str, Any]:
    """
    The headline numbers of section 7 of the plan.
    """
    with_baseline = [r for r in rows if r["baseline"]]

    baseline_sum = sum(r["baseline"] for r in with_baseline)
    best_sum = sum(r["best"] for r in with_baseline)

    gaps = [r["gap_pct"] for r in rows if r["gap_pct"] is not None]

    wins = sorted(with_baseline, key=lambda r: r["vs_baseline_pct"])[:5]
    worst = sorted(rows, key=lambda r: -(r["gap_pct"] or 0))[:5]

    # recomputed from bound and best so rounding in the csv does not add up
    pessimistic = 0.0
    for row in rows:
        score = pessimistic_score(row["lower_bound"], row["best"])
        if score is not None:
            pessimistic += score

    return {
        "feasible": len(rows),
        "instances": total_instances,
        "baseline_sum": baseline_sum,
        "best_sum": best_sum,
        "sum_change_pct": _pct(best_sum, baseline_sum) if baseline_sum else None,
        "median_gap_pct": round(statistics.median(gaps), 1) if gaps else None,
        "score_gain_total": round(sum(r["score_gain"] for r in with_baseline), 2),
        "pessimistic_score": round(pessimistic, 1),
        "median_gap_by_family": median_gap_by_family(rows),
        "biggest_wins": [(r["instance"], r["vs_baseline_pct"]) for r in wins],
        "worst_gaps": [(r["instance"], r["gap_pct"]) for r in worst],
    }


def median_gap_by_family(rows: list[dict[str, Any]]) -> dict[str, float]:
    """
    Median gap to the bound in percent per cutter family, widest gap first.
    """
    gaps: dict[str, list[float]] = {}
    for row in rows:
        if row["gap_pct"] is None:
            continue

        name = family(row["instance"])
        gaps.setdefault(name, []).append(row["gap_pct"])

    medians = {}
    for name, values in gaps.items():
        medians[name] = round(statistics.median(values), 1)

    return dict(sorted(medians.items(), key=lambda item: -item[1]))


def _round(value: float | None, digits: int) -> float | None:
    return None if value is None else round(value, digits)


def _pct(value: float, reference: float) -> float:
    return round(100.0 * (value - reference) / reference, 1)


def main(argv: list[str] | None = None) -> int:
    data = Path(argv[0]) if argv else Path("data")

    with (data / "leaderboard.csv").open() as file:
        rows = []
        for row in csv.DictReader(file):
            parsed: dict[str, Any] = dict(row)
            for key in ("baseline", "best", "lower_bound"):
                parsed[key] = int(row[key]) if row[key] else None
            for key in ("gap_pct", "vs_baseline_pct", "score_gain", "pessimistic_score"):
                parsed[key] = float(row[key]) if row.get(key) else None
            rows.append(parsed)

    with (data / "catalog.csv").open() as file:
        challenge = [r for r in csv.DictReader(file) if r["source"] == "instances"]

    print(json.dumps(summary(rows, len(challenge)), indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
