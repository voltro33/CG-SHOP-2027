"""
Writes `window_map` and `summarize` of `solver/window_map.py` for every
stored best and the default sides, and prints how long Python took.

    cd <python checkout>
    PYTHONPATH=. python <repo>/tools/golden/polish/export_window_map.py <repo>/crates/plugins/polish/tests/fixtures/window_map.txt

One line per instance and side:

    uid tours_hash | spans side gain total rows columns arrays_hash | summary

`tours_hash` is the hash of the best's corners, so the test can tell a best
that changed since. `arrays_hash` covers the length, need and pieces arrays,
and the summary is its five numbers as `repr` writes them.
"""

import sys
import time
from pathlib import Path

from canonical import BEST, fnv, load
from solver.window_map import DEFAULT_SIDES, cutter_span, summarize, window_map

from cgshop2027_pyutils.io import read_solution


def arrays_text(result) -> str:
    arrays = (result.length, result.need, result.pieces)
    lines = [
        " ".join(str(value) for value in array.ravel().tolist()) for array in arrays
    ]

    return "\n".join(lines) + "\n"


def main(out: Path) -> int:
    uids = sorted(
        path.name.removesuffix(".solution.json") for path in BEST.glob("*.json")
    )

    lines = []
    seconds = []
    for uid in uids:
        problem, _, corners = load(uid)
        solution = read_solution(BEST / f"{uid}.solution.json")

        for spans in DEFAULT_SIDES:
            side = spans * cutter_span(problem)

            started = time.perf_counter()
            result = window_map(problem, solution, side)
            elapsed = time.perf_counter() - started
            seconds.append((elapsed, uid, spans))

            summary = summarize(problem, result, spans)
            rows, columns = result.length.shape
            shape = (
                f"{spans} {side} {result.gain} {result.total_length} {rows} {columns}"
            )
            numbers = (
                summary.room_share,
                summary.worst_10_share,
                summary.worst_25_share,
                summary.worst_10_pieces,
                summary.worst_10_room,
            )

            text = " ".join(repr(number) for number in numbers)
            lines.append(
                f"{uid} {fnv(corners)} | {shape} {fnv(arrays_text(result))} | {text}"
            )

        print(uid, f"{sum(s for s, u, _ in seconds if u == uid):.2f}s", flush=True)

    out.write_text("\n".join(lines) + "\n")

    seconds.sort(reverse=True)
    print("total window_map seconds", round(sum(s for s, _, _ in seconds), 1))
    for elapsed, uid, spans in seconds[:10]:
        print(f"  {uid} spans={spans} {elapsed:.2f}s")

    return 0


if __name__ == "__main__":
    sys.exit(main(Path(sys.argv[1])))
