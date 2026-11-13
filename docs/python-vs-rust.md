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
- **Same scores for a twelfth of the CPU.** One Rust mining run over all 250
  instances took 103 minutes on 16 threads (25.5 CPU hours). Its bests, every
  one checked by the official verifier, tie Python's stored bests (geomean
  0.9999), and those include CP-SAT polish, which Rust doesn't have. Against
  Python's own mining without polish (247 CPU hours), Rust is shorter on 170 of
  250 instances (geomean 0.9973). Full table:
  [rust-results.md](rust-results.md).

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

`mow mine --threads 16 --kick-scale 0.25` ran Python's job grid on every
instance, and `mow compare` re-checked every Rust best with the official
verifier (0 failures) before comparing.

| group | instances | vs Python best: W / T / L | geomean | vs Python mined: W / T / L | geomean |
| ----- | --------- | ------------------------- | ------- | -------------------------- | ------- |
| all   | 250       | 122 / 17 / 111            | 0.9999  | 170 / 28 / 52              | 0.9973  |
| k = 1 | 85        | 32 / 2 / 51               | 1.0014  | 66 / 6 / 13                | 0.9971  |
| k = 2 | 83        | 51 / 5 / 27               | 0.9979  | 61 / 9 / 13                | 0.9961  |
| k = 3 | 82        | 39 / 10 / 33              | 1.0003  | 43 / 13 / 26               | 0.9988  |
| bench | 31        | 13 / 1 / 17               | 1.0019  | 21 / 3 / 7                 | 0.9971  |

Below 1 means Rust is shorter. _Python best_ is the stored best, CP-SAT polish
included; _Python mined_ is Python's best before polish. Per instance, Rust
ranges from 0.977 to 1.036 of Python's best: 11 instances are more than 1%
shorter, 19 more than 1% longer. CPU time: Rust 25.5 h, Python 247.2 h mining
plus 59.9 h polish.

Kicks run at a quarter of Python's time (`--kick-scale 0.25`); every other
setting and time limit is Python's.

---

## Not ported

| Python                                                                 | Status                                                        |
| ---------------------------------------------------------------------- | ------------------------------------------------------------- |
| `window.py`, `window_map.py`, `polish.py` (CP-SAT)                     | not ported; Python's polished bests stay in `data/solutions/` |
| `report.py`, `mine_report.py`, `diagnose.py`, `bench.py`, `catalog.py` | `mow compare` covers the score report; the rest is not ported |

Everything else (`mixed`, `repair`, `improve`, `split`, `groups`, `partition`,
`pipeline`, `jobs`, `mine`) is in the crates. The official verifier
(`cgshop2027_pyutils.verify`) stays in Python and stays the judge. Rust's
coverage check is a fast pre-check, not a replacement.

---

## Talking points

1. **Polish is the gap on k = 1.** Rust loses 51 of 85 single-cutter instances
   to the polished bests but wins 66 of 85 against Python's mining. Porting the
   windowed CP-SAT polish (or running it on Rust's bests) is the clearest next
   gain.
2. **Spend the saved CPU.** Rust used a twelfth of Python's mining time. More
   seeds, full kick times, or longer limits on the large instances are now cheap
   to try.
3. **Which bests to submit.** Taking the shorter of Python's and Rust's per
   instance is free today, since both pass the official verifier.
4. **Known parity gaps.** As in Python, a partitioned job can overrun its time
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
target/release/mow compare --out data/rust --report docs/rust-results.md
```

Python's numbers come from the scripts in `tools/golden/`, which need the Python
solver; see `tools/golden/README.md` for running them on `init-solver`.
