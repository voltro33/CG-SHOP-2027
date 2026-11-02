# solver: a CG:SHOP 2027 pipeline

Builds feasible solutions for every instance, verifies them with the official
`SolutionValidator`, and keeps the best one per instance.

```bash
python -m solver.mine --hours 8 --workers 15            # overnight mining on the challenge instances
python -m solver.mine --instances example_instances.zip --hours 0.2
python -m solver.polish --all --hours 5                 # CP-SAT window polish of every stored best
python -m solver.plot_app                               # a window for drawing pictures of runs and bests
python -m solver.report                                 # headline numbers from data/leaderboard.csv
python -m solver.catalog                                # data/catalog.csv, one row per instance
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
| `repair.py` | adds out-and-back detours for any cell a walk misses |
| `split.py` | splits one walk into `k` closed tours, minimizing the longest |
| `partition.py` | `k` equal-cell bands with rebalancing rounds |
| `improve.py` | corner-cut local search; windowed coverage check for huge cutters |
| `pipeline.py` | `SolveConfig` + `solve`: one config through all stages, verified `Attempt` |
| `jobs.py` | the mining job grid, in rounds |
| `mine.py` | parallel resumable miner, best store, crash recovery |
| `polish.py` | re-solves 20 x 20 windows of the longest tour with CP-SAT, splices in shorter walks |
| `plot.py` | pictures of stored bests and runs with the official plot, into `plots/` |
| `plot_app.py` | a Tkinter window over `plot.py`: pick runs or instances, draw, preview |
| `report.py` | leaderboard and summary numbers |

## Pictures of solutions

Run these from the repo root with the virtual environment active (Git Bash:
`source .venv/Scripts/activate`, or call `.venv/Scripts/python` directly).

The window:

```bash
python -m solver.plot_app
```

- Left: every instance with a stored best and its length. Type in the box to filter.
- Middle: the stored runs of the instance clicked last, shortest first.
- Click selects one, shift click a range, ctrl click adds one, ctrl a selects all.
- **Draw selected runs** draws every selected run, **Draw bests of selected instances** the best of each.
- `bare` drops axes and titles for slides, `gif` also writes an animation, `dpi` sets the resolution.
- The last picture shows on the right; double click it to open it full size. **Stop** ends a long batch.

The same from the command line:

```bash
python -m solver.plot fjs3649_17k_1x7                  # the best of one instance
python -m solver.plot fjs3649_17k_1x7 --runs           # list its stored runs
python -m solver.plot fjs3649_17k_1x7 --run groups-v   # the runs whose name has that text
python -m solver.plot --bench                          # the bests of the benchmark set
python -m solver.plot --all --max-cells 200000         # every best up to that size
```

A best goes to `plots/<instance>.png`, a run to `plots/<instance>/<run>.png`.
Instances above a million cells take a while to draw.

## Adding an idea

- **New constructor**: write `def my_walk(problem: Problem, **knobs) -> list[Point]`
  returning an open walk, register it in `construct/__init__.py`. Repair,
  splitting and verification come for free.
- **New knob or config**: add it to `configs_for` in `jobs.py`. If code changes make old results
  stale, bump `CODE_VERSION` so the miner reruns every config.
- **New improvement move**: corner-level moves go in `improve.py` (use `VisitCounts`
  to check coverage locally); order-level moves go in `ordering.py`.

Concept docs for all of this live in `docs/concepts/`, terms in `docs/glossary.md`.
