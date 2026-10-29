"""
Where the tour length goes. Works on any verified solution, whatever built it.

    python -m solver.diagnose                       # every benchmark best
    python -m solver.diagnose --only iso3131        # one instance

The tours are walked one after the other, unit step by unit step. A step
from `p` to `q` brings `gain_e` fresh footprint cells, the cells of the
footprint at `q` that the footprint at `p` did not hold. The area bound
assumes every step brings `gain` new region cells, the most any direction
can. Each step's unit of length is split by what its fresh cells are:

- productive: new region cells, `new / gain`
- overshoot: cells outside the region, `outside / gain`
- near resweep: region cells this same sweep took within the last few steps,
  the cutter running over its own trail, `near / gain`
- far resweep: region cells swept longer ago, lanes overlapping or crossing,
  `far / gain`
- direction: `(gain - gain_e) / gain`, a step direction that brings fewer
  fresh cells than the best one
- dead: a step with no new region cell at all counts fully here instead,
  links between lanes, returns and repair drives

The parts add up to the summed tour length exactly, and `length - new / gain`
is the length lost against the area bound. Shares are over all tours, since
the split balances them and the longest tour loses about the average share.

A lane is a straight run that sweeps at least one new cell. Waste that comes
with every lane is split out, since fewer, longer lanes remove it:

- overshoot at lane ends: overshoot in the first or last cutter span of a lane
- short dead stretches: runs of dead steps shorter than the cutter span,
  mostly turns and shoulders where one lane hands over to the next
- travel: dead stretches of at least four spans, real links

The lane reference is `(cells - k |C|) / (k * lane_gain)`, where `lane_gain`
is the number of rows a straight sweep covers. It is not a proven bound, but
next to the area bound it shows how much of a gap is the bound being loose.
"""

import argparse
import json
import sys
from dataclasses import asdict, dataclass
from pathlib import Path

import numpy as np

from cgshop2027_pyutils.instance_database import InstanceDatabase
from cgshop2027_pyutils.io import read_solution
from cgshop2027_pyutils.schemas import CGSHOP2027Solution

from .bounds import lower_bound
from .model import Problem
from .report import family
from .tours import densify

__all__ = ["Breakdown", "diagnose", "lane_gain", "lane_reference"]

DATA = Path("data")
DEFAULT_INSTANCES = DATA / "instances" / "challenge_v0" / "instances"

DIRECTIONS = ((1, 0), (-1, 0), (0, 1), (0, -1))

# the parts of a step, in the order they are reported
PARTS = ("productive", "overshoot", "near_resweep", "far_resweep", "direction", "dead")


@dataclass(frozen=True)
class Breakdown:
    instance: str
    family: str
    k: int
    cells: int
    gain: int
    lane_gain: int
    lower_bound: int
    lane_reference: int
    objective: int
    total_length: int
    new_cells: int
    start_cells: int
    productive: float
    overshoot: float
    near_resweep: float
    far_resweep: float
    direction: float
    dead: float
    overshoot_at_lane_ends: float
    dead_short: float
    dead_travel: float
    lanes: int
    dead_steps: int
    partial_steps: int
    full_steps: int

    @property
    def lost(self) -> float:
        return self.total_length - self.productive

    def shares(self) -> dict[str, float]:
        """
        Each part as a share of the summed tour length.
        """
        result = {}
        for part in PARTS:
            result[part] = getattr(self, part) / self.total_length

        return result


def fresh_offsets(offsets: np.ndarray) -> dict[tuple[int, int], np.ndarray]:
    """
    Per unit direction `e`, the cutter offsets `o` whose cell is fresh after
    a step by `e`: `q + o` was not under the footprint at `q - e`.
    """
    cells = {tuple(o) for o in offsets.tolist()}

    result = {}
    for dx, dy in DIRECTIONS:
        fresh = []
        for x, y in cells:
            if (x + dx, y + dy) not in cells:
                fresh.append((x, y))

        result[(dx, dy)] = np.array(fresh, dtype=np.int64).reshape(-1, 2)

    return result


def lane_gain(problem: Problem) -> int:
    """
    The new cells per step of a long straight sweep: the number of rows the
    cutter touches for a horizontal sweep, columns for a vertical one. The
    better of the two.
    """
    offsets = problem.offsets
    rows = len(set(offsets[:, 1].tolist()))
    columns = len(set(offsets[:, 0].tolist()))

    return max(rows, columns)


def lane_reference(problem: Problem) -> int:
    remaining = problem.num_cells - problem.k * len(problem.cutter)
    if remaining <= 0:
        return 0

    per_cutter = -(-remaining // (problem.k * lane_gain(problem)))

    return per_cutter + per_cutter % 2


def diagnose(problem: Problem, solution: CGSHOP2027Solution) -> Breakdown:
    """
    The breakdown of one solution. Raises if the accounting does not add up.
    """
    offsets = problem.offsets
    fresh = fresh_offsets(offsets)
    gain = problem.step_gain()

    # the cutter's own trail: a cell swept fewer steps ago than the cutter is long
    span = int(offsets.max() - offsets.min()) + 1

    tours = []
    for tour in solution.tours:
        corners = list(zip(tour.x, tour.y))
        tours.append(densify(corners, closed=True))

    # one frame that holds every footprint of every tour and the whole region
    everything = np.concatenate([*tours, problem.region_cells])
    low = everything.min(axis=0) + offsets.min(axis=0) - 1
    high = everything.max(axis=0) + offsets.max(axis=0) + 1
    width, height = (high - low + 1).tolist()

    in_region = np.zeros((height, width), dtype=bool)
    rx0, ry0, _, _ = problem.region.bounds
    mask = problem.region.mask
    top, left = ry0 - low[1], rx0 - low[0]
    in_region[top : top + mask.shape[0], left : left + mask.shape[1]] = mask

    # the global step at which each cell was first swept, minus one for never
    first_swept = np.full((height, width), -1, dtype=np.int64)

    parts = dict.fromkeys(PARTS, 0.0)
    per_lane = {"overshoot_at_lane_ends": 0.0, "dead_short": 0.0, "dead_travel": 0.0, "lanes": 0}
    counts = {"dead": 0, "partial": 0, "full": 0}
    new_total = 0
    start_total = 0
    clock = 0

    for positions in tours:
        # the first footprint of a tour comes free, before any step
        start = positions[0] + offsets - low
        start_new = in_region[start[:, 1], start[:, 0]] & (first_swept[start[:, 1], start[:, 0]] < 0)
        first_swept[start[start_new, 1], start[start_new, 0]] = clock
        start_total += int(start_new.sum())

        count = len(positions)
        new_per_step = np.zeros(count, dtype=np.int64)
        outside_per_step = np.zeros(count, dtype=np.float64)

        for index in range(count):
            here = positions[index]
            there = positions[(index + 1) % count]
            step = (int(there[0] - here[0]), int(there[1] - here[1]))
            clock += 1

            cells = there + fresh[step] - low
            xs, ys = cells[:, 0], cells[:, 1]

            inside = in_region[ys, xs]
            seen_at = first_swept[ys, xs]
            is_new = inside & (seen_at < 0)
            is_near = inside & (seen_at >= 0) & (clock - seen_at <= span)
            is_far = inside & (seen_at >= 0) & (clock - seen_at > span)

            new = int(is_new.sum())
            first_swept[ys[is_new], xs[is_new]] = clock
            new_total += new
            new_per_step[index] = new

            if new == 0:
                parts["dead"] += 1.0
                counts["dead"] += 1
                continue

            outside = int((~inside).sum()) / gain
            outside_per_step[index] = outside

            parts["productive"] += new / gain
            parts["overshoot"] += outside
            parts["near_resweep"] += int(is_near.sum()) / gain
            parts["far_resweep"] += int(is_far.sum()) / gain
            parts["direction"] += (gain - len(cells)) / gain

            if new == gain:
                counts["full"] += 1
            else:
                counts["partial"] += 1

        lane_parts = _lane_overheads(positions, new_per_step, outside_per_step, span)
        for key, value in lane_parts.items():
            per_lane[key] += value

    total_length = sum(len(positions) for positions in tours)
    _check_accounting(parts, total_length, new_total + start_total, problem.num_cells)

    return Breakdown(
        instance=problem.uid,
        family=family(problem.uid),
        k=problem.k,
        cells=problem.num_cells,
        gain=gain,
        lane_gain=lane_gain(problem),
        lower_bound=lower_bound(problem),
        lane_reference=lane_reference(problem),
        objective=max(len(positions) for positions in tours),
        total_length=total_length,
        new_cells=new_total,
        start_cells=start_total,
        dead_steps=counts["dead"],
        partial_steps=counts["partial"],
        full_steps=counts["full"],
        **per_lane,
        **parts,
    )


def _lane_overheads(
    positions: np.ndarray, new: np.ndarray, outside: np.ndarray, span: int
) -> dict[str, float]:
    """
    The waste of one tour that comes with each lane: overshoot near lane
    ends, short dead stretches, long dead stretches, and the lane count.
    """
    count = len(positions)
    steps = np.diff(np.concatenate([positions, positions[:1]]), axis=0)

    # straight runs, numbered in tour order
    starts = np.ones(count, dtype=bool)
    starts[1:] = np.any(steps[1:] != steps[:-1], axis=1)
    run_id = np.cumsum(starts) - 1

    # where each step sits inside its run, counted from either end
    run_start = np.flatnonzero(starts)[run_id]
    run_size = np.bincount(run_id)[run_id]
    from_start = np.arange(count) - run_start
    from_end = run_size - 1 - from_start
    near_end = np.minimum(from_start, from_end) < span

    is_dead = new == 0
    sweeps = np.bincount(run_id, weights=(~is_dead).astype(np.float64)) > 0
    in_lane = sweeps[run_id]

    # stretches of consecutive dead steps
    edges = np.diff(np.concatenate([[0], is_dead.astype(np.int64), [0]]))
    stretch = np.flatnonzero(edges == -1) - np.flatnonzero(edges == 1)

    return {
        "overshoot_at_lane_ends": float(outside[in_lane & near_end].sum()),
        "dead_short": float(stretch[stretch < span].sum()),
        "dead_travel": float(stretch[stretch >= 4 * span].sum()),
        "lanes": int(sweeps.sum()),
    }


def _check_accounting(parts: dict, total_length: int, swept: int, cells: int) -> None:
    summed = sum(parts.values())
    if abs(summed - total_length) > 1e-6 * max(1, total_length):
        msg = f"parts add up to {summed:.3f}, tours are {total_length} long"
        raise AssertionError(msg)

    # a verified solution sweeps every region cell exactly once first
    if swept != cells:
        msg = f"{swept} cells swept first, region has {cells}"
        raise AssertionError(msg)


def _row(result: Breakdown) -> str:
    shares = result.shares()
    loose = result.lane_reference / result.lower_bound if result.lower_bound else 0.0
    gap = result.objective / result.lower_bound - 1 if result.lower_bound else 0.0

    pieces = [f"{result.instance:30}", f"{gap:6.1%}", f"{loose:5.2f}"]
    for part in PARTS:
        pieces.append(f"{shares[part]:6.1%}")

    return " ".join(pieces)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("--instances", type=Path, default=DEFAULT_INSTANCES)
    parser.add_argument("--best", type=Path, default=DATA / "solutions" / "best")
    parser.add_argument("--only", default="")
    parser.add_argument("--json", type=Path, default=None, help="also write every breakdown here")
    args = parser.parse_args(argv)

    from .bench import read_bench_list

    uids = [uid for uid in read_bench_list() if args.only in uid]
    database = InstanceDatabase(str(args.instances))

    header = ["instance".ljust(30), "   gap", "lane/", *[part[:6].rjust(6) for part in PARTS]]
    print(" ".join(header))

    results = []
    for uid in uids:
        problem = Problem.from_instance(database[uid])
        solution = read_solution(args.best / f"{uid}.solution.json")

        result = diagnose(problem, solution)
        results.append(result)
        print(_row(result), flush=True)

    if args.json:
        args.json.write_text(json.dumps([asdict(result) for result in results], indent=2))

    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
