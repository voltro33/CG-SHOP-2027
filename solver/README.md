# solver: a CG:SHOP 2027 pipeline

Builds feasible solutions for every instance, verifies them with the official
`SolutionValidator`, and keeps the best one per instance.

All commands below are for Git Bash, run from the repo root.

## Setup, once

Python 3.10 or newer. The solver needs the package itself, its dev tools,
and OR-Tools for the CP-SAT polish. Pillow (for the picture window) comes
with matplotlib, and Tkinter ships with the python.org installer.

```bash
python -m venv .venv
source .venv/Scripts/activate        # every new terminal; or call .venv/Scripts/python directly
pip install -e ".[dev]" ortools
pre-commit install                   # ruff on every commit
```

The challenge instances go in `data/instances/challenge_v0/instances/`
(git-ignored). The example instances in `example_instances.zip` work without them.

## Tests

```bash
python -m pytest tests/solver -q     # solver tests, about 4 minutes
python -m pytest tests -q            # everything, the official pyutils tests too
python -m pytest tests/solver/test_polish.py -q   # one file
```

Lint and formatting run through the pre-commit hook on every commit.

## Running

```bash
# mining: every round, resumable, all cores but one
python -m solver.mine --hours 8
python -m solver.mine --rounds 9 --hours 5           # only some rounds
python -m solver.mine --only pla7k --hours 0.2       # only matching instances
python -m solver.mine --instances example_instances.zip --hours 0.2

# polish: CP-SAT window re-solves of the stored bests
python -m solver.polish --all --hours 5              # every best, through the miner
python -m solver.polish --only fjs3649 --seconds 300 # a few, for a quick look

# reports
python -m solver.report                              # headline numbers from data/leaderboard.csv
python -m solver.mine_report                         # which rounds and configs paid off
python -m solver.catalog                             # data/catalog.csv, one row per instance

# benchmark and diagnosis, on the fixed 31 instance set
python -m solver.bench pick                          # write data/bench.txt once
python -m solver.bench run best                      # score the stored bests
python -m solver.bench run "groups-h/gap_factor=0" --seconds 60
python -m solver.diagnose                            # where the tour length goes
python -m solver.window_map                          # where the waste sits, window by window

# pictures, see below
python -m solver.plot_app                            # the window
python -m solver.plot --all                          # every stored best into plots/
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
