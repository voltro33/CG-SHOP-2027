"""
Mutated bench solutions and their uncovered cell counts, for the Rust
coverage tests.

    .venv/Scripts/python.exe tools/golden/coverage/make_fixtures.py
    .venv/Scripts/python.exe tools/golden/coverage/make_fixtures.py --time euronight0070000_8749k_3s12

Every mutation moves a few consecutive corners of one tour by the same
vector, so a fixture line is a recipe, not a solution:

    uid expected tour index count dx dy

`count` corners starting at `index` (wrapping around) move by `(dx, dy)`.
`count == 0` is the stored best itself. The expected count comes from the
official verifier. Three kinds of mutation are drawn:

- cut a corner: one corner moves to the other bend between its neighbours
- shorten a lane: the two corners after an edge move back along it
- shift a tour: every corner moves
"""

import argparse
import random
import sys
import time
from pathlib import Path

from cgshop2027_pyutils.io import read_instance, read_solution
from cgshop2027_pyutils.schemas.solution import CGSHOP2027Solution, CutterTour
from cgshop2027_pyutils.verify import SolutionValidator

DATA = Path("data")
BENCH = DATA / "bench.txt"
INSTANCES = DATA / "instances" / "challenge_v0" / "instances"
BESTS = DATA / "solutions" / "best"
OUT = Path("crates/domain/coverage/tests/fixtures/mutations.txt")

CORNER_CUTS = 3
LANE_CUTS = 3
SHIFTS = 1
TRIES = 200


def bench_uids() -> list[str]:
    lines = BENCH.read_text().splitlines()
    return [line.strip() for line in lines if line.strip() and not line.startswith("#")]


def load(uid: str):
    instance = read_instance(INSTANCES / f"{uid}.instance.json")
    solution = read_solution(BESTS / f"{uid}.solution.json")
    return instance, solution


def corners(tour: CutterTour) -> list[tuple[int, int]]:
    return list(zip(tour.x, tour.y, strict=True))


def moved(points, index, count, dx, dy):
    result = list(points)
    for step in range(count):
        i = (index + step) % len(points)
        x, y = result[i]
        result[i] = (x + dx, y + dy)
    return result


def deduplicated(points):
    """Drops corners equal to their successor, cyclically, as the schema asks."""
    result = list(points)
    while len(result) > 1:
        repeats = [
            i for i in range(len(result)) if result[i] == result[(i + 1) % len(result)]
        ]
        if not repeats:
            break
        del result[repeats[0]]
    return result


def is_orthogonal(points) -> bool:
    pairs = zip(points, [*points[1:], points[0]], strict=True)
    return all(a[0] == b[0] or a[1] == b[1] for a, b in pairs)


def corner_cut(rng, points):
    """One corner moved to the other bend between its neighbours."""
    i = rng.randrange(len(points))
    prev, cur, nxt = points[i - 1], points[i], points[(i + 1) % len(points)]

    bend = (nxt[0], prev[1])
    if bend == cur:
        bend = (prev[0], nxt[1])
    return (i, 1, bend[0] - cur[0], bend[1] - cur[1])


def lane_cut(rng, points, reach):
    """The two corners after a long edge moved back along it."""
    i = rng.randrange(len(points))
    a, b = points[i], points[(i + 1) % len(points)]

    length = abs(b[0] - a[0]) + abs(b[1] - a[1])
    if length < 2:
        return None

    amount = rng.randint(1, min(length - 1, reach))
    ux = (b[0] > a[0]) - (b[0] < a[0])
    uy = (b[1] > a[1]) - (b[1] < a[1])
    return ((i + 1) % len(points), 2, -ux * amount, -uy * amount)


def shift(rng, points):
    dx, dy = rng.choice([(1, 0), (-1, 0), (0, 1), (0, -1), (2, 1), (-1, -3)])
    return (0, len(points), dx, dy)


def mutated_solution(solution, tour, recipe):
    tours = []
    for t, original in enumerate(solution.tours):
        points = corners(original)
        if t == tour:
            points = deduplicated(moved(points, *recipe))
        tours.append(CutterTour(x=[p[0] for p in points], y=[p[1] for p in points]))
    return CGSHOP2027Solution(instance_uid=solution.instance_uid, tours=tours)


def draw(rng, solution, make, wanted):
    """Up to `wanted` valid recipes from `make`, as `(tour, recipe)`."""
    found = []
    for _ in range(TRIES):
        if len(found) == wanted:
            break

        tour = rng.randrange(len(solution.tours))
        points = corners(solution.tours[tour])
        if len(points) < 4:
            continue

        recipe = make(rng, points)
        if recipe is None or recipe[2:] == (0, 0):
            continue
        if not is_orthogonal(deduplicated(moved(points, *recipe))):
            continue
        found.append((tour, recipe))
    return found


def fixture_lines(uid: str) -> list[str]:
    instance, solution = load(uid)
    validator = SolutionValidator(instance)
    rng = random.Random(uid)
    reach = 3 * (validator.cutter.width + validator.cutter.height)

    recipes = [(0, (0, 0, 0, 0))]
    recipes += draw(rng, solution, corner_cut, CORNER_CUTS)
    recipes += draw(rng, solution, lambda r, p: lane_cut(r, p, reach), LANE_CUTS)
    recipes += draw(rng, solution, shift, SHIFTS)

    lines = []
    for tour, recipe in recipes:
        expected = len(
            validator.uncovered_cells(mutated_solution(solution, tour, recipe))
        )
        lines.append(" ".join(str(v) for v in (uid, expected, tour, *recipe)))
    return lines


def time_verifier(uid: str, repeats: int = 3) -> None:
    instance, solution = load(uid)

    start = time.perf_counter()
    validator = SolutionValidator(instance)
    setup = time.perf_counter() - start

    timings = []
    for _ in range(repeats):
        start = time.perf_counter()
        missing = len(validator.uncovered_cells(solution))
        timings.append(time.perf_counter() - start)

    print(
        f"{uid}: rasterize {setup * 1e3:.1f} ms, check best of {repeats} {min(timings) * 1e3:.1f} ms, "
        f"uncovered {missing}"
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[1])
    parser.add_argument(
        "--time", metavar="UID", help="time the official check on one stored best"
    )
    args = parser.parse_args()

    if args.time:
        time_verifier(args.time)
        return 0

    lines = ["# uid expected tour index count dx dy (written by make_fixtures.py)"]
    for uid in bench_uids():
        lines.extend(fixture_lines(uid))
        print(lines[-1], file=sys.stderr)

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text("\n".join(lines) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
