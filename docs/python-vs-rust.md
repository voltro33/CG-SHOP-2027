# Python vs Rust

The Python solver (branch `init-solver`) against the Rust port (branch
`rust-port`), piece by piece. Every Rust piece was first checked to give the
same answers as Python, and only then timed.

---

## In short

- **Same answers.** Every ported piece matches Python exactly on real instances:
  cells, coverage verdicts, segment lists. The ordering search isn't meant to
  match move for move, and it finds shorter orders instead.
- **The big change is coverage.** Asking "does this edit to a tour still mow
  every cell?" went from 2 a second to about 78,000. That check sits inside any
  search that edits tours, so those searches become possible.
- **Memory drops about 55 times.** Loading the largest lawn takes 5.8 MiB
  instead of 318 MiB, so many more jobs fit on one machine.
- **Ordering alone won't move the score.** Rust orders 100 to 3,000 times
  faster, but real instances have at most about 2,400 segments, where Python's
  search has already settled.
- **Searches Python couldn't afford now pay.** The fast edit check made two new
  searches on finished tours possible: a ruin and recreate search (`mow lns`)
  and a corner search (`mow turns`). With them every Rust best is shorter than
  Python's stored best, 8% on the geometric mean, and the score went from 113.6
  to 132.3 of 250 for 112 CPU hours against Python's 307. Every best passed the
  official verifier. Full table: [rust-results.md](rust-results.md).
- **Depth on a few instances pays little now.** An overnight tournament of four
  search strategies on 15 instances (one per cutter family and k) added 0.31
  points for 100 CPU hours, score 132.6. Restarting `mow lns` from a different
  construction beat continuing from the best on k = 1 and 2.

| Piece      | Matches Python                          | Python         | Rust              | Rust is      |
| ---------- | --------------------------------------- | -------------- | ----------------- | ------------ |
| Load       | 250 / 250 instances, same cells         | 96 ms, 318 MiB | 12 ms, 5.8 MiB    | 8x, 55x less |
| Coverage   | 248 / 248 uncovered counts, as verifier | 4,410 ms       | 15.7 ms           | 280x         |
| Edit check | equals a full recheck on random edits   | 2.25 / s       | 78,000 / s        | ~35,000x     |
| Order      | shorter on 7 / 7, same 3 s budget       | see table      | 2% to 23% shorter | 110x–3,300x  |
| Segments   | 372 / 372 lists identical               | 50.2 ms        | 5.3 ms            | 9.5x         |
| Pictures   | all 250 bests render                    | not timed      | 57 ms, 22 KB SVG  | –            |

---

## How it was measured

|         |                                                      |
| ------- | ---------------------------------------------------- |
| Machine | AMD Ryzen 7 9800X3D (8 cores, 16 threads), 31 GB RAM |
| System  | Windows 11                                           |
| Python  | 3.10.11, numpy, the `init-solver` solver             |
| Rust    | 1.89.0, `--release`                                  |
| Date    | 2026-11-12                                           |

Python was timed at each piece's check, on this machine, on `init-solver`. Rust
was timed again after the move to the new layout, and every number matched the
first run within a few percent. Each time is one process doing one thing, with
nothing else running.

The largest lawn, used for most timings, is `euronight0070000_8749k_3s12`:
8,748,612 cells, a 5,184-cell cutter, k = 3.

---

## 1. Reading an instance

From the instance file to a ready problem: parse, rasterize the region with its
holes, centre the cutter.

|        | Time  | Peak memory                                 |
| ------ | ----- | ------------------------------------------- |
| Python | 96 ms | 318 MiB for the process (47 MiB is imports) |
| Rust   | 12 ms | 5.8 MiB for the whole process               |

Python keeps a cell set as numpy arrays, 26 to 40 bytes per cell of the bounding
box. Rust keeps one bit per cell. That is where the memory goes, and it's the
same for every piece below that holds a cell set.

**Checked:** all 250 challenge instances give the same cells, cutter and `k`.
All 250 stored bests read and write back byte for byte.

---

## 2. Coverage

The full check: which cells does a solution leave unmowed? Python's number is
the official verifier.

| Lawn                                 | Python   | Rust    |
| ------------------------------------ | -------- | ------- |
| `euronight0070000_8749k_3s12` (8.7M) | 4,410 ms | 15.7 ms |
| `octa28351_946k_3x7` (946k)          | 1,032 ms | 3.4 ms  |

The edit check: remove 1 to 40 positions from a tour and ask whether every cell
is still mowed. This is the question a tour-editing search asks for every move
it tries.

| Lawn         | Python (`VisitCounts.all_swept_around`) | Rust (`VisitCounts`) | Rust setup |
| ------------ | --------------------------------------- | -------------------- | ---------- |
| `octa…`      | 2.25 / s                                | 78,084 / s           | 24 ms      |
| `euronight…` | not measured                            | 14,778 / s           | 335 ms     |

**Checked:** on 248 stored bests with a corner removed or a lane shortened, the
uncovered count equals the official verifier's. On random edits, the edit check
equals a full recheck.

**Why it matters:** the ideas left to try (border-aware tours, searched lane
ends, per-band offsets, large neighbourhood search) all edit tours. At 2 checks
a second Python can try about 8,000 moves an hour. Rust tries that many in a
tenth of a second.

---

## 3. Ordering the segments

The same search in both: nearest neighbour start, 2-opt and Or-opt over the 8
nearest segment ends, kicks, keep the best. Seed 0, 3 seconds of kicks each.
"Link" is the length driven between segments, so lower is better.

| Case                                 | Segments | Python link | Rust link | Shorter by | Checks / s, Py → Rust | Moves / s, Py → Rust |
| ------------------------------------ | -------: | ----------: | --------: | ---------: | --------------------- | -------------------- |
| `fjr3672_65k_2p4`                    |      186 |       2,425 |     2,341 |       3.5% | 63k → 5.6M (89x)      | 17k → 2.1M (128x)    |
| `iso8005_105k_2s3`                   |      402 |       5,411 |     5,279 |       2.4% | 56k → 5.1M (90x)      | 16k → 1.8M (111x)    |
| `srpg5461_22k_1l3`                   |      728 |       4,783 |     4,507 |       5.8% | 44k → 4.8M (109x)     | 13k → 1.7M (136x)    |
| `fna52057_1415k_2x6-gap0`            |    1,343 |      20,480 |    18,132 |      11.5% | 37k → 5.0M (134x)     | 11k → 1.9M (173x)    |
| `monalisa100k_389k_1l4-gap0`         |    2,153 |      12,709 |    11,745 |       7.6% | 30k → 3.6M (121x)     | 8.4k → 1.3M (155x)   |
| `monalisa100k_389k_1l4-unit`         |   10,956 |      32,781 |    25,255 |      23.0% | 3.8k → 2.0M (539x)    | 750 → 660k (886x)    |
| `uniform00800001_708k_1s5-unit-gap0` |   30,489 |      82,069 |    65,389 |      20.3% | 717 → 777k (1083x)    | 76 → 252k (3312x)    |

The gap grows with the number of segments. Python's moves slow down as the order
gets longer and Rust's barely do. The two cases where Rust wins by 20% are the
`-unit` stress cases built for this test, with 11k and 30k segments. Real
instances have at most about 2,400, where the gain is 2% to 12% of the links.
Links are only part of a tour, so the gain in the final score is smaller still.

A stress case: 100,000 segments order from 558,452 to 495,936 in 1.3 s in Rust.

**Checked:** the nearest-neighbour start is identical to Python's on every
fixture.

---

## 4. Lanes and segments

Lane intervals per cutter row, trimmed by the cutter's reach, merged across
small gaps. Both axes, on the largest bench lawn, `isoa22945_987k_3x10` (987k
cells, 1,380 segments).

|        | Time    |
| ------ | ------- |
| Python | 50.2 ms |
| Rust   | 5.3 ms  |

**Checked:** 372 of 372 segment lists identical: 31 bench instances, both axes,
6 settings each (plain lanes, default, `gap_factor` 0 and 4, `offset` 1, `shift`
0.5).

---

## 5. Pictures

`mow-viz` draws a lawn and a solution as SVG. On `euronight…` a whole run takes
57 ms: about 12 ms to load, 12 ms to draw, and the rest is starting the process.
The file is 22 KB because the lawn merges into about 800 rectangles. All 250
bests render; the largest file is 510 KB (`iso252735_2982k_2l4`).

Python's `plot.py` draws with matplotlib and wasn't timed against it. Examples
are in `docs/figures/viz/`.

---

## Running in parallel

Python runs one process per job, and each process holds its own copy of the
instance. On the largest lawn that is about 318 MiB a job before any search
starts. Rust runs jobs as threads that share one read-only problem, so a job
costs only its own search state. The full grid (14,600 jobs) ran on 16 threads
in 103 minutes with no job failing.

---

## End to end

The port first ran Python's own pipeline:
`mow mine --threads 16 --kick-scale 0.25` ran Python's job grid on every
instance (25.5 CPU hours), then `mow polish --all` polished every best with
CP-SAT (59.9 CPU hours). That tied Python's polished bests before polish
(geomean 0.9999) and beat them on 168 of 250 after (geomean 0.9977). Score, the
sum of `(lower_bound / objective)²`: 113.57 of 250.

Then the new searches, each gated on the 31 bench instances first:

| step                                                               | CPU h   | points | points per CPU h |
| ------------------------------------------------------------------ | ------- | ------ | ---------------- |
| bench and all-instance runs of the new searches (bests taken over) | about 7 | +5.51  |                  |
| mining round 10, lanes flush with the region's edges               | 0.65    | +0.03  | 0.05             |
| `mow turns --all --seconds 30`                                     | 0.95    | +0.42  | 0.44             |
| `mow lns --all --seconds 60`                                       | 4.17    | +9.53  | 2.28             |
| `mow lns --all --pass 2 --seconds 300`                             | 20.84   | +3.24  | 0.16             |
| for comparison: `mow polish --all`                                 | 59.9    | +0.39  | 0.007            |

Final score 132.29 of 250. `mow compare` re-checked every best with the official
verifier (0 failures):

| group | instances | vs Python best: W / T / L | geomean | vs Python mined: W / T / L | geomean |
| ----- | --------- | ------------------------- | ------- | -------------------------- | ------- |
| all   | 250       | 250 / 0 / 0               | 0.9179  | 250 / 0 / 0                | 0.9155  |
| k = 1 | 85        | 85 / 0 / 0                | 0.8938  | 85 / 0 / 0                 | 0.8899  |
| k = 2 | 83        | 83 / 0 / 0                | 0.9139  | 83 / 0 / 0                 | 0.9122  |
| k = 3 | 82        | 82 / 0 / 0                | 0.9475  | 82 / 0 / 0                 | 0.9462  |
| bench | 31        | 31 / 0 / 0                | 0.8936  | 31 / 0 / 0                 | 0.8893  |

Below 1 means Rust is shorter. _Python best_ is the stored best, CP-SAT polish
included; _Python mined_ is Python's best before polish. CPU time: Rust 111.9 h
in all, Python 307.1 h (247.2 h mining, 59.9 h polish).

Tried and dropped on the bench: diagonal lanes (a proved bound shows no walk
sweeps faster than straight lanes), joining lanes across gaps, spirals, and
splitting the region into k parts before touring (5 of 21 k > 1 instances).

Kicks run at a quarter of Python's time (`--kick-scale 0.25`); every other
setting and time limit is Python's.

### Strategy tournament, 15 instances

One instance per cutter family and k (50k to 400k cells), four strategies with
the same 10 CPU hours each, then the leader alone for 60 more:

| arm | strategy                                                    | points at 10 CPU h | better on |
| --- | ----------------------------------------------------------- | ------------------ | --------- |
| A   | `mow lns`, 600 s passes, from the best                      | +0.145             | 15 / 15   |
| B   | same, temperature 0.003 instead of 0.0003                   | +0.130             | 10 / 15   |
| C   | `mow lns` from the best candidate of the other construction | **+0.169**         | 11 / 15   |
| D   | `mow lns` 600 s then `mow turns` 60 s, alternating          | +0.167             | 15 / 15   |

C's 60 deeper CPU hours added 0.107 more (0.0018 points per CPU hour, falling
each pass). The shortest of all arms went into `data/rust/best`: +0.31 points,
score **132.60**, all 250 re-checked by the official verifier. Restarting every
k = 1 and 2 instance that way then reached 133.57 (see PROGRESS.md).

By group, the fresh restart (C) led on 9 of the 10 k = 1 and 2 instances, and
continuing from the best (A, D) led on 4 of the 5 k = 3 instances. Cutter family
made no visible difference. With one instance per group this is a lead to test,
not a result.

---

## Where each Python module went

| Python                                                     | Rust                                                                     |
| ---------------------------------------------------------- | ------------------------------------------------------------------------ |
| `model.py`, `bounds.py`, `tours.py`                        | `mow-core`, `mow-io`                                                     |
| `construct/lanes.py`, `segments.py`, `mixed.py`            | `mow-decompose`                                                          |
| `construct/ordering.py`                                    | `mow-order`                                                              |
| `repair.py`, `improve.py`                                  | `mow-improve`, with coverage in `mow-coverage`                           |
| `split.py`, `construct/groups.py`, `partition.py`          | `mow-assign`                                                             |
| `pipeline.py`                                              | `mow-pipeline`, `mow solve`                                              |
| `jobs.py`, `mine.py`                                       | `mow mine` (`mow-cli`), `mow-store`, `mow-verify`                        |
| `window.py`, `window_map.py`, `polish.py`                  | `mow-polish`, `mow polish`, `mow window-map`; CP-SAT through `mow-cpsat` |
| `report.py`, `mine_report.py`, `diagnose.py`, `catalog.py` | `mow-report`, `mow report \| mine-report \| diagnose \| catalog`         |
| `bench.py`                                                 | `mow bench pick \| score \| run`                                         |
| `plot.py`                                                  | `mow-viz`                                                                |
| `resources.py` (memory permits for job processes)          | not ported: Rust jobs are threads sharing one problem per instance       |
| `plot_app.py` (a desktop window)                           | not ported: `mow-viz` writes SVG files instead                           |

Two pieces stay in Python on purpose. The official verifier
(`cgshop2027_pyutils.verify`) stays the judge, and Rust's coverage check is only
a fast pre-check. CP-SAT is OR-Tools: Rust builds each window model itself
(checked equal to Python's, constraint by constraint) and hands it to a
long-lived Python process as OR-Tools' own `CpModelProto`, so a native binding
could replace the process later without touching the model code.

---

## Talking points

1. **Ruin and recreate is the lever.** `mow lns` earned 12.8 points, two thirds
   of the gain, and its 60 s pass paid 2.3 points per CPU hour, 300 times CP-SAT
   polish. Its 300 s pass still paid 0.16, so the large instances have not
   settled.
2. **Polish is ported but no longer worth its time.** It took 54% of Rust's CPU
   for 0.39 points. `mow polish --all` now skips instances above 400k cells.
3. **The X cutters' gap was the bound.** Their area bound counted both arms of
   the X; the hull bound (`mow-core`) shows X has no more room than any other
   shape.
4. **Restarts beat depth.** Past the first two `mow lns` passes, a restart from
   another construction earns more than more time on the best, at least for k =
   1 and 2.
5. **Known parity gaps.** As in Python, a partitioned job can overrun its time
   limit, and a failed job isn't retried after a restart.

---

## Reproduce

Rust, on `rust-port`:

```bash
cargo test --release --workspace -- --ignored --nocapture       # order and segment timings
cargo run --release -p mow-coverage --example bench octa28351_946k_3x7
cargo run --release -p mow-coverage --example bench euronight0070000_8749k_3s12
cargo run --release -p mow-io --example load_time -- <instance.json>
cargo run --release -p mow-viz -- <instance.json> <solution.json> -o out.svg
target/release/mow mine --threads 16 --kick-scale 0.25 --out data/rust
# docs/rust-results.md came from the old `mow compare` report against Python,
# removed on 2026-11-18; the report itself is kept as it was
```

Python's numbers come from the scripts in `tools/golden/`, which need the Python
solver; see `tools/golden/README.md` for running them on `init-solver`.
