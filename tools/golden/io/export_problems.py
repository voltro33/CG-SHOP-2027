"""
Exports what `Problem.from_instance` makes of every challenge instance, so the
Rust crate `mow-io` can be checked against it.

Run from the repository root:

    .venv/Scripts/python.exe tools/golden/io/export_problems.py

Writes `crates/adapters/io/tests/fixtures/problems.json`: one entry per instance with the
uid, k, cell count, both grid boxes, the cutter as runs, and a hash of the
region's runs (`run_hash` below, mirrored in the Rust test).
"""

import json
import sys
import time
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / "src"))

from solver.model import Problem  # noqa: E402

from cgshop2027_pyutils.io import read_instance  # noqa: E402

INSTANCES = ROOT / "data" / "instances" / "challenge_v0" / "instances"
OUTPUT = ROOT / "crates" / "adapters" / "io" / "tests" / "fixtures" / "problems.json"

FNV_OFFSET = 0xCBF29CE484222325
FNV_PRIME = 0x100000001B3
MASK = (1 << 64) - 1


def runs(cells) -> list[tuple[int, int, int]]:
    """
    The maximal horizontal runs `(y, x_min, x_max)`, row by row from the
    bottom, left to right.
    """
    padded = np.zeros((cells.height, cells.width + 2), dtype=np.int8)
    padded[:, 1:-1] = cells.mask

    step = np.diff(padded, axis=1)
    start_rows, start_cols = np.nonzero(step == 1)
    _, end_cols = np.nonzero(step == -1)

    x0, y0 = cells.origin
    return [
        (y0 + int(r), x0 + int(s), x0 + int(e) - 1)
        for r, s, e in zip(start_rows, start_cols, end_cols, strict=True)
    ]


def run_hash(region_runs: list[tuple[int, int, int]]) -> str:
    """
    FNV-1a over 64 bit words: each of `y, x_min, x_max` of every run, as a
    two's complement u64, xored in and multiplied by the FNV prime.
    """
    h = FNV_OFFSET
    for run in region_runs:
        for value in run:
            h = ((h ^ (value & MASK)) * FNV_PRIME) & MASK
    return f"{h:016x}"


def export(path: Path) -> dict:
    problem = Problem.from_instance(read_instance(path))
    region_runs = runs(problem.region)

    return {
        "uid": problem.uid,
        "k": problem.k,
        "num_cells": problem.num_cells,
        "region_frame": list(problem.region.bounds),
        "region_runs": len(region_runs),
        "region_hash": run_hash(region_runs),
        "cutter_frame": list(problem.cutter.bounds),
        "cutter_area": len(problem.cutter),
        "cutter_runs": [list(run) for run in runs(problem.cutter)],
    }


def main() -> None:
    started = time.perf_counter()
    paths = sorted(INSTANCES.glob("*.instance.json"))

    entries = []
    for index, path in enumerate(paths, 1):
        entries.append(export(path))
        print(f"{index:3}/{len(paths)} {entries[-1]['uid']}", flush=True)

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(json.dumps(entries, separators=(",", ":")) + "\n")
    print(f"wrote {len(entries)} entries in {time.perf_counter() - started:.1f}s")


if __name__ == "__main__":
    main()
