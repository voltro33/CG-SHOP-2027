"""
Pictures of stored bests, written to `plots/best/`.

    python -m solver.show fjs3649_17k_1x7                  # official plot
    python -m solver.show fjs3649_17k_1x7 --dead           # dead steps in red
    python -m solver.show iso8005_26k_1s3 --dead --crop 0,200,0,200
    python -m solver.show --bench                          # every benchmark instance

The official plot (`cgshop2027_pyutils.visualize`) shows the swept area, any
gap and every tour. The dead step view draws each unit step of the tours in
the colour of its tour, and in red when the step sweeps no region cell that
was not swept before: that is where the length is lost.
"""

import argparse
from pathlib import Path

import matplotlib

matplotlib.use("Agg")

import matplotlib.pyplot as plt
import numpy as np

from cgshop2027_pyutils.instance_database import InstanceDatabase
from cgshop2027_pyutils.io import read_solution
from cgshop2027_pyutils.visualize import create_solution_plot

from .bench import read_bench_list
from .diagnose import fresh_offsets
from .mine import DEFAULT_INSTANCES
from .model import Problem
from .tours import densify

__all__ = ["draw_dead_steps"]

BEST = Path("data/solutions/best")
OUT = Path("plots/best")

TOUR_COLORS = ("tab:blue", "tab:green", "tab:purple")


def draw_dead_steps(problem: Problem, solution, crop: list[int] | None = None):
    """
    A figure of the region with every unit step of the tours, dead steps
    in red. `crop` is `[x_min, x_max, y_min, y_max]`.
    """
    mask = problem.region.mask
    x0, y0 = problem.region.origin
    offsets = problem.offsets
    fresh = fresh_offsets(offsets)

    fig, ax = plt.subplots(figsize=(12, 12))
    extent = (x0 - 0.5, x0 + mask.shape[1] - 0.5, y0 - 0.5, y0 + mask.shape[0] - 0.5)
    ax.imshow(mask, origin="lower", extent=extent, cmap="Greys", alpha=0.35)

    # swept cells as a grid in region coordinates, so a step is a few lookups
    swept = np.zeros_like(mask, dtype=bool)
    dead_total = 0
    steps_total = 0

    for index, tour in enumerate(solution.tours):
        positions = densify(list(zip(tour.x, tour.y, strict=True)), closed=True)
        color = TOUR_COLORS[index % len(TOUR_COLORS)]
        _sweep(swept, mask, positions[0] + offsets, x0, y0)

        live = []
        dead = []
        for position, after in zip(positions[:-1], positions[1:], strict=True):
            step = (int(after[0] - position[0]), int(after[1] - position[1]))
            new = _sweep(swept, mask, after + fresh[step], x0, y0)

            segment = [(position[0], after[0]), (position[1], after[1])]
            if new:
                live.append(segment)
            else:
                dead.append(segment)

        _draw_steps(ax, live, color, 0.8)
        _draw_steps(ax, dead, "red", 1.6)
        dead_total += len(dead)
        steps_total += len(positions) - 1

    if crop:
        ax.set_xlim(crop[0], crop[1])
        ax.set_ylim(crop[2], crop[3])

    share = 100 * dead_total / max(1, steps_total)
    ax.set_aspect("equal")
    ax.set_title(
        f"{problem.uid}: objective {solution.objective}, red = dead steps ({share:.1f}% of all steps)"
    )

    return fig


def _sweep(swept, mask, cells, x0, y0) -> int:
    """
    Marks region cells as swept, returns how many were new.
    """
    columns = cells[:, 0] - x0
    rows = cells[:, 1] - y0
    inside = (columns >= 0) & (columns < mask.shape[1]) & (rows >= 0) & (rows < mask.shape[0])
    columns = columns[inside]
    rows = rows[inside]

    is_new = mask[rows, columns] & ~swept[rows, columns]
    swept[rows, columns] = True

    return int(is_new.sum())


def _draw_steps(ax, segments, color: str, width: float) -> None:
    # one plot call per colour, nan breaks the line between steps
    if not segments:
        return

    xs = []
    ys = []
    for (xa, xb), (ya, yb) in segments:
        xs.extend((xa, xb, np.nan))
        ys.extend((ya, yb, np.nan))

    ax.plot(xs, ys, color=color, linewidth=width)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("instances", nargs="*")
    parser.add_argument("--bench", action="store_true", help="every benchmark instance")
    parser.add_argument("--dead", action="store_true", help="dead step view instead of the official plot")
    parser.add_argument("--crop", default=None, help="x_min,x_max,y_min,y_max")
    args = parser.parse_args(argv)

    uids = list(args.instances)
    if args.bench:
        uids.extend(read_bench_list())

    crop = [int(value) for value in args.crop.split(",")] if args.crop else None

    database = InstanceDatabase(str(DEFAULT_INSTANCES))
    OUT.mkdir(parents=True, exist_ok=True)

    for uid in uids:
        instance = database[uid]
        solution = read_solution(BEST / f"{uid}.solution.json")

        if args.dead:
            figure = draw_dead_steps(Problem.from_instance(instance), solution, crop)
            path = OUT / f"{uid}.dead.png"
        else:
            figure = create_solution_plot(instance, solution)
            path = OUT / f"{uid}.png"

        figure.savefig(path, dpi=90, bbox_inches="tight")
        plt.close(figure)
        print("saved", path)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
