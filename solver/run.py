"""
Batch runner: solve instances with every method, verify, keep the best.

    python -m solver.run                                  # all example instances
    python -m solver.run --only pla7k --methods lanes-h,cover-tsp
    python -m solver.run --zip submission.zip            # also pack the best solutions

The best verified solution per instance is kept in `<out>/best/` across runs and
only replaced by a strictly better one; `<out>/summary.csv` compares it with
the lower bound and the provided baseline.
"""

import argparse
import csv
import fnmatch
import logging
import sys
from pathlib import Path

from cgshop2027_pyutils.instance_database import InstanceDatabase
from cgshop2027_pyutils.io import read_solution
from cgshop2027_pyutils.schemas import CGSHOP2027Instance, CGSHOP2027Solution
from cgshop2027_pyutils.verify import SolutionValidator
from cgshop2027_pyutils.zip import ZipSolutionIterator, ZipWriter

from .bounds import lower_bound
from .construct import CONSTRUCTORS
from .model import Problem
from .pipeline import Attempt, solve

log = logging.getLogger("solver")


def main(argv: list[str] | None = None) -> int:
    args = _parse(argv)
    logging.basicConfig(level=logging.INFO, format="%(message)s", stream=sys.stdout)
    methods = args.methods.split(",")
    unknown = [m for m in methods if m not in CONSTRUCTORS]
    if unknown:
        log.error("Unknown method(s) %s; choose from %s", unknown, sorted(CONSTRUCTORS))
        return 2

    best_dir = args.out / "best"
    best_dir.mkdir(parents=True, exist_ok=True)
    baseline = _load_baseline(args.baseline)
    rows = []
    for instance in InstanceDatabase(str(args.instances)):
        if args.only and not fnmatch.fnmatch(instance.instance_uid, f"*{args.only}*"):
            continue
        rows.append(
            _solve_instance(instance, methods, args.time_limit, best_dir, baseline)
        )

    _write_summary(args.out / "summary.csv", rows)
    if args.zip:
        _write_zip(args.zip, best_dir)
    return 0


def _solve_instance(
    instance: CGSHOP2027Instance,
    methods: list[str],
    time_limit: float,
    best_dir: Path,
    baseline: dict[str, int],
) -> dict:
    problem = Problem.from_instance(instance)
    validator = SolutionValidator(instance)
    attempts = [solve(problem, validator, m, time_limit) for m in methods]
    for a in attempts:
        status = a.objective if a.feasible else f"FAILED ({a.errors[0][:80]})"
        log.info(
            "  %-28s %-10s %8s  %5.1fs",
            instance.instance_uid,
            a.method,
            status,
            a.seconds,
        )

    path = best_dir / f"{instance.instance_uid}.solution.json"
    kept = _keep_best(path, validator, [a for a in attempts if a.feasible])
    bound = lower_bound(problem)
    base = baseline.get(instance.instance_uid)
    return {
        "instance": instance.instance_uid,
        "k": problem.k,
        "cells": problem.num_cells,
        "lower_bound": bound,
        "baseline": base,
        "best": kept.objective if kept else None,
        "method": kept.meta.get("method") if kept else None,
        "vs_baseline_pct": _pct(kept.objective, base) if kept and base else None,
        "gap_to_bound_pct": _pct(kept.objective, bound) if kept and bound else None,
    }


def _keep_best(
    path: Path, validator: SolutionValidator, attempts: list[Attempt]
) -> CGSHOP2027Solution | None:
    """
    The better of the stored best and this run's best; writes it if it changed.
    A stored file that no longer verifies is ignored.
    """
    stored = None
    if path.exists():
        candidate = read_solution(path)
        if not validator.check_for_errors(candidate):
            stored = candidate
    fresh = min((a.solution for a in attempts), key=lambda s: s.objective, default=None)
    if fresh is not None and (stored is None or fresh.objective < stored.objective):
        path.write_text(fresh.model_dump_json())
        return fresh
    return stored


def _load_baseline(path: Path | None) -> dict[str, int]:
    if path is None or not path.exists():
        return {}
    return {s.instance_uid: s.objective for s in ZipSolutionIterator(str(path))}


def _pct(value: int, reference: int) -> float:
    return round(100.0 * (value - reference) / reference, 1)


def _write_summary(path: Path, rows: list[dict]) -> None:
    if not rows:
        log.warning("No instances matched.")
        return
    with path.open("w", newline="") as f:
        writer = csv.DictWriter(f, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)
    better = sum(
        1 for r in rows if r["vs_baseline_pct"] is not None and r["vs_baseline_pct"] < 0
    )
    log.info(
        "\n%d instances, %d better than baseline. Summary: %s", len(rows), better, path
    )


def _write_zip(path: Path, best_dir: Path) -> None:
    if path.exists():
        path.unlink()
    with ZipWriter(path) as writer:
        for file in sorted(best_dir.glob("*.solution.json")):
            writer.add_solution(read_solution(file))
    log.info("Wrote %s", path)


def _parse(argv: list[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("--instances", type=Path, default=Path("example_instances.zip"))
    parser.add_argument("--baseline", type=Path, default=Path("example_solutions.zip"))
    parser.add_argument("--out", type=Path, default=Path("results"))
    parser.add_argument("--methods", default=",".join(CONSTRUCTORS))
    parser.add_argument("--only", default="", help="substring filter on instance names")
    parser.add_argument(
        "--time-limit", type=float, default=20.0, help="seconds per method per instance"
    )
    parser.add_argument(
        "--zip", type=Path, help="also write the best solutions to this archive"
    )
    return parser.parse_args(argv)


if __name__ == "__main__":
    sys.exit(main())
