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
"""

import csv
import io
import json
import os
import statistics
import sys
import time
from pathlib import Path
from typing import Any

from .jobs import BASELINE_ROUNDS

__all__ = ["leaderboard_rows", "summary", "write_leaderboard"]


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
            "lower_bound": bound,
            "baseline": base,
            "best": best,
            "best_config": best_label.get(uid),
            "gap_pct": _pct(best, bound) if bound else None,
            "vs_baseline_pct": _pct(best, base) if base else None,
            "score_gain": round((base / best) ** 2, 4) if base else None,
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

    return {
        "feasible": len(rows),
        "instances": total_instances,
        "baseline_sum": baseline_sum,
        "best_sum": best_sum,
        "sum_change_pct": _pct(best_sum, baseline_sum) if baseline_sum else None,
        "median_gap_pct": round(statistics.median(gaps), 1) if gaps else None,
        "score_gain_total": round(sum(r["score_gain"] for r in with_baseline), 2),
        "biggest_wins": [(r["instance"], r["vs_baseline_pct"]) for r in wins],
        "worst_gaps": [(r["instance"], r["gap_pct"]) for r in worst],
    }


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
            for key in ("gap_pct", "vs_baseline_pct", "score_gain"):
                parsed[key] = float(row[key]) if row[key] else None
            rows.append(parsed)

    with (data / "catalog.csv").open() as file:
        challenge = [r for r in csv.DictReader(file) if r["source"] == "instances"]

    print(json.dumps(summary(rows, len(challenge)), indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
