"""
Loads one instance the way the Python solver does and prints how long it
took, the most memory tracemalloc saw it hold (numpy included) and the
process's peak working set, to
compare with `cargo run --release -p mow-io --example load_time`. With
`MOW_HOLD` set it waits two seconds before exiting, as the Rust example does.

    .venv/Scripts/python.exe tools/golden/io/load_time.py <instance.json>
"""

import os
import sys
import time
import tracemalloc
from pathlib import Path

import psutil

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / "src"))

from solver.model import Problem  # noqa: E402

from cgshop2027_pyutils.io import read_instance  # noqa: E402


def main() -> None:
    before = psutil.Process().memory_info().peak_wset
    tracemalloc.start()
    started = time.perf_counter()
    problem = Problem.from_instance(read_instance(Path(sys.argv[1])))
    seconds = time.perf_counter() - started

    _, peak = tracemalloc.get_traced_memory()
    after = psutil.Process().memory_info().peak_wset
    print(
        f"{problem.uid} cells={problem.num_cells} load={seconds:.3f}s "
        f"traced_peak={peak / 2**20:.1f}MiB "
        f"process_peak={after / 2**20:.1f}MiB (after imports {before / 2**20:.1f}MiB)",
        flush=True,
    )

    if "MOW_HOLD" in os.environ:
        time.sleep(2)


if __name__ == "__main__":
    main()
