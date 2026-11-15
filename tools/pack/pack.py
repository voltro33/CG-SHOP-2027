"""Pack the shorter of the Rust and Python best per instance into one zip.

Every chosen solution passes the official verifier first, and the finished
archive is read back with the official iterator. The zip is only written;
submitting it is up to its owner.

usage: python tools/pack/pack.py <repo root> <out zip>
"""

import sys
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

from cgshop2027_pyutils.io import read_instance, read_solution
from cgshop2027_pyutils.schemas.solution import CGSHOP2027Solution
from cgshop2027_pyutils.verify import check_for_errors
from cgshop2027_pyutils.zip import ZipSolutionIterator, ZipWriter

INSTANCES = "data/instances/challenge_v0/instances"
SOURCES = (("rust", "data/rust/best"), ("python", "data/solutions/best"))
WORKERS = 8


Choice = tuple[str, str | None, int | None, str | None]


def choose(args: tuple[Path, str]) -> Choice:
    """The shortest verified solution of one instance, Rust first on ties."""
    root, uid = args
    instance = read_instance(root / INSTANCES / f"{uid}.instance.json")

    found = []
    for source, folder in SOURCES:
        path = root / folder / f"{uid}.solution.json"
        if path.is_file():
            solution = read_solution(path)
            found.append((solution.max_tour_length, source, solution))

    for length, source, solution in sorted(found, key=lambda f: (f[0], f[1] != "rust")):
        if not check_for_errors(instance, solution):
            return uid, source, length, solution.model_dump_json()

    return uid, None, None, None


def main() -> None:
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    root, out = Path(sys.argv[1]), Path(sys.argv[2])
    if out.exists():
        sys.exit(f"{out} exists already")

    uids = sorted(
        p.name.removesuffix(".instance.json")
        for p in (root / INSTANCES).glob("*.instance.json")
    )
    print(f"{len(uids)} instances, verifying on {WORKERS} processes", flush=True)

    with ProcessPoolExecutor(max_workers=WORKERS) as pool:
        chosen = list(pool.map(choose, [(root, uid) for uid in uids]))

    missing = [uid for uid, source, _, _ in chosen if source is None]
    if missing:
        sys.exit(f"no verified solution for {missing}")

    with ZipWriter(out) as writer:
        for _, _, _, text in chosen:
            writer.add_solution(CGSHOP2027Solution.model_validate_json(text))

    read_back = {s.instance_uid: s.max_tour_length for s in ZipSolutionIterator(out)}
    if read_back != {uid: length for uid, _, length, _ in chosen}:
        sys.exit(f"{out} does not read back as written")

    rust = sum(1 for _, source, _, _ in chosen if source == "rust")
    print(
        f"{len(chosen)} solutions, {rust} from Rust, {len(chosen) - rust} from Python, all verified"
    )
    print(f"written {out} ({out.stat().st_size / 1e6:.1f} MB), read back intact")


if __name__ == "__main__":
    main()
