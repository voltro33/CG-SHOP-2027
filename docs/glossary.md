# Glossary

Terms as the code and docs use them. Where a term names something in code, the file is given.

**Anneal (knob).** Starting temperature of the simulated annealing order search, as a fraction of the order length; 0 means plain ILS. (`solver/construct/ordering.py`)

**Anchor / position.** A lattice point where the cutter's center stands. Tours are lists of anchors. (`solver/model.py`)

**Anytime algorithm.** One that can be stopped at any moment and still return its best answer so far. Every
miner job has a deadline and returns what it has. (`solver/mine.py`)

**Area bound.** The lower bound `(cells - k*|C|) / (k * gain)`, see `docs/concepts/lower-bounds.md`. (`solver/bounds.py`)

**Band.** (1) The rows one lane sweeps, as many as the cutter is tall. (2) In partitioning, the slab of the region
given to one cutter. (`solver/partition.py`)

**Baseline.** In the leaderboard, the best result from mining rounds 0 and 1 (default segments, plain lanes,
cover-tsp, band partitions): the Phase 4 starting point that later rounds are measured against. (`solver/jobs.py`)

**Boustrophedon.** Back-and-forth strip sweeping, like plowing a field ("as the ox turns"). (`solver/construct/lanes.py`)

**Candidate.** A verified solution kept in `data/solutions/candidates/` because it came within 5% of the best.

**Catalog.** One row of features per instance in `data/catalog.csv`. (`solver/catalog.py`)

**Code version.** A number in every job id (`|v2`); bumping it makes the miner rerun old configs with new code. (`solver/jobs.py`)

**Config.** Everything that decides how one solve runs: method, params, partition. Recorded with every solution. (`SolveConfig` in `solver/pipeline.py`)

**Constructor.** A function that builds one open walk over the region; the rest of the pipeline repairs,
splits and polishes it. (`solver/construct/`)

**Corner.** A point where a tour changes direction; the solution file stores only corners.

**Cover-TSP.** A constructor: pick stamp spots by greedy set cover, then visit them in a short order. (`solver/construct/cover_tsp.py`)

**Coverage / sweep.** The set of cells the cutters pass over: all visited anchors, Minkowski-summed with the
cutter. See `docs/concepts/coverage.md`.

**Cutter.** The small polyomino that mows; its center traces the tour. Five families: square, diamond,
clipped square, plus, X.

**Densify / compress.** Turn corners into every unit-step position, and back. (`solver/tours.py`)

**Descent.** Local search that only ever accepts improving moves, until none is left.

**Double bridge.** A kick that swaps two neighbouring blocks of a sequence. No single 2-opt or Or-opt move
undoes it, which is what makes it a good kick. (`solver/construct/ordering.py`)

**Footprint / stamp.** The cells covered by the cutter standing at one anchor.

**Gain (step gain).** The most cells one unit step can newly sweep. (`Problem.step_gain`)

**Gap.** `(ours - bound) / bound`: how far a solution is above the lower bound. An upper limit on the room
left to improve, since bounds are loose.

**Gap factor.** How many lane heights of empty space a lane segment may bridge instead of splitting. (`segments.py`)

**Heuristic / local search.** Improving a solution by small targeted changes instead of trying every possibility.

**Iterated local search (ILS).** Descent, then a random kick, then descent again; keep the result if it is better,
else go back. Repeats until time is up. (`solver/construct/ordering.py`)

**Job.** One `(instance, config, seed)` run in the miner, with an id like
`fpg73_1k_1s2|segments-h/gap_factor=4|s0|v2`. (`solver/jobs.py`)

**Kick.** A random change made on purpose to escape a local optimum.

**Lane.** One straight horizontal (or vertical) pass of the cutter. See *strip*.

**Lane segment.** The part of a lane that actually has cells to mow; lanes are cut into segments at gaps. (`solver/construct/segments.py`)

**Leaderboard.** `data/leaderboard.csv`: best, baseline, bound and gap per instance. (`solver/report.py`)

**Link.** The drive from the end of one lane segment to the start of the next: pure overhead.

**Local optimum.** A solution no single move can improve, though better ones may exist.

**Lower bound (floor).** A proven value no solution can beat. (`solver/bounds.py`)

**Makespan / max tour length.** The length of the longest tour; the contest objective.

**Minkowski sum (⊕).** `A ⊕ B = {a + b}`: stamping shape `B` at every point of `A`.

**Mining.** Running many independent jobs in parallel and keeping the best verified result per instance. (`solver/mine.py`)

**Neighbour list.** For each segment, the few segments with the closest end points. Moves only consider these, which keeps search fast.

**NP-hard.** At least as hard as the hardest problems in NP; no known method solves every instance exactly in reasonable time. See `docs/concepts/np-hardness.md`.

**Or-opt.** A move that takes one element out of a sequence and reinserts it elsewhere, here also choosing its driving direction.

**Partition.** An assignment of region cells to cutters. (`solver/partition.py`)

**Polyomino.** A shape made of unit grid squares joined edge to edge; may have holes.

**Prune.** The corner-cutting local search: replace a corner by a shortcut while every cell stays swept. (`solver/improve.py`)

**Rebalancing.** Shifting work from the longest tour to shorter ones. (`_rebalanced` in `solver/partition.py`)

**Repair.** Adding small out-and-back detours for any cell a walk misses. (`solver/repair.py`)

**Round.** A group of miner jobs; every instance finishes one round before the next starts, in the order of `ROUND_ORDER`. (`solver/jobs.py`)

**Seed.** The random seed of the order search's kicks; different seeds give different tours, and the miner keeps the best. (`segments.py`)

**Score.** Per instance, about `(best of all teams / ours)^2`, summed. The square means a 10% shorter tour is worth about 21% more.

**Shift.** Moving all lanes by a fraction of a lane height, which changes where band borders fall. (`segments.py`)

**Simulated annealing.** Local search that sometimes accepts worse moves early to escape dead ends, less often as it "cools". Used for the segment order. (`ordering.py`)

**Split.** Cutting one walk into `k` consecutive pieces with the smallest possible longest closed piece. (`solver/split.py`)

**Suspect.** A miner job caught in a broken worker pool; it reruns alone so a crash can be pinned on the right job. (`solver/mine.py`)

**Strip.** One straight pass, as wide as the cutter. Same as *lane*.

**Tour.** A closed sequence of axis-parallel moves for one cutter's center.

**Turn cost.** Length spent changing lanes, which covers little new ground.

**2-opt.** A move that reverses a block of a sequence, changing only the two links at its ends.

**Verifier.** The official `cgshop2027_pyutils.verify`; the source of truth for feasibility.

**VisitCounts.** How many times any tour stands at each anchor, so prune can check coverage locally. (`solver/improve.py`)

**Walk.** An open sequence of axis-parallel corners; becomes tours after splitting and closing.

**Windowed check.** Checking coverage with whole masks in a window instead of point by point, to keep memory flat for huge cutters. (`_all_swept_windowed`)

**Witness.** A checkpoint cell every tour must reach; the shortest route through witnesses gives a lower bound. See `docs/concepts/witness-bounds.md`.
