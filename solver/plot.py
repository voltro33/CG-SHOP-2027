"""
Pictures of stored solutions, drawn with the official visualizer.

    python -m solver.plot fjs3649_17k_1x7                  # the best of one instance
    python -m solver.plot fjs3649_17k_1x7 iso8005_26k_1s3  # several
    python -m solver.plot fjs3649_17k_1x7 --runs           # list its stored runs
    python -m solver.plot fjs3649_17k_1x7 --run groups-v   # the runs whose name has that text
    python -m solver.plot --bench                          # the bests of the benchmark set
    python -m solver.plot --all --max-cells 200000         # every best up to that size
    python -m solver.plot path/to/some.solution.json       # any solution file

Pictures go to `plots/`: a best to `plots/<instance>.png`, a run to
`plots/<instance>/<run>.png`. `--bare` drops axes and titles for slides,
`--gif` also writes an animation of the cutters at work.

Each picture shades the swept area, marks uncovered cells in red and draws
one tour per cutter. Instances above a million cells take a while to draw.
"""

import argparse
import csv
import sys
from dataclasses import dataclass
from pathlib import Path

import matplotlib

matplotlib.use("Agg")

import matplotlib.pyplot as plt

from cgshop2027_pyutils.instance_database import InstanceDatabase
from cgshop2027_pyutils.io import read_solution
from cgshop2027_pyutils.visualize import (
    create_solution_animation,
    create_solution_plot,
)

from .bench import read_bench_list
from .mine import DEFAULT_INSTANCES

__all__ = ["Target", "find_targets"]

DATA = Path("data")
BEST = DATA / "solutions" / "best"
CANDIDATES = DATA / "solutions" / "candidates"
CATALOG = DATA / "catalog.csv"
DEFAULT_OUT = Path("plots")

SUFFIX = ".solution.json"


@dataclass(frozen=True)
class Target:
    """
    One solution file to draw, and where its picture goes.
    """

    solution: Path
    picture: Path


def find_targets(
    names: list[str],
    *,
    run: str | None,
    out: Path,
    best: Path = BEST,
    candidates: Path = CANDIDATES,
) -> list[Target]:
    """
    Solution files for instance names or file paths. With `run`, the
    stored runs of each instance whose file name holds that text, instead
    of its best.
    """
    targets = []
    for name in names:
        path = Path(name)
        if name.endswith(SUFFIX) and path.exists():
            targets.append(Target(path, out / (path.name.removesuffix(SUFFIX) + ".png")))
            continue

        if run is None:
            targets.append(Target(best / f"{name}{SUFFIX}", out / f"{name}.png"))
            continue

        for found in _runs_of(name, candidates):
            run_name = _run_name(name, found)
            if run in run_name:
                targets.append(Target(found, out / name / f"{run_name}.png"))

    return targets


def _runs_of(uid: str, candidates: Path) -> list[Path]:
    folder = candidates / uid

    return sorted(folder.glob(f"*{SUFFIX}"))


def _run_name(uid: str, path: Path) -> str:
    # candidate files are named after the job, which starts with the instance
    stem = path.name.removesuffix(SUFFIX)

    return stem.removeprefix(uid + "_")


def _all_instances(max_cells: int | None) -> list[str]:
    with CATALOG.open() as file:
        cells = {row["instance"]: int(row["cells"]) for row in csv.DictReader(file)}

    uids = []
    for path in sorted(BEST.glob(f"*{SUFFIX}")):
        uid = path.name.removesuffix(SUFFIX)
        if max_cells is not None and cells.get(uid, 0) > max_cells:
            continue

        uids.append(uid)

    return uids


def _list_runs(uids: list[str]) -> None:
    for uid in uids:
        runs = _runs_of(uid, CANDIDATES)
        print(f"{uid}: {len(runs)} stored runs")

        for path in runs:
            objective = read_solution(path).objective
            print(f"  {objective:>8}  {_run_name(uid, path)}")


def _draw(target: Target, database: InstanceDatabase, *, bare: bool, dpi: int, gif: bool) -> None:
    solution = read_solution(target.solution)
    instance = database[solution.instance_uid]

    target.picture.parent.mkdir(parents=True, exist_ok=True)

    figure = create_solution_plot(instance, solution, bare=bare)
    figure.savefig(target.picture, dpi=dpi)
    plt.close(figure)
    print(f"saved {target.picture}  length {solution.objective}", flush=True)

    if gif:
        animation = create_solution_animation(instance, solution, bare=bare)
        moving = target.picture.with_suffix(".gif")
        animation.save(moving, writer="pillow")
        plt.close(animation._fig)
        print(f"saved {moving}", flush=True)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("names", nargs="*", help="instance names or solution files")
    parser.add_argument("--all", action="store_true", help="every stored best")
    parser.add_argument("--bench", action="store_true", help="the bests of the benchmark set")
    parser.add_argument("--max-cells", type=int, default=None, help="skip bigger instances")
    parser.add_argument("--run", default=None, help="draw the stored runs whose name has this text")
    parser.add_argument("--runs", action="store_true", help="list the stored runs, draw nothing")
    parser.add_argument("--bare", action="store_true", help="no axes or titles")
    parser.add_argument("--gif", action="store_true", help="also write an animation")
    parser.add_argument("--dpi", type=int, default=200)
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT)
    parser.add_argument("--instances", type=Path, default=DEFAULT_INSTANCES)
    args = parser.parse_args(argv)

    names = list(args.names)
    if args.all:
        names += _all_instances(args.max_cells)
    if args.bench:
        names += read_bench_list()

    if not names:
        parser.error("name an instance or a solution file, or pass --all or --bench")

    if args.runs:
        _list_runs(names)
        return 0

    targets = find_targets(names, run=args.run, out=args.out)
    if not targets:
        print("nothing to draw, try --runs to see what is stored")
        return 1

    database = InstanceDatabase(str(args.instances))

    failures = 0
    for target in targets:
        if not target.solution.exists():
            print(f"missing {target.solution}")
            failures += 1
            continue

        # one broken picture should not stop a long batch
        try:
            _draw(target, database, bare=args.bare, dpi=args.dpi, gif=args.gif)
        except Exception as error:
            print(f"failed {target.solution}: {type(error).__name__}: {error}")
            failures += 1

    print(f"{len(targets) - failures} of {len(targets)} drawn into {args.out}")

    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
