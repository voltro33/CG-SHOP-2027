# PROGRESS

Running log for the overnight plan (branch `init-solver`, local commits only,
never pushed).

Decisions made with Geo at kickoff (2026-10-28):

- Extend the existing `solver/` package rather than starting `src/mow/`. Plan
  paths map as: `src/mow/catalog.py` -> `solver/catalog.py`,
  `split/partition.py` -> `solver/partition.py`, and so on.
- Challenge instances v0 (250) copied from Downloads to
  `data/instances/challenge_v0/` (git-ignored, untouched).
- Mining budget: 8 hours, 15 workers (16-core machine).
- pyutils: this repo _is_ the pyutils source (v0.1.0, editable install). Not
  upgraded to 0.2.0 tonight; the verifier semantics below were read directly
  from `src/cgshop2027_pyutils/verify.py`.
- Code style: Python written in readable "paragraphs" (related lines together,
  blank lines between steps), plain loops over clever one-liners, short
  lower-case comments where intent is not obvious.

## Phase 0: Environment (done)

- `.venv` (Python 3.10.11) already had pyutils, numpy, shapely, pytest,
  matplotlib. Added `ortools 9.15`.
- Full suite: 338 tests pass. `test_shipped_example_solutions_verify_and_agree`
  checks every shipped example solution with the official verifier: zero errors.
- Committed the pre-existing solver scaffold as-is first (`1eddb7d`).

## Phase 1: Instance catalog (done)

- `python -m solver.catalog` -> `data/catalog.csv` (75 example + 250 challenge
  instances, ~7 s on 15 workers).
- Findings in `docs/concepts/instances.md`. Headline: **cutters are not
  uniform.** Five families (square, diamond, clipped square, plus, X), sizes 3
  to 72 cells across. Challenge instances are far larger than the examples:
  median ~104k cells, 40 instances over 1M, max 8.7M.

## Phase 2: Coverage cross-check (done)

- Our model (`Problem.uncovered`) matched the official uncovered-cell **count**
  on all shipped solutions and on 200/200 random tours (tours allowed to leave
  the region). Zero mismatches.
- Rule recorded in `docs/concepts/coverage.md`.

## Timing probe (existing pipeline, challenge instances)

| instance          | cells | lanes-h      | lanes-v      | cover-tsp    |
| ----------------- | ----- | ------------ | ------------ | ------------ |
| bck2217_18k_1d6   | 18k   | 3882         | 3880         | 5156         |
| bgb4355_76k_2l3   | 76k   | 9072         | 9380         | 13470        |
| bch2762_1508k_3p5 | 1.5M  | 25668 (18 s) | 25398 (21 s) | 36852 (34 s) |

Lanes beat cover-tsp on challenge instances (the opposite of the examples).
Lanes scale fine.

## Phase 3: Strip baseline (done)

- Existing `lanes-h/v` already met the acceptance bar. Added `segments-h/v`
  (`solver/construct/segments.py`): lanes cut into segments at gaps (bridging
  gaps under `gap_factor` lane heights), ordered by nearest neighbour + 2-opt,
  either driving direction.
- On 33 challenge instances (<= 400k cells), segments beat plain lanes on most,
  up to 32% shorter (srpg5461_22k_1l3: 23706 -> 16170). Score proxy 27.2 -> 32.4
  of 33.
- Acceptance: all 75 example instances feasible with lanes and segments
  (official verifier), 72 better than the shipped baseline solutions.
- **Bug found and fixed:** the corner-cut prune built a `removed x cutter cells`
  array, which needed 5.5 GB on the 8.7M-cell instance with a 72x72 cutter. Now
  switches to a windowed mask check above 2M entries (`_all_swept_windowed`,
  tested against the pointwise version).

## Phase 4: Multi-cutter split (done)

- `solver/partition.py`: k equal-cell bands (cuts along x or y), each solved as
  k = 1, then up to 4 rebalancing rounds (shares scaled by
  `sqrt(mean / length)`). Selected with `SolveConfig(partition=...)`.
- On 29 challenge instances with k > 1, the existing "split one walk" is
  slightly better on average, but bands win by up to 8% on some, so the miner
  runs both.
- Balanced k-means partitioning: **not done**; bands plus the walk split already
  give near-perfect balance (see Phase 6 note), so it would not move the
  objective much.
- `data/leaderboard.csv` is written by the miner (Phase 7) with baseline, best,
  bound and gap.

## Phase 5: Lower bounds (done)

- `solver/bounds.py` already had the area bound (with the exact per-direction
  step gain instead of the plan's `max(w, h)`, which is valid and tighter) and
  an extent bound for k = 1.
- `tests/solver/test_bounds.py`: bound <= every shipped example solution and
  every mined best (verified).
- Derivation in `docs/concepts/lower-bounds.md`.

## Phase 7 before Phase 6 (deviation, on purpose)

Mining needs wall-clock time, so the miner was built and started before the
improvement work, and improvements joined it by restarting (resumable, so
nothing verified was lost). Restarts at 00:00, 00:12, 00:18 and 00:25; current
run started 00:25 with a 7.7 h budget.

## Phase 7: Parallel mining (done)

- `solver/mine.py` (runner, best store, job log), `solver/jobs.py` (config grid
  in rounds), `solver/report.py` (leaderboard and summary).
  `python -m solver.mine --hours 8 --workers 15`.
- Acceptance: hard-killed a run after 40 s (1040 jobs logged), restarted it:
  75/75 bests intact, 0 lost or worse, 34 improved, all re-verified, no
  duplicate jobs, no stray temp files.
- **Memory:** one worker on an 8.7M-cell instance reached 3.5 GB, so at most 3
  jobs on > 3M-cell instances run at once.
- **Bug found and fixed:** a dead worker breaks the whole `ProcessPoolExecutor`;
  the first version would then have logged every remaining job as failed. Now
  jobs caught in a broken pool are requeued, the pool is rebuilt, and suspects
  rerun alone so only the true culprit is given up on
  (`tests/solver/test_mine.py` kills a worker on purpose). 35 records lost to
  killed workers during restarts were removed so they rerun.
- Job ids carry a code version (`|v2`); bumping `CODE_VERSION` reruns every
  config with new code.
- Candidates are kept only within 5% of the best (not every candidate as
  planned): the repo is in OneDrive, and thousands of solution files would sync
  for no benefit.

## Phase 6: Improvement loop (done, measured)

Measured first: in segment walks, **links between segments are 14-44% of the
length**, while the mowing part sits near `cells / lane height`. And k > 1 tours
are already balanced (max/mean median 1.002, p90 1.017). So the effort went into
ordering, not rebalancing.

1. **Strip ordering:** OR-Tools routing (2 nodes per segment, free intra arc)
   cut links 13-17% on 50-100 segments in 10 s, but lost to plain 2-opt at 728
   segments (Python distance callback). Replaced by
   `solver/construct/ordering.py`: 2-opt + Or-opt over 8 spatial neighbours,
   then **iterated local search** with block-reversal and local double-bridge
   kicks. Links -8% to -26%; end to end (official verifier) better on 22 of 25
   sampled instances, typically 1-8%. k = 1 orders are optimized as closed
   cycles.
2. **2-opt on corner sequences:** covered by the existing corner-cut prune
   (`improve.py`) plus 2-opt on segments.
3. **Rebalancing:** exists inside band partitioning; a separate post-hoc
   rebalance was skipped because the measured imbalance (0.2% median) caps its
   gain.
4. **Simulated annealing:** applied to the segment order instead of rebalancing
   (where the length is). `anneal=0.003` beat plain ILS on 3 of 5 sampled
   instances; runs as mining round 6.

Phase 6 acceptance on the example set is measured after the overnight mine (CPU
is busy until then).

## Overnight mining result

Run from 00:25 to 08:12 (7.7 h, 15 workers). 8003 of 10396 planned jobs
finished, **0 failed**. Rounds run cheapest first (0, 1, 2, 5, 6, 3, 4): 236
instances got through the annealing round (6), only 31 reached the last round
(4, extra lane shifts). Leaderboard committed as `data/leaderboard.csv`.

|                                      |                                                               |
| ------------------------------------ | ------------------------------------------------------------- |
| Feasible (official verifier)         | **250 / 250**                                                 |
| Summed max tour length               | 5,291,192 baseline -> **5,231,356** (-1.1%)                   |
| Instances better than baseline       | 193 of 250 (median -1.6% among those)                         |
| Estimated relative score vs baseline | about **+3.2%** (sum of `(baseline/best)^2` = 258.0 over 250) |
| Median gap to lower bound            | **47.9%** (quartiles 33% / 48% / 88%, best 8.8%)              |

Median gap by cutter: X 100% (52 instances), diamond 46%, plus 44%, clipped
square 39%, square 39%. Median gap by k: 1 -> 47.5%, 2 -> 50.4%, 3 -> 49.0%.

Winning configs: plain segments 175, segments + band partition 57, extra ILS
seeds 6, annealing 4. The strictest gap settings (`gap_factor` 0 and 1) won 144
between them.

What it says:

1. **The X cutter is the weak spot.** Its median gap is double every other
   family, and all five worst gaps are X instances. Likely cause, not yet
   measured: lanes treat the X like its bounding box, but it only sweeps its
   diagonals, so one lane leaves holes that repair patches expensively. Lanes
   may be the wrong primitive here.
2. **Gains came early.** Base segment configs won almost everything; extra seeds
   and annealing rarely beat them. More rounds of the same search will not move
   the total much.
3. **k is not the problem.** Gaps are flat across 1, 2 and 3 cutters, so the
   split and partition are not where length is lost.
4. **The gap overstates the room.** The area bound is loose, so part of the 48%
   cannot be closed.

Data quirk: 8 leaderboard rows show a raw job id in `best_config` instead of a
config label. They are bests saved before the label fix; the solutions
themselves are verified.

## Morning report

**Phases completed:** 0 to 7 done. 8 (docs) partly: concept pages, diagrams,
glossary and solver README are committed; the Marp slide deck is not written
yet. **Feasibility:** 250/250 challenge instances, 75/75 example instances.
**Summed max tour length:** baseline 5,291,192 -> best 5,231,356 (-1.1%).
**Median gap to bound:** 47.9%. **Biggest wins:** spg-c-poly6002_28k_1s4 -10.8%,
spg-c-poly6002_995k_3s4 -10.4%, spg-c-poly6002_111k_2s4 -8.3%, srpg5461_82k_1s11
-7.5%, iso26643_27k_1s4 -6.5%. **Worst gaps:** iso3131_107k_2x10 296%,
octa6025_952k_3x9 278%, isoa22945_987k_3x10 237%, isoa22945_110k_2x10 231%,
octa28351_946k_3x7 211%. **Blockers / open questions:**

- Phase 6 acceptance on the example set (baseline rounds vs improved) was not
  measured after the run.
- Git history was rewritten on 2026-10-29 at the owner's request: no co-author
  lines, no phase numbers in messages, docs committed separately. Old history
  kept on local branch `init-solver-backup`. **Recommended next step:** see
  `plans/2026-10-29-today.md`. A benchmark set and a breakdown of where tour
  length goes, before any new constructor. Square cutters at a 39% gap, where
  the bound is exact per step, show the loss is in construction overhead, not
  only in the X cutter.

## Blockers

(none)

## 2026-10-29: LKH check for the segment order

LKH 3 through `elkai` 2.0.1 (PyPI, Windows wheel; installed to the scratchpad
only, not the venv). Building LKH-3 from a GitHub mirror was blocked by the
session's safety check; the official site refused connections. Segments as two
end nodes each, own edge free, closed tour:

| instance         | segments | our ILS links | LKH links | LKH time |
| ---------------- | -------- | ------------- | --------- | -------- |
| fht47608_34k_2p4 | 50       | 997           | 997       | 0.0 s    |
| smo3467_11k_1x5  | 102      | 1108          | 1094      | 0.1 s    |
| map_frog_48k_1x9 | 112      | 2353          | 2327      | 0.1 s    |
| octa383_102k_2d6 | 84       | 2496          | 2472      | 0.1 s    |
| srpg5461_22k_1l3 | 728      | 4581          | 4461      | 7.5 s    |

LKH is 0 to 2.6% better on links, much faster on small orders, but needs a dense
matrix (no good past a few thousand segments) and has no min-max k objective in
`elkai`. Since links are 14 to 44% of the tour, that is at most about 1% of tour
length. **Our order is already near what LKH finds; the waste is in the lanes.**

## 2026-10-29: step 1, score columns in the report

`python -m solver.report` now prints `pessimistic_score` (sum of
`(bound / best)^2`) and the median gap per cutter family, read from the name
suffix. Current leaderboard: **109.9 / 250**. Family medians: X 100.2%, diamond
46.4%, plus 44.0%, square 38.8%, clipped square 38.6%. Tests in
`tests/solver/test_report.py`.

## 2026-10-29: step 2, benchmark set and runner

`data/bench.txt` (committed): 31 instances, two per (family, k) up to 400k cells
(there are no square k=3 instances that small, so 14 strata), plus the 3 worst X
gaps above 400k: octa6025_952k_3x9, isoa22945_987k_3x10, octa28351_946k_3x7.

`python -m solver.bench run best` reruns each instance's overnight winning
config: **self score 31.0 / 31**, 31/31 feasible, every ratio between 0.997 and
1.001, **1 min 40 s** on 15 workers. Reproducible. Two reruns were slightly
shorter and got promoted (iso8005_26k_1s3 11554 -> 11548, octa28351_946k_3x7
14820 -> 14814). Benchmark pessimistic score: 11.01 of 31. Run logs and
summaries go to `data/bench/` (git-ignored, added to `.gitignore` next to
`plans/`).

## 2026-10-29: step 3, where the length goes

`python -m solver.diagnose` walks every tour step by step. Accounting checks
pass on all 31 benchmark bests (parts add up to the summed length, every region
cell is swept first exactly once). 9 s for the whole set. Shares of summed tour
length:

| family         | n   | productive | overshoot (at lane ends) | near resweep | far resweep | dead (short / travel) | waste per lane | lane ref / bound |
| -------------- | --- | ---------- | ------------------------ | ------------ | ----------- | --------------------- | -------------- | ---------------- |
| all            | 31  | 53%        | 20% (8%)                 | 6%           | 1%          | 19% (6% / 5%)         | 28             | 1.00             |
| X              | 9   | 39%        | 24% (9%)                 | 15%          | 1%          | 21% (7% / 5%)         | 83             | 1.50             |
| clipped square | 6   | 63%        | 16% (7%)                 | 1%           | 1%          | 19% (6% / 5%)         | 37             | 1.00             |
| diamond        | 6   | 60%        | 18% (9%)                 | 1%           | 1%          | 20% (5% / 4%)         | 14             | 1.00             |
| plus           | 6   | 62%        | 19% (7%)                 | 1%           | 1%          | 17% (5% / 5%)         | 23             | 1.00             |
| square         | 4   | 64%        | 17% (8%)                 | 0%           | 0%          | 18% (6% / 2%)         | 9              | 1.00             |

Short dead = stretches under one cutter span, travel = 4 spans or more; the rest
of dead is 1 to 4 spans. Waste per lane = lost length / number of sweeping
straight runs.

Split again over all steps, as shares of the _lost_ length: overshoot on lane
sides 26%, overshoot at lane ends 18%, dead steps inside runs that also sweep
32%, pure travel runs 10%, X near resweep about 13%.

What it says:

1. **Resweep between lanes is about 1%.** Lane spacing (4c) is not a lever. The
   X near resweep (15%) is the cutter running over its own trail inside one
   lane; lane reference / bound is 1.5 for X, so most of the X gap is the area
   bound being loose for X, not lost length we can win back.
2. **Pure travel is small (about 10% of lost).** Matches the LKH check: the
   order is fine.
3. **Most waste comes with each lane:** lane end overshoot, turns and short dead
   stretches. Fewer, longer lanes remove it. Side overshoot (lanes straddling
   the boundary) is the second block, 26%.

**Decision: 4b first** (sweep direction per piece, fewer and longer lanes), then
4a (lanes that hug the boundary) for the side overshoot. 4c dropped.

## 2026-10-29: step 4, removing the biggest waste

### 4b: mixed lanes (built, kept as an extra config)

`solver/construct/mixed.py`, method `segments-mixed`. Tiles of `tile_lanes`
lanes on the lane grid, each tile horizontal or vertical. First version picked
per tile on its own and lost (29.62 / 31): tile borders cut lanes. Second
version starts from the better plain direction and flips single tiles only when
the exact segment cost (`length + segment_cost * band` per segment) of the
tile's row and column strips goes down.

| config                                                         | self score / 31 | bench pessimistic |
| -------------------------------------------------------------- | --------------- | ----------------- |
| overnight bests (`best`)                                       | 31.00           | 11.01             |
| `segments-h/gap_factor=0` alone                                | 30.13           | 10.68             |
| `segments-mixed` (tile 8, cost 2, first version)               | 29.62           | 10.54             |
| `segments-mixed/gap_factor=0` (hill climb)                     | 29.56           | 10.49             |
| `segments-mixed/gap_factor=0/tile_lanes=2/segment_cost=4`      | 29.54           | 10.49             |
| `segments-mixed/gap_factor=0/bands-x` (k=3 only, 11 instances) | 10.03 / 11      |                   |
| **best of all runs today**                                     | **31.18**       |                   |

The segment level cost only drops 0 to 2% versus the better plain direction, so
mixing directions on a tile grid does not remove much lane overhead on these
shapes. As one more config in the portfolio it shortens 12 of 31 benchmark
instances by 0.1 to 2.5% (best: srpg22373_399k_3d4 -2.5%, iso3713_104k_2p5
-2.4%, iso5009_399k_3x5 -1.4%). Added to `jobs.py` as round 7 (4 variants plus
band partitions for k > 1), run right after the gap sweep. `CODE_VERSION` not
bumped on purpose: that would rerun the whole old grid. The tile search takes 10
s on the largest instance (8.7M cells).

### Checks that ruled ideas out

- **4a ceiling (lanes hugging the boundary):** if every column could place its
  lanes freely, it would need `ceil(run height / lane height)` lane cells per
  run. Against the global grid that saves **4.9%** of lane length over the
  benchmark, an upper estimate that ignores lane continuity. Realistic gain 2 to
  3% of tour length.
- **Thin cells:** share of region cells that no cutter placement fully inside
  the region reaches (morphological opening). Tracks the gap closely: srpg41087
  (s5) 83% thin with 112 to 122% gap, smo64863_393k_3p6 66% and 136%, the low
  gap instances are 1 to 4% thin. Overshoot on thin cells is forced for any
  constructor.
- **Local gain lower bound:** `sum over cells of 1 / g(c)`, where `g(c)` is the
  most region cells a step that newly sweeps `c` can bring (valid: new cells of
  a step into `q` with direction `e` lie in `q + F_e`). Only 0 to 8% above the
  area bound (bench pessimistic 11.01 -> 11.13), because a corridor cell still
  has a step direction that sweeps a full row of the corridor. Not worth adding.
- **Picture of iso8005_26k_1s3:** dead steps trace the region boundary at lane
  ends, where the cutter runs along the ragged edge to the next lane's start.

## Step 5: mining round 7, and the memory crash it hit

- **First attempt (12:56):** `solver.mine --hours 2 --workers 15`. A worker died
  30 s in ("worker pool broke, 3722 jobs requeued"), then the machine ran
  critically low on memory and the run was stopped. One job finished.
- **Cause:** not mixed lanes. `prune_routes` built an anchor array of covered
  cells times cutter cells, and only the smaller removed times cutter cells
  product was capped. paris0003000_2020k_3x10 (1872 cell cutter) peaked at **4.4
  GB** in one plain `segments-h` job. Fixed in 542d9d0: **152 MB**, same
  objective 18060.
- **Peak memory per job after the fix** (40 s runs, one at a time):

  | instance                    | cells | box with cutter | segments-h | mixed | mixed bands-x |
  | --------------------------- | ----- | --------------- | ---------- | ----- | ------------- |
  | paris0003000_2020k_3x10     | 2.0M  | 3.8M            | 152 MB     | 153   | 133           |
  | srpg133_3600k_3d12          | 3.6M  | 17.9M           | 500        | 501   | 415           |
  | smo10575_6398k_3x9          | 6.4M  | 17.4M           | 491        | 497   | 403           |
  | pla85900_7154k_3p8          | 7.2M  | 10.7M           | 320        | 320   | 265           |
  | euronight0070000_8749k_3s12 | 8.7M  | 14.6M           | 414        | 415   | 339           |

  Fits 50 MB plus 26 bytes per box cell within a few percent; cells alone do not
  predict it.

- **Scheduler rewrite (68e268c):** a core semaphore and a megabyte semaphore
  sized from the machine (all cores but one; 80% of free memory after a 2 GB
  reserve). A job takes 1 core and 100 MB + 40 bytes per box cell, gets its own
  process only when both fit, and returns them on exit. A crash loses only its
  own job.
- **Smoke run (0.1 h, defaults, 15 cores and 7015 MB permits):** at most 11
  worker processes, memory was the limit, total peak 3.3 GB, free memory never
  below 7.9 GB, no failures. 12 jobs finished, all big round 2 and round 7 jobs
  at their 600 s cap, no new bests.
- **Not done yet:** the full round 7 run.

## Improve plan, step 1: where the waste sits, in windows

Plan: `plans/2026-10-29-improve.md`. Work on branch `lns` in the worktree
`../CG-SHOP-2027-lns`, so the running miner keeps its code.
`solver/window_map.py` (702350b): anchors cut into square windows of 2, 4 or 8
cutter spans; per window the length inside it, the cells only it sweeps
(`need`), room `length - need / gain`, and how often the tours enter it. 5 min
for the 31 benchmark bests.

Medians over the benchmark:

| window side | room / tour length | worst 10% of windows hold | worst 25% hold | pieces in the worst windows |
| ----------- | ------------------ | ------------------------- | -------------- | --------------------------- |
| 2 spans     | 62%                | 19% of the room           | 39%            | 3                           |
| 4 spans     | 51%                | 18%                       | 38%            | 4                           |
| 8 spans     | 44%                | 18%                       | 38%            | 6                           |

Room at 4 spans by family: X 68%, square 52%, diamond 51%, plus 42%, clipped
square 39%.

What it says:

1. **Gate 1 passes, but it says little.** The room is measured against the area
   rate (`need / gain`), so at 8 spans it is just the 47% global loss spread
   over windows. It cannot tell waste a re-solve can remove from waste the
   region forces (thin parts, lane ends that must turn somewhere). The step 2
   pilot is the real test.
2. **The waste is spread evenly, not in hot spots.** The worst 10% of windows
   hold 18% of the room, and an even spread would give 10%. Polishing only the
   worst windows would miss most of it. The loop has to visit every window, so
   **speed per window is what matters**: iso8005_26k_1s3 has 336 windows at 4
   spans.
3. **4 spans looks right for the model:** worst windows have about 4 pieces to
   link. At 8 spans it is 6 to 9.

## Improve plan, step 2: CP-SAT window pilot — gate fails

Miner stopped at 16:04 to free the cores (10 new bests kept, pessimistic score
about +0.07; resume later). `solver/window.py` (74ba0b7, branch `lns`): cuts the
longest tour inside a window into pieces, CP-SAT finds the shortest walk through
the window that keeps every outside run, covers every cell nothing else sweeps,
and stays one closed walk (steps usable twice, flow for connectivity, old pieces
as a complete hint and upper bound). Pilot: 31 benchmark bests, two windows each
(worst by room, and random), 15 CP-SAT workers, splice then official verifier.

| windows                                        | limit | proven optimal | no answer | any saving | median saving | mean | pooled saving |
| ---------------------------------------------- | ----- | -------------- | --------- | ---------- | ------------- | ---- | ------------- |
| 4 cutter spans (median 2,800 anchors, X 9,200) | 10 s  | 5 / 62         | 7         | 17 / 62    | **0%**        | 3.4% |               |
| 20 x 20 anchors                                | 5 s   | 15 / 62        | 0         | 12 / 62    | **0%**        | 3.6% | 2.7%          |

Every spliced solution that came back passed the official verifier (55 and 62).

What it says:

1. **Gate 2 fails on both counts.** Median saving 0% (needed 5%), and big
   windows do not solve in 5 s.
2. **At small scale the tours are already locally optimal.** The 15 windows
   CP-SAT proved optimal (0.3 s each) had a median saving of 0: the current tour
   was already the best walk through them.
3. **At the scale where the waste is, the model is too big.** Windows of 4 spans
   have thousands of anchors; most time out, and some find nothing better than
   the hint.
4. The savings that exist are in a few windows (mean 3.6% against median 0),
   mostly X. Pooled 2.7% at about 0.3 saved steps per second of 15 core solving:
   a full pass over one instance of median tour length would take 10 to 15
   minutes of the whole machine, so overnight covers a few dozen instances.

## Paper and mining report

**A Closer Cut** (Fekete et al., ALENEX 2023, arXiv 2211.05891): witness sets
plus exact close enough TSP give lower bounds and tours, optimal or within 3% up
to region / cutter area about 6, up to 50% above their bound at 30, and the
method stalls beyond about 50. Ours: 416 to 113,657, median 926, none under 50.
No scalable construction to borrow; the witness lower bound is worth a grid
version to see how much of our gap is forced.

**`python -m solver.mine_report`** (5c61525) over 8,639 logged jobs, 133 CPU
hours:

| round                               | jobs | cpu h | improvements | points | points per cpu h |
| ----------------------------------- | ---- | ----- | ------------ | ------ | ---------------- |
| 0 base                              | 971  | 9.3   | 410          | 6.58   | 0.706            |
| 1 partitions                        | 1282 | 26.5  | 168          | 1.76   | 0.066            |
| 2 gap sweep                         | 2000 | 25.4  | 310          | 2.55   | 0.100            |
| 3 shifts                            | 1416 | 9.1   | 32           | 0.22   | 0.024            |
| 4 cross                             | 492  | 25.3  | 22           | 0.11   | 0.004            |
| 5 seeds                             | 1416 | 17.7  | 10           | 0.04   | 0.002            |
| 6 annealing                         | 944  | 11.9  | 5            | 0.02   | 0.002            |
| 7 mixed (big instances only so far) | 118  | 8.0   | 9            | 0.05   | 0.006            |

- 14 of 55 configs never won (21.8 CPU hours): `lanes-h/v`, `cover-tsp`,
  `gap_factor=8`, every `gap_factor=4` with band partitions,
  `gap_factor=0/shift=0.5/bands-y`.
- Points at stake (score lost if a config had never run): `gap_factor=0` h and v
  0.19 and 0.16, `bands-y` 0.14, everything else under 0.09; all seed and
  annealing configs together about 0.03.
- `gap_factor=0` wins most strata, especially under 50k cells.
- Time caps: under 400k cells no job hits its cap (median uses 4 to 15%); 400k
  to 2M 11% capped, above 2M 21%.

## 2026-10-29 evening: rethink, and the new plan

Tonight's round 7 run is scrapped. New plan:
`plans/2026-10-30-decompose-and-search.md` (replaces the window polishing plan).

**Findings that forced the rethink:**

1. Every mining job builds a tour from one global setting and starts from
   nothing; the miner only keeps the shortest. The mining report shows this is
   used up: rounds 3 to 6 bought 0.4 points for 64 CPU hours, 14 of 55 configs
   never won, seeds and annealing together are worth about 0.03 points.
2. The diagnosis puts about half the lost length at lane ends and turns, which a
   single lane grid over a ragged region cannot reduce.
3. Exact local polishing (CP-SAT windows) finds the tours already optimal at
   small scale and is too slow at the scale of the waste. Exact global methods
   (A Closer Cut) stop at region / cutter area about 50; ours start at 416.
4. Mixed lanes, a crude per tile direction choice on a grid unrelated to the
   shape, still improved 12 of 31 benchmark instances by up to 2.5%. The freedom
   helps; the tiles were the wrong cells.

**What we do instead, and why:**

- Cells that follow the region's shape (boustrophedon decomposition,
  neighbouring cells merged when one sweep covers both), several sweep patterns
  per cell, and the pattern per cell and the order chosen together. This is the
  standard of practical coverage planning: Bähnemann et al. 2019 report 14%
  shorter paths than the best fixed direction, and 29% for shape following cells
  over trapezoidal ones; Huang 2001 shows optimal sweep decompositions choose
  the direction per cell. Expected for us (judgement): 3 to 8% shorter on ragged
  instances, about 1% on compact ones; 5% everywhere would be about +12
  pessimistic points.
- An adaptive large neighbourhood search over that structure (Ropke and Pisinger
  2006): swap a cell's pattern, merge or split cells, shift a boundary, reorder;
  operators picked by weights learned from their wins.
- Progress from run to run: each instance keeps its cells, patterns, order and
  operator statistics in `data/state/`, every run continues from there, and a
  report after each run moves time to the instances and operators still
  improving.
- Gates: step 2 (best pattern per cell, one shot, no search) must beat the
  stored bests on a third of the benchmark before the search is built; the
  search must reach about 1.5% shorter on average before any night run.

## Decompose plan, steps 1 and 2: cells pass, one pattern per cell fails gate 2

**Step 1, `solver/decompose.py`:** boustrophedon cells on the lane grid (lane
pieces chained where they neither split nor merge, cells under 3 pieces merged
into their widest neighbour). Benchmark: 7 to 187 cells per instance (raw chains
21 to 447), under a second for all 31. **Gate 1 passes.** Also seen: the regions
are only 23 to 183 lanes high, the cutters are large against them.

**Step 2, `segments-cells` (`solver/construct/cells.py`):** per cell, horizontal
and vertical lanes at up to 4 offsets from the cell's own edge, cheapest by
strip cost, then the usual order search. Every solution verified.

| config                                    | self score / 31 | bench pessimistic | better than stored best     |
| ----------------------------------------- | --------------- | ----------------- | --------------------------- |
| `segments-h/gap_factor=0` alone (earlier) | 30.13           | 10.68             |                             |
| `segments-cells/gap_factor=0`             | **28.38**       | 10.31             | 1 (srpg5461_22k_1l3, -1.6%) |

**Gate 2 fails.** Why, measured over the 31 instances: one pattern per cell has
4% fewer segments than one global grid (12,053 against 12,606), but 5% more lane
length (329,990 against 313,381), and 86 to 100% of cells chose horizontal
anyway. Lanes aligned to each cell's own edge overlap the neighbour's lanes at
every cell border, and that costs more than the lane ends it saves. Worst on X
(octa6025_952k_3x9 +17% lane length).

**What it says about the research:** on our grid with only horizontal and
vertical lanes, the global lane sweep already _is_ a boustrophedon decomposition
swept one way: every lane piece is a cell's lane. The 14% of Bähnemann et al.
came from free sweep angles in continuous polygons with obstacles, freedom we do
not have. What our setting adds is a direction per cell (mixed lanes: 0 to 2.5%)
and an offset per cell (costs overlap at borders). Choosing per cell on its own
does not work; any gain needs choices made jointly against the real cost, which
is the mixed lanes hill climb again.

## 2026-10-29 evening: would a Rust port help? Profile and order search time scaling

Question: is any part worth porting to Rust for speed. Two measurements, no new
solver code.

**Profile** (cProfile, `segments-h/gap_factor=0`, one job each):

| instance         | wall   | order search | prune  | everything else |
| ---------------- | ------ | ------------ | ------ | --------------- |
| fjs3649_17k_1x7  | 4.7 s  | 4.5 s        | 0.1 s  | < 0.1 s         |
| dkd1973_128k_2d8 | 4.1 s  | 3.2 s        | 0.9 s  | < 0.1 s         |
| iso331_400k_3p4  | 20.4 s | 3.6 s        | 16.4 s | 0.4 s           |

The order search always runs to its kick budget (it is time boxed, not slow).
Prune dominates on large instances.

**Time scaling** (benchmark, same config, order search kick budget 6 s versus 60
s, both with a 200 s job cap):

| kick budget | self score | pessimistic |
| ----------- | ---------- | ----------- |
| 6 s         | 29.84      | 10.65       |
| 60 s        | 29.86      | 10.61       |

Per instance, 60 s was shorter on 12, longer on 7, equal on 12, with swings of
up to 9% both ways (dkd1973_128k_2d8: 5454 at 6 s, 5928 at 60 s). Ten times more
order search buys nothing on average.

**What it means:**

- A Rust port of the order search would not shorten tours; it would only finish
  the same search sooner.
- A faster prune would only add throughput, and the mining report already shows
  more configs and seeds buy little.
- The order search minimises link length between segments, but the final length
  comes after prune and the split into k. Better orders by that proxy give final
  tours that are 5 to 9% better or worse at random. The link between order and
  final length, not search speed, is where this stage loses; worth remembering
  for any later move that scores by real length.
- No Rust for now. Five benchmark results were better than the stored bests and
  were promoted.

## 2026-10-30 night: where the order's length goes, and k closed tours from the start

**Measurement** (31 benchmark instances x 10 seeds, `segments-h/gap_factor=0`, 6
s kicks, every stage logged):

|                                                    | k = 1              | k = 2 or 3                        |
| -------------------------------------------------- | ------------------ | --------------------------------- |
| spread of the final length over 10 seeds           | 0.1 to 1.9%        | 0.5 to 8.7%                       |
| rank correlation, order score against final length | mostly 0.5 to 0.95 | -0.47 to 0.85, often near 0       |
| return trip of every tour, after the split         | 0.1 to 1.3%        | **3.3 to 13.1%**, median about 7% |
| prune saving                                       | 0 to 1.8%          | 0 to 1.7%                         |
| imbalance of the k tours after the split           | 0                  | 0 to 1.3%                         |

Best of 10 seeds over seed 0: median 0.6%. So the swings for k > 1 are the
return trips: the order search sees an open walk, the split then makes every
piece drive home. For k = 1 the order search optimizes the closed tour and the
problem does not exist.

**Fix: `groups-h`, `groups-v`** (`solver/construct/groups.py`): order all
segments as before, cut the order into k groups priced with their return trips
(binary search on the longest closed group), order every group again as a closed
tour, then move single segments out of the longest tour into the cheapest slot
of another while that shortens the longest, and polish again. `repair_tours`
repairs coverage over several tours.

| benchmark (31)              | self score | pessimistic | better than stored best  |
| --------------------------- | ---------- | ----------- | ------------------------ |
| segments-h gap 0, old split | 29.84      | 10.65       | -                        |
| **groups-h gap 0**          | **31.16**  | 11.11       | **17 of 31**, up to 7.7% |
| groups-v gap 0              | 30.35      | 10.80       | 9 of 31                  |

Gate passed (better on more than a third). Round 8 added: groups-h and groups-v
x gap 0, 1 x seed 0, 1 on every instance with k > 1 (1,320 jobs), first in the
run order.

**Bug found and fixed:** running both benchmarks at once let the slower one
overwrite 7 better bests with worse ones (each best store loaded the bests once
at start). All 7 were restored from verified candidates. The store now reads the
file on disk before promoting
(`fix: never let a stale best store overwrite a better best`).

**Overnight run** started 01:36, 7.5 h, 15 cores, memory permits 4.1 GB (only
5.2 GB free on the machine): round 8, then mixed lanes (round 7), then the
leftovers of rounds 5, 6, 3, 4. 4,904 jobs queued.

## 2026-10-30 morning: overnight run results

Run 01:36 to 09:16 (7.5 h budget, then running jobs finished), 15 cores, memory
permits 4.1 GB. 2,633 jobs ran, **0 failures**, 2,271 left unstarted (mostly
round 4, the lowest yield round). One Claude Code background watcher was stopped
by Windows memory pressure around 04:00; the miner itself kept running within
its permits.

| round                                | jobs        | cpu h | better | points     | points per cpu h |
| ------------------------------------ | ----------- | ----- | ------ | ---------- | ---------------- |
| 8 groups (new)                       | 1,320       | 27.9  | 287    | **2.04**   | **0.073**        |
| 7 mixed lanes                        | 1,330 total | 29.6  | 47     | 0.15       | 0.005            |
| 5, 6 seeds and annealing (leftovers) |             |       |        | 0.04, 0.02 | 0.002            |

- Round 8 paid 15 times more per cpu hour than any round since round 2. 123 of
  the 165 instances with k > 1 got shorter, median 1.3%, up to 4.3%.
- Pessimistic score **112.17** (about 110.8 without round 8; 110 two days ago).
- Group configs now hold the top 3 spots by points at stake, and win most strata
  above 50k cells in every cutter family. Below 50k cells plain segments still
  win (those are mostly k = 1).
- 21 of 63 configs never won anything (47.6 of 189.9 cpu hours): plain lanes,
  cover-tsp, gap 4 and 8 for h, most seed and annealing configs, most band x gap
  x shift crosses.
- Time caps: 15% of 400k-2M jobs and 26% of >2M jobs hit their cap, so the
  largest instances are time limited.

**What it means for the next steps:**

1. The return trip was real and is mostly removed. What is left for k > 1 is the
   same lane waste as for k = 1.
2. Rounds 4 to 7 can be dropped from the grid; their hours go to group variants
   (shift, offset, more seeds, longer caps on the largest instances).
3. Next ideas, unchanged and now judged by final length: narrow lanes (one
   setting), the per cutter diagonal measurement, outline passes in thin arms.

## 2026-10-30 afternoon: narrow lanes, rejected

**Idea:** space lanes closer than the cutter height and let each lane answer
only for the widest cutter rows that fit (`pitch` in `lane_segments`,
`widest_rows` picks the window). For a diamond or a plus the narrow tips stop
stretching every lane out to the border, at the price of more lanes.

**Benchmark** (31 instances, groups-h gap 0, run one after another so only one
run writes the store):

| pitch                | self score | pessimistic | better than best |
| -------------------- | ---------- | ----------- | ---------------- |
| 1.0 (baseline rerun) | 30.46      | 11.11       | 2 of 31          |
| 0.75                 | 20.84      | 7.54        | 0                |
| 0.5                  | 13.47      | 4.81        | 0                |

Worse on every instance and every cutter family, diamonds and plus included.
Pitch 0.75 makes tours about 25 to 40% longer, which is just the extra lanes:
the shorter lane ends save almost nothing. **The tour length is set by the lane
count, not by the lane ends.** Gate failed; the knob stays off the mining grid.

What it means: the remaining waste is not at the lane ends. Better ideas cut the
lane count or the overlap between lanes, not the ends: the diagonal measurement
per cutter (a direction where the swept width is larger than the height) and
lane spacing set from the swept width instead of the bounding box height.

## 2026-10-30 evening: window polishing brought back, a small steady gain

Branch `lns` merged (window map and CP-SAT window solver). New
`solver/polish.py`: the longest tour is tiled into 20 x 20 anchor windows at a
random offset, windows are re-solved in random order (CP-SAT, one worker, 3 s
each), every shorter walk is spliced in, the tiles follow when another tour
becomes the longest. Each polished solution goes through the official verifier
before the best store sees it.

**Benchmark:** the 31 benchmark bests, 5 minutes each, 15 at a time (15 min
wall).

|                           | value                                                            |
| ------------------------- | ---------------------------------------------------------------- |
| instances improved        | 25 of 31, all verified and promoted                              |
| median gain               | about 0.2%, best 1.9% (fjs3649_17k_1x7), X and s5 instances 0.9% |
| windows solved / improved | 6,294 / 112 (1.8%)                                               |
| pessimistic score         | 11.32 to 11.35 (+0.031)                                          |
| points per cpu hour       | **0.012**                                                        |

What it says:

1. Unlike the pilot (two windows per instance, median 0), a pass over all
   windows finds something on almost every instance: the waste the pilot missed
   is spread thin, as the window map said.
2. The rate, 0.012 points per cpu hour, beats repeating mining rounds (0.002 to
   0.005) and loses to a real construction change (groups, 0.073). It is worth
   running on all instances as a background loop, not worth more engineering at
   the step scale.
3. Most windows hit the 3 s limit, so the solver is time bound; whole passes
   over the bigger instances did not finish in 5 minutes.

Next: a ruin and recreate search at the scale of lane segments (remove the
segments of an area, re-cover it with another direction or offset, reconnect,
keep if the final length drops).

## 2026-10-31 morning: round 9 mining, then polish on every instance

**Run:** `solver.mine --rounds 9` (00:52 to 05:22), then `solver.polish --all`
(05:22 to 09:33), 15 cores, no failures, no retries needed. Round 9 is the group
variants on k > 1: seeds 2 and 3, shifts 0.25 and 0.5, gap 0.5, each for
groups-h and groups-v, with a doubled cap above 400k cells. Polish gives each
instance 10, 20 or 30 minutes by size.

| part                  | jobs  | cpu h | better | points | points per cpu h |
| --------------------- | ----- | ----- | ------ | ------ | ---------------- |
| round 9 mining        | 1,650 | 57.4  | 109    | 0.51   | 0.009            |
| polish, all instances | 250   | 59.9  | 198    | 0.46   | 0.008            |
| night total           | 1,900 | 117.3 |        | 0.97   | 0.008            |

For comparison: round 8 (the first group run) 0.073, the polish benchmark 0.012,
repeated rounds 0.002 to 0.005.

|                               | value                                             |
| ----------------------------- | ------------------------------------------------- |
| pessimistic score, all 250    | 113.10 (112.20 in the last committed leaderboard) |
| instances with a shorter best | 216 of 250 (83 of them k = 1, all from polish)    |
| median gain on those          | 0.36%, best 3.5% (srpg41087_555k_3s5)             |
| median gap to the lower bound | 45.9%                                             |

What it says:

1. Both parts paid about the same, under a point each, at a rate close to
   repeated mining. More seeds and shifts of the same construction are now in
   the noise; round 8's gain came from the new construction, not from the grid
   around it.
2. Polish improves almost every instance a little, as in the benchmark. The rate
   dropped from 0.012 to 0.008 because the long caps on big instances give fewer
   windows per point.
3. The median gap of 46% is untouched by a night of 117 cpu hours. Either the
   tours or the lower bound are far off, and the next step is to find out which:
   the witness lower bound first, then a construction that changes the lane
   count (pass cover or witness loop) if the gap is real.

## 2026-11-12 to 11-13: the solver ported to Rust

Branch `rust-port`: the Python solver replaced by a Cargo workspace (domain,
plugins, adapters, app; see `README.md`). Every ported piece was checked against
fixtures exported from Python before it was timed (`docs/python-vs-rust.md`).

| piece      | matches Python                              | Rust is                    |
| ---------- | ------------------------------------------- | -------------------------- |
| load       | 250 / 250 instances, same cells             | 8x faster, 55x less memory |
| coverage   | 248 / 248 uncovered counts, as the verifier | 280x                       |
| edit check | equals a full recheck                       | 2.25 / s -> 78,000 / s     |
| order      | shorter on 7 / 7, same budget               | 110x to 3,300x             |

End to end, Python's own grid (`mow mine`, kicks at a quarter of Python's time)
then `mow polish --all`: 85.3 cpu h against Python's 307.1, beats Python's
stored bests on 168 of 250 (geomean 0.9977). Score **113.57**. First submission
zip 113.85.

What it says: the port alone buys time, not points. Two implementations of the
same construction land within 0.2% of each other, so the points have to come
from searches the 78,000 edit checks per second now allow.

## 2026-11-13: five optimization tracks in parallel

Plan `plans/2026-11-13-optimize.md`. Each track in its own worktree and data
folder, gated on the bench: better than the stored best on at least 11 of 31,
and at least 0.05 points per cpu hour.

| track | idea                                                                          | gate                                                                          | kept                         |
| ----- | ----------------------------------------------------------------------------- | ----------------------------------------------------------------------------- | ---------------------------- |
| A     | drop rounds 3 to 7 and 9 (under 0.03 pts/cpu h); polish only below 400k cells | -                                                                             | yes                          |
| B     | tighter lower bounds: orthogonal hull bound (proved), witness bound           | X room is 27.1 points, not 37.1; the witness bound never beats the hull bound | yes, as catalog columns      |
| C     | `mow turns`: slide, shortcut, 2-opt, or-opt on the corners of finished tours  | 31 / 31, 3.15 pts/cpu h                                                       | yes                          |
| D     | lane patterns: hug the region's edges, join lanes across gaps, spirals        | hug 5 / 31 at 0.2 pts/cpu h; join and spiral fail                             | hug only, as mining round 10 |
| E     | `mow lns`: ruin and recreate with adaptive operator weights                   | 31 / 31, 4.50 pts/cpu h at 60 s                                               | yes                          |
| F     | split the region into k parts before touring (k > 1)                          | 5 / 21, 0.025 pts/cpu h                                                       | no                           |

- **Diagonal lanes dropped without building:** the hull bound proves no walk
  sweeps faster than straight lanes.
- **Join fails** because joining only removes freedom from the order; **spirals
  fail** because ring corners pay the same dead steps as lane ends.
- **Split first** beats the group constructions head to head (self score 7.40 vs
  7.28, mostly k = 3) but rarely beats a best that has already been searched.
- Of the points left against the area bound, the room is concentrated in sparse
  regions (fill under 0.35, median objective / bound 1.64) and in k = 3.

## 2026-11-13 night: rerun with what passed

Seeded with the tracks' bests, then round 10, `mow turns`, and two `mow lns`
passes on all 250. 0 failures; every best re-checked by the official verifier.

| step                                   | cpu h   | points    | points per cpu h |
| -------------------------------------- | ------- | --------- | ---------------- |
| bests taken over from the track runs   | about 7 | +5.51     |                  |
| round 10, hugged lanes                 | 0.65    | +0.03     | 0.05             |
| `mow turns --all --seconds 30`         | 0.95    | +0.42     | 0.44             |
| `mow lns --all --seconds 60`           | 4.17    | **+9.53** | **2.28**         |
| `mow lns --all --pass 2 --seconds 300` | 20.84   | +3.24     | 0.16             |
| for comparison: polish, all 250        | 59.9    | +0.39     | 0.007            |

Score 113.57 -> **132.29**. Every best is shorter than Python's stored best (250
/ 0 / 0, geomean 0.918; k = 1 0.894, k = 2 0.914, k = 3 0.948). Zip
`data/rust/submission-2026-11-13-rerun.zip`.

What it says:

1. Ruin and recreate is the lever: its first 60 s paid 300 times more per cpu
   hour than polish.
2. The 300 s pass still paid 20 times polish, and the largest instances were
   still improving when it stopped.
3. Next: more lns time where the last pass still improved, a kick loop in
   `mow turns`, lns from fresh constructions instead of only from the bests.

## 2026-11-14 night: strategy tournament on 15 instances

Instead of mining all 250, one instance per cutter family and k (median cells in
50k to 400k) got as much search as one night allows. Four arms, 4 threads each,
4 passes of 600 s (10 cpu h per arm); the leader then ran 30-minute passes on 15
threads until 07:45. Driver and scripts in `data/rust-arms/` (git-ignored).

| arm | strategy                                                    | points at 10 cpu h | better on |
| --- | ----------------------------------------------------------- | ------------------ | --------- |
| A   | `mow lns`, 600 s passes, from the best                      | +0.145             | 15 / 15   |
| B   | same, temperature 0.003 (ten times the default)             | +0.130             | 10 / 15   |
| C   | `mow lns` from the best candidate of the other construction | **+0.169**         | 11 / 15   |
| D   | `mow lns` 600 s then `mow turns` 60 s, alternating          | +0.167             | 15 / 15   |

C won and ran 8 more passes (60 cpu h): +0.107 more, 0.0018 points per cpu h,
each pass paying less than the last. Merged the shortest of all arms: sample
7.53 -> 7.84, score 132.29 -> **132.60** (+0.31 for about 100 cpu h). All 250
pass the official verifier. Zip `data/rust/submission-2026-11-14-arms.zip`.

What it says:

1. C's seeds started 7 to 23% longer than the bests and still passed them on 11
   of 15 within 10 cpu h: the bests sit in local optima that more lns time on
   the same tour does not leave.
2. Fresh restarts led on 9 of 10 k = 1 and 2 instances; continuing from the best
   (A, D) led on 4 of 5 k = 3. Cutter family showed no pattern. One instance per
   group, so this is a lead, not a result.
3. A hotter walk (B) is worse: it stalled on 5 of 15.
4. Depth is the wrong spend now: 0.017 points per cpu h at best here, against
   0.16 for the 300 s pass over all 250. Next: lns from the other construction's
   candidate on every k = 1 and 2 instance, 10 minutes each.

## 2026-11-14 morning: restarts on every k = 1 and 2 instance

The tournament's lead, tried on all of them: every k = 1 and 2 instance with a
candidate from the other construction (149; 9 have none) got 600 s of `mow lns`
from that candidate instead of from the best, 15 threads, one pass (25 cpu h).
Seeds started a median 12.3% longer than the bests. Driver and scripts in
`data/rust-restart/` (git-ignored).

The restart beat the best on **92 of 149**, +0.967 points, score 132.60 ->
**133.57**: 0.039 points per cpu h, twice the depth runs of the tournament but
below the 0.16 of a first 300 s pass over all 250. The ones it lost run larger
(median 102k cells against 32k for the ones it won), where 600 s is too short to
close a 12% gap. All 250 pass the official verifier. Zip
`data/rust/submission-2026-11-14-restart.zip`.

## 2026-11-14 noon: an lns move aimed at the lawn's edge

`mow diagnose` on the bench bests: 19.6% of tour length mows past the edge,
12.6% at lane ends. A new lns destroy, `edge-3`, samples 64 stops and tears out
a 3 x 3 window around the one whose cutter hangs furthest off the lawn (stops
wholly off it are left to `dead-3`); the existing recreates rebuild it.
`VisitCounts::outside_cells` counts a stop's cells off the region. A/B on the 31
bench instances from the same bests and state, same seed, 300 s, 7 threads each
(6 cpu h together). Scripts in `data/rust-edge/` (git-ignored).

| arm               | points  | better than start | shorter head to head |
| ----------------- | ------- | ----------------- | -------------------- |
| base (7 destroys) | +0.0404 | 21 / 31           | 10                   |
| edge (8 destroys) | +0.0454 | 19 / 31           | 9 (12 tied)          |

A wash. The adaptive weights ranked edge-3 a median 7th of 8 and in the top two
on 1 of 31: the search found it rarely pays. Edge did better on the large k = 3
X and diamond instances (octa28351_946k, isoa22945_987k, isoa4725_400k), but 31
instances over 300 s cannot tell that from noise. Not rolled out. The shorter of
both arms merged: score **133.62**, zip
`data/rust/submission-2026-11-14-edge.zip`, all 250 verified.

Edge overshoot is mostly at lane ends, set by where lanes start and stop, and a
local window rebuilt with the same lanes puts the ends back where they were.
Shortening it likely needs lane ends chosen per lane in the construction, not
another lns move.

## 2026-11-14 afternoon: a faster lns

Profiling `mow lns` showed that moves per second fall with the cutter's size,
not the lawn's: about 28k/s on a 14k-cell instance, 13/s on a 987k one with a 60
x 60 X cutter (1872 cells). Two causes:

- **Repair anchors.** `anchors_for` stood each missed cell at the anchor nearest
  the tour, which puts the cell on the cutter's far edge and the rest of the
  footprint back over swept ground: about 14 new cells per anchor and 137
  anchors per move, each an O(m) cycle insert. It now picks, among the nearest
  anchor and an even spread of 32 of the cutter's offsets, the one sweeping the
  most cells still missed, the nearest among ties.
- **Bared cells.** After a removal the move rescanned every cell under the
  removed footprints. `VisitCounts::remove_positions_baring` now returns the
  cells whose cover reaches 0 while removing, and `unswept_around` marks a local
  bitmap instead of sorting duplicates.

A/B on the 31 bench instances from the same bests, same seed, 300 s, 7 threads
each (about 3 cpu h per arm). Scripts in `data/rust-speed/` (git-ignored).

| binary   | points  | better than start | shorter head to head |
| -------- | ------- | ----------------- | -------------------- |
| old lns  | +0.0214 | 16 / 31           | 4                    |
| fast lns | +0.1047 | 20 / 31           | 19 (8 tied)          |

Median 2.6x the moves, 10 to 11x on the 950k-cell X and diamond instances, where
most of the gain came from (octa28351_946k 12988 -> 12800, octa6025_952k 11242
-> 11018). Shorter of both merged: score 133.73.

Then one 300 s pass of the fast binary over all 250 bests, 15 threads, 85.5 min
(21 cpu h): **187 new bests**, score 133.73 -> **136.34**, +2.61 points, 0.12
points per cpu h, the best rate since the first lns pass. All 250 pass the
official verifier. Zip `data/rust/submission-2026-11-14-speed.zip`.

## 2026-11-15: stamping runs of anchors at once

Profiling the faster `lns` again on 7 large instances: stamping cutter
footprints into the visit counts, one position at a time, took most of a move.
It happens on every removal, on each repair round's apply and unapply, and on
every undo. Three changes, the tours chosen unchanged:

- **Runs stamped whole** (`mow-coverage` `runs.rs`). Neighbouring positions on a
  lane share almost all of their footprint. A straight run along a row is
  stamped one cutter row at a time (a column run, one cutter column), each cell
  getting the count of the run's copies over it: about `n + r` per strip instead
  of `n` times the cutter's area.
- **Repair checks only the bared cells.** Adding walks never uncovers a cell, so
  after a removal only the cells it bared can still be unmowed; the repair loop
  no longer walks the removed footprints again, up to four times a move.
- **Repair anchors counted with prefix sums** (`mow-lns` `anchors.rs`). A
  candidate's count of missed cells is one lookup per cutter row strip, not a
  pass over up to 1872 cells, for each of 33 candidates.

Moves in 60 s on one thread, before and after:

| instance            | before | after | x   |
| ------------------- | ------ | ----- | --- |
| isoa22945_987k_3x10 | 16k    | 80k   | 5.0 |
| octa6025_952k_3x9   | 26k    | 115k  | 4.4 |
| smo64863_393k_3p6   | 72k    | 283k  | 3.9 |
| iso5009_399k_3x5    | 93k    | 328k  | 3.5 |
| srpg22373_399k_3d4  | 161k   | 460k  | 2.9 |
| dkd1973_128k_2d8    | 340k   | 733k  | 2.2 |
| iso8005_105k_2s3    | 1.19M  | 1.32M | 1.1 |

The 3 x 3 cutters barely gain: there cutting the dense tour (38% of a move) and
writing it back (13%) cost most, both linear in the tour's length.

A/B on the 31 bench instances from the same bests, same seed, 300 s, 7 threads
each. Scripts in `data/rust-runs/` (git-ignored).

| binary          | points  | better than start | shorter head to head |
| --------------- | ------- | ----------------- | -------------------- |
| committed lns   | +0.0243 | 13 / 31           | 5                    |
| run-stamped lns | +0.0550 | 16 / 31           | 15 (11 tied)         |

Moves a median 1.58x the committed binary's, up to 4.5x (isoa22945_987k 80k ->
357k). Shorter of both merged: score 136.40.

Then `lns` pass 4 over all 250 bests with it, 300 s each, 12 threads, started
15:17, 105.6 min (21 cpu h, the machine shared with the tracks below): **156 new
bests**, score 136.40 -> **137.66**, +1.26 points, 0.06 points per cpu h, half
pass 3's rate as the bests get harder to shorten. All 250 pass the official
verifier. Zip `data/rust/submission-2026-11-15-runs.zip`.

## 2026-11-15 evening: three tracks in parallel

Three workers, each in its own git worktree, on the ideas from the waste
breakdown of all 250 bests (productive 75.4%, overshoot past the edge 11.1% of
which 7.0% at lane ends, re-mowing 6.8%, dead steps 6.7%).

**B: what splitting the lawn between cutters costs.** `mow diagnose` learned to
split re-mowing into a tour's own cells and another tour's, and to classify each
lane end by the step not taken: at the lawn edge, inside the lawn, or at the
border with another cutter's area. Cross-tour overlap plus border lane ends are
0.2% of length at k = 2 and 0.9% at k = 3: 1.2 points over all 165 k > 1
instances if removed entirely, no instance above 0.03. Not worth a construction
change. Lane ends inside the lawn (turns, shifts between lanes) cost 9.4% at k =
1, 9.5% at k = 2, 13.3% at k = 3, 24-27% for X cutters. Output in
`data/rust-measure/`.

**C: lane ends chosen jointly.** A dynamic program over neighbouring lanes picks
shared turn columns, crediting the turn's sideways leg with what it sweeps
(`ends=1`). On the 31 bench constructions, twice each: tours 0.3-0.4% longer (10
better, 21 worse), lane-end overshoot 10.1% -> 9.8%. A probe explains it:
`turns`' slide move, which pulls every U turn inward exactly, cut the raw
constructions by 3.05% but lane-end overshoot only to 9.9%. Lanes one cutter
apart sweep separate strips, and a turn's cost is set by the far end of a lane's
outer row, which no sideways move reaches. Edge overshoot needs lanes along the
edge (laps, mixed directions), not other ends. Not merged. Output in
`data/rust-ends/`.

**A: search time where the points are.** `mow lns --allocate weighted` gives
each instance seconds in proportion to points x its recent relative gain per
second (from the job log, the last two passes, a floor for stalled instances),
clamped to [S/5, 5S], same total, longest jobs first. History does predict: the
gain of passes 1 to 3 ranks the next pass's gain at Spearman 0.53. A/B on the 31
bench instances after pass 4, same total CPU, 300 s mean, 7 threads each:

| arm      | points  | better than start | points per cpu h | wall     |
| -------- | ------- | ----------------- | ---------------- | -------- |
| even     | +0.0154 | 13 / 31           | 0.0060           | 25.0 min |
| weighted | +0.0159 | 10 / 31           | 0.0061           | 22.7 min |

A tie within noise: the instances given the most time gained more (0.013 against
0.003 points for the top nine), the ones cut lost about as much. Both arms show
how little a fifth pass on the same tours now pays: 0.006 points per cpu h, a
tenth of pass 4. Output in `data/rust-alloc/`.

## 2026-11-15: menus

`mow` with no arguments now opens menus (`mow-tui`, built on `ratatui`), moved
through with the arrow keys and Enter, no commands typed:

- **Dashboard**: the score, by k and by cutter family, median gaps, the latest
  new best.
- **Instances**: all 250, sorted and filtered; each one's facts, its best tour
  drawn in the terminal in braille, its candidates and job history, and a
  browser page of the tours (pan, zoom, one checkbox per cutter, a candidate
  side by side with the best) written to `data/rust/view/`.
- **Runs**: start a pass of `lns`, `turns` or `polish` from a form, watch its
  progress and log, stop it. A run is a folder `data/rust/runs/<id>/` kept going
  by a detached `mow __supervise`, so it outlives the menus, and it runs a copy
  of the binary, so rebuilding never disturbs it.
- **Experiments**: binaries kept in `data/rust/bin/` (the running build, or a
  commit built in a worktree with half the cores), A/B runs between two of them,
  a live report, and merging the winners.
- **Bests**: merge the shorter tours of any `best/` folder, each merge backed up
  in `data/rust/backups/` and undoable.
- **Submission**: pack a verified zip with `tools/pack/pack.py`, the old
  throwaway script made a committed tool. Never uploads.

This replaces the per-experiment `setup.py`, `run.sh`, `compare.py`, `merge.py`
and `pack.py` copied into each `data/rust-*` folder. Two end-to-end tests
(ignored by default: they need the release binary and Python) run a real pass
through the supervisor and stop another, and build HEAD in a worktree and run an
experiment against it.

## 2026-11-15 night: a second restart round

`lns` 600 s on the 147 k = 1 and 2 instances, each from the construction its
best did not grow from (round 1's winners from `same_orientation`, the rest from
`fresh_start`), 15 threads. Shorter on 49 of 147: +0.254 points, 137.66 ->
137.914, 0.010 points per cpu h. The wins are 0.3-0.7% on mid sized instances;
on the largest (`sun0100000_1000k`, `usnight0060000_3644k`) 600 s from scratch
ends 1-1.5% behind the best. Output in `data/rust-restart2/`, zip
`data/rust/submission-2026-11-15-restart2.zip`.

## 2026-11-16 overnight: lns pass 5, time weighted

`mow lns --all --pass 5 --allocate weighted --seconds 1900 --hours 9.5` on all
250, 15 threads: 132 cpu h shared from 380 s to 7778 s by the history of passes
1 to 4, the large k = 3 instances first. 177 new bests, 137.914 -> 139.523
(+1.61), 0.012 points per cpu h: twice the 0.006 the bench A/B gave a fifth
pass, since the bench holds none of the large instances that got the long slots.
The biggest gains: `smr12511_915k_3p9` 9048 -> 8560 (5.4%), `iso14835_5525k_3d5`
99034 -> 93864 (5.2%), `smr16111_876k_1d10` 110666 -> 106542 (3.7%),
`octa14582_7203k_3s4` 128062 -> 124160 (3.0%). Output in `data/rust-night/`, zip
`data/rust/submission-2026-11-16-night.zip`.

**The walk drifts on large instances.** `lns` continues from the walk the last
run ended on while the stored best is unchanged. On 40 instances that walk
started above the best: 17 of the 25 within 0.36% got under their best, 0 of the
14 at 0.46% or more did, and on the largest the gap was 2-48%
(`iso2071_2273k_2l3` walk 301560 against best 203648). The temperature is 0.0003
of the whole objective per move, so on a long tour many small worsening moves
pass, run after run. `resume` now starts from the best once the saved walk ends
more than 0.5% above it (`DRIFT`). The temperature itself is left for an A/B:
scaled to the size of a move rather than the tour, it would stop the drift
within a run as well.

## 2026-11-16: lns pass 6, and the temperature capped in cutter spans

**Pass 6**, the same as pass 5 with the drift fix: 129 new bests, 139.523 ->
140.049 (+0.53), 0.004 points per cpu h, a third of pass 5. The 14 instances
whose walk had drifted now started from their best and gained nothing: the walk
climbs away again within a run. Zip `data/rust/submission-2026-11-16-pass6.zip`,
output in `data/rust-pass6/`.

**The start temperature in spans.** 0.0003 of the objective is 0.04 cutter spans
on the shortest tours, 0.18 at the median, 0.35 at p75 and up to 11 on the
longest, while a move changes a few spans of tour whatever the tour's length.
`mow lns --temperature-cap SPANS` holds the start temperature to that many spans
(`LnsOptions::temperature_cap`); below the cap nothing changes. A/B on the 39
instances starting above 0.6 spans, 900 s each, seed 7, 7 threads per arm at
once, both from copies of the bests with no saved state:

| arm     | points  | better than start | points per cpu h |
| ------- | ------- | ----------------- | ---------------- |
| no cap  | +0.0111 | 7 / 39            | 0.001            |
| cap 0.3 | +0.5776 | 25 / 39           | 0.055            |

The capped arm is shorter than the other on 23 instances and longer on 4 (all
under 0.15%). The stuck large instances move again: `smo64863_1931k_2s3` 6.8%,
`isoa22945_6183k_3l3` 4.6%, `iso252735_2982k_2l4` 4.0%, `octa20637_1732k_2l5`
3.5%, `iso5009_524k_1d4` 3.4%, `iso2071_2273k_2l3` 1.2%, which no pass had
moved. Eight large instances moved in neither arm. Output in `data/rust-cap/`.
