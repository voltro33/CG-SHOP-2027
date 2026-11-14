"""
Writes the tiles `solver/polish.py` lays over the longest tour and the order
`_aimed` puts them in, on the tours of the window fixtures.

    cd <python checkout>
    PYTHONPATH=. python <repo>/tools/golden/polish/export_tiles.py <repo>/crates/plugins/polish/tests/fixtures

Reads `windows/<uid>.tours` and writes `tiles/<uid>.txt`, one line per side
and seed:

    side offset longest | x0 y0 ... | shuffled | aimed

The tiles are those of `_tiles` with the given offset, before the shuffle,
as `x0 y0` pairs. `shuffled` is an order of them (indices into that list)
as `random.Random(seed).shuffle` leaves it, `aimed` the order `_aimed` makes
of the shuffled tiles. Rust does not draw the same random numbers, so the
offset and the shuffle are handed over rather than drawn.
"""

import random
import sys
from pathlib import Path

import numpy as np
from canonical import load
from solver.polish import _aimed, _longest, _tiles
from solver.tours import densify

SIDES = (8, 20)
SEEDS = (0, 1)


class FixedOffset:
    """
    Stands in for the random generator: a given offset and no shuffle.
    """

    def __init__(self, offset: int) -> None:
        self.offset = offset

    def randrange(self, _stop: int) -> int:
        return self.offset

    def shuffle(self, _items: list) -> None:
        return None


def read_tours(path: Path) -> list[np.ndarray]:
    tours = []
    for line in path.read_text().splitlines():
        numbers = [int(text) for text in line.split()]
        corners = list(zip(numbers[0::2], numbers[1::2], strict=True))
        tours.append(densify(corners, closed=True))

    return tours


def main(fixtures: Path) -> int:
    out = fixtures / "tiles"
    out.mkdir(parents=True, exist_ok=True)

    for path in sorted((fixtures / "windows").glob("*.tours")):
        uid = path.stem
        problem, _, _ = load(uid)
        tours = read_tours(path)
        longest = _longest(tours)

        lines = []
        for side in SIDES:
            for seed in SEEDS:
                rng = random.Random(seed)
                offset = rng.randrange(side)
                tiles = _tiles(tours[longest], side, FixedOffset(offset))

                order = list(range(len(tiles)))
                rng.shuffle(order)
                shuffled = [tiles[index] for index in order]

                aimed = _aimed(problem, tours, longest, shuffled)
                aimed_order = [tiles.index(tile) for tile in aimed]

                corners = " ".join(f"{tile.x0} {tile.y0}" for tile in tiles)
                lines.append(
                    f"{side} {offset} {longest} | {corners} | "
                    f"{' '.join(map(str, order))} | {' '.join(map(str, aimed_order))}"
                )

        (out / f"{uid}.txt").write_text("\n".join(lines) + "\n")
        print(uid, flush=True)

    return 0


if __name__ == "__main__":
    sys.exit(main(Path(sys.argv[1])))
