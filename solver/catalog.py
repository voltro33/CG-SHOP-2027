"""
Instance catalog: one row of plain features per instance, so we can see at a
glance what kinds of shapes, cutters and cutter counts the solver must handle.

    python -m solver.catalog                                   # examples + challenge
    python -m solver.catalog --instances data/instances/challenge_v0/instances

The `cutter_kind` column answers "are cutters uniform across instances?":
it classifies each cutter as a square, a rectangle, or some other polyomino.
"""

import argparse
import csv
import sys
from concurrent.futures import ProcessPoolExecutor
from dataclasses import asdict, dataclass
from pathlib import Path

from cgshop2027_pyutils.instance_database import InstanceDatabase
from cgshop2027_pyutils.schemas import CGSHOP2027Instance

from .bounds import lower_bound
from .model import Problem

__all__ = ["InstanceFeatures", "cutter_kind", "features"]


DEFAULT_SOURCES = (
    Path("example_instances.zip"),
    Path("data/instances/challenge_v0/instances"),
)


@dataclass(frozen=True)
class InstanceFeatures:
    source: str
    instance: str

    # the region
    cells: int
    width: int
    height: int
    aspect: float
    fill: float  # cells divided by bounding box area, low means sparse or ragged
    holes: int

    # the cutters
    k: int
    cutter_cells: int
    cutter_width: int
    cutter_height: int
    cutter_kind: str
    step_gain: int

    lower_bound: int


def cutter_kind(problem: Problem) -> str:
    """
    `square`, `rectangle` (a full box that is not square) or `other`.
    """
    x0, y0, x1, y1 = problem.cutter.bounds
    width = x1 - x0 + 1
    height = y1 - y0 + 1

    # a cutter that fills its bounding box exactly is a rectangle
    is_full_box = len(problem.cutter) == width * height
    if not is_full_box:
        return "other"

    if width == height:
        return "square"

    return "rectangle"


def features(instance: CGSHOP2027Instance, source: str = "") -> InstanceFeatures:
    """
    Measures one instance. Everything here is cheap except rasterizing the
    region, which `Problem.from_instance` does once.
    """
    problem = Problem.from_instance(instance)

    x0, y0, x1, y1 = problem.region.bounds
    width = x1 - x0 + 1
    height = y1 - y0 + 1

    cx0, cy0, cx1, cy1 = problem.cutter.bounds

    return InstanceFeatures(
        source=source,
        instance=instance.instance_uid,
        cells=problem.num_cells,
        width=width,
        height=height,
        aspect=round(max(width, height) / min(width, height), 2),
        fill=round(problem.num_cells / (width * height), 3),
        holes=len(instance.region_to_cover.inner_boundaries),
        k=problem.k,
        cutter_cells=len(problem.cutter),
        cutter_width=cx1 - cx0 + 1,
        cutter_height=cy1 - cy0 + 1,
        cutter_kind=cutter_kind(problem),
        step_gain=problem.step_gain(),
        lower_bound=lower_bound(problem),
    )


def _features_of(job: tuple[str, str]) -> InstanceFeatures:
    """
    Worker entry point. Takes plain strings, because those are cheap to send
    to another process; the worker loads the instance itself.
    """
    source, uid = job
    instance = InstanceDatabase(source)[uid]

    return features(instance, Path(source).name)


def catalog(sources: list[Path], workers: int) -> list[InstanceFeatures]:
    # one job per instance so big and small instances spread across workers
    jobs: list[tuple[str, str]] = []
    for source in sources:
        # the challenge folder is optional, skip what is not there
        if not source.exists():
            continue

        for instance in InstanceDatabase(str(source)):
            jobs.append((str(source), instance.instance_uid))

    with ProcessPoolExecutor(max_workers=workers) as pool:
        return list(pool.map(_features_of, jobs))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--instances", type=Path, action="append")
    parser.add_argument("--out", type=Path, default=Path("data/catalog.csv"))
    parser.add_argument("--workers", type=int, default=8)
    args = parser.parse_args(argv)

    sources = args.instances or list(DEFAULT_SOURCES)
    rows = catalog(sources, args.workers)

    args.out.parent.mkdir(parents=True, exist_ok=True)
    with args.out.open("w", newline="") as file:
        writer = csv.DictWriter(file, fieldnames=list(asdict(rows[0])))
        writer.writeheader()
        for row in rows:
            writer.writerow(asdict(row))

    print(f"{len(rows)} instances -> {args.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
