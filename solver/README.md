# solver: a CG:SHOP 2027 starting scaffold

Builds feasible solutions for every instance, verifies them with the official
`SolutionValidator`, and keeps the best one per instance.

```bash
python -m solver.run                                   # all instances in example_instances.zip
python -m solver.run --only pla7k --time-limit 5       # a subset, faster
python -m solver.run --methods cover-tsp               # one constructor
python -m solver.run --zip results/submission.zip      # pack the best solutions
python -m pytest tests/solver                          # solver tests
```

Output goes to `results/` (git-ignored):

- `results/best/<uid>.solution.json`: best verified solution, only replaced
  by a strictly better one, so runs accumulate.
- `results/summary.csv`: per instance, the lower bound, the provided baseline,
  our best and the gaps in percent.

## Pipeline

```
construct ──> repair ──> prune walk ──> split into k ──> close tours ──> prune tours ──> verify
(one open     (detours   (cut corners    (min-max        (bend back       (cut corners,
 walk)         for any    while still     binary search)   to start)        longest first)
               missed     covered)
               cells)
```

| Module | What it does |
| --- | --- |
| `model.py` | `Problem`: region cells, cutter offsets from its center, `k`, coverage helpers |
| `tours.py` | corners <-> unit positions <-> `CutterTour`; bends, closing, normalizing |
| `bounds.py` | lower bounds (area per unit step; extent for `k = 1`) |
| `construct/lanes.py` | back-and-forth lanes, horizontal and vertical |
| `construct/cover_tsp.py` | greedy set cover of anchor spots, nearest neighbour + windowed 2-opt |
| `repair.py` | adds out-and-back detours for any cell a walk misses |
| `split.py` | splits one walk into `k` closed tours, minimizing the longest |
| `improve.py` | local search: replace a corner by a shortcut if coverage survives |
| `pipeline.py` | runs one constructor through all stages, returns a verified `Attempt` |
| `run.py` | CLI batch runner, best-solution store, summary, submission zip |

## Adding an idea

- **New constructor**: write `def my_walk(problem: Problem) -> list[Point]`
  returning an open walk, register it in `construct/__init__.py`. Repair,
  splitting and verification come for free.
- **New improvement move**: add it to `improve.py` next to `_try_cut`; use
  `VisitCounts` to check coverage locally instead of re-verifying everything.

## Known weak spots (good next steps)

1. Lanes span gaps in the region and pick one fixed lane offset; skipping
   gaps and trying several offsets is cheap.
2. `cover-tsp` uses an O(n^2) nearest neighbour and a windowed 2-opt; a real
   TSP solver (LKH-3, OR-Tools) on the anchors should help a lot.
3. Splitting cuts one walk into consecutive pieces; partitioning the region
   first (one compact zone per cutter) avoids long trips back.
4. No rebalancing after the final prune: move work from the longest tour to
   the others.
5. Pure Python/numpy; profile, then move hot loops to numba or C++.
