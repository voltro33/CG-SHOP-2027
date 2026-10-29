# solver: a CG:SHOP 2027 pipeline

Builds feasible solutions for every instance, verifies them with the official
`SolutionValidator`, and keeps the best one per instance.

```bash
python -m solver.mine --hours 8 --workers 15            # overnight mining on the challenge instances
python -m solver.mine --instances example_instances.zip --hours 0.2
python -m solver.report                                 # headline numbers from data/leaderboard.csv
python -m solver.catalog                                # data/catalog.csv, one row per instance
python -m solver.run --only pla7k --time-limit 5        # quick single run, results/ (older runner)
python -m pytest tests/solver                           # solver tests
```

Mining output goes to `data/` (large parts git-ignored):

- `data/solutions/best/<uid>.solution.json`: best verified solution, replaced only by a strictly better one.
- `data/solutions/candidates/<uid>/`: verified solutions within 5% of the best.
- `data/jobs.jsonl`: one line per finished job; restarts skip these.
- `data/leaderboard.csv`: bound, baseline, best, gap per instance. `data/mine.log`: the run log.

## Pipeline

```
single walk:  segments -> ILS order -> repair -> prune walk -> split into k -> close -> prune tours -> verify
partitioned:  k bands -> per band (segments, order, repair, close, prune) -> rebalance -> prune -> verify
```

| Module | What it does |
| --- | --- |
| `model.py` | `Problem`: region cells, cutter offsets from its center, `k`, coverage helpers |
| `tours.py` | corners <-> unit positions <-> `CutterTour`; bends, closing, normalizing |
| `bounds.py` | lower bounds (area per unit step; extent for `k = 1`) |
| `catalog.py` | instance features into `data/catalog.csv` |
| `construct/segments.py` | lanes cut into segments at gaps; knobs `gap_factor`, `shift`, `seed`, `kick_seconds`, `anneal` |
| `construct/ordering.py` | segment order: 2-opt + Or-opt on neighbour lists, iterated local search, optional annealing |
| `construct/lanes.py` | plain back-and-forth lanes, horizontal and vertical |
| `construct/cover_tsp.py` | greedy set cover of anchor spots, nearest neighbour + windowed 2-opt |
| `repair.py` | adds out-and-back detours for any cell a walk misses |
| `split.py` | splits one walk into `k` closed tours, minimizing the longest |
| `partition.py` | `k` equal-cell bands with rebalancing rounds |
| `improve.py` | corner-cut local search; windowed coverage check for huge cutters |
| `pipeline.py` | `SolveConfig` + `solve`: one config through all stages, verified `Attempt` |
| `jobs.py` | the mining job grid, in rounds |
| `mine.py` | parallel resumable miner, best store, crash recovery |
| `report.py` | leaderboard and summary numbers |
| `run.py` | the original batch runner (writes `results/`) |

## Adding an idea

- **New constructor**: write `def my_walk(problem: Problem, **knobs) -> list[Point]`
  returning an open walk, register it in `construct/__init__.py`. Repair,
  splitting and verification come for free.
- **New knob or config**: add it to `configs_for` in `jobs.py`. If code changes make old results
  stale, bump `CODE_VERSION` so the miner reruns every config.
- **New improvement move**: corner-level moves go in `improve.py` (use `VisitCounts`
  to check coverage locally); order-level moves go in `ordering.py`.

Concept docs for all of this live in `docs/concepts/`, terms in `docs/glossary.md`.
