# Timeline

How the solver got from nothing to a pessimistic score of 141.18 of 250, in five
stretches of work. The details of every step are in
[`PROGRESS.md`](../PROGRESS.md). How the solver works is in
[`how-it-works.md`](how-it-works.md).

**Pessimistic score:** each instance scores
`(lower bound / our longest tour)^2`, summed over the 250 challenge instances.
It is the score we would get if some team reached every lower bound, so the real
score can only be higher.

## At a glance

| dates          | stretch                        | score at the end | what moved it                               |
| -------------- | ------------------------------ | ---------------- | ------------------------------------------- |
| 08-18 to 08-25 | starting point                 | -                | the organizers' `pyutils`: format, verifier |
| 10-28 to 10-31 | Python solver and mining       | 113.10           | lane segments, k-cutter groups, mining      |
| 11-12 to 11-13 | port to Rust                   | 113.57           | same tours, 3.6x less CPU                   |
| 11-13 to 11-15 | searches on the finished tours | 137.91           | `turns` and `lns` (ruin and recreate)       |
| 11-15 to 11-18 | tuning the search              | 141.18           | drift fix, temperature cap, long passes     |

## Score over time

| date        | event                                             | score  | change |
| ----------- | ------------------------------------------------- | ------ | ------ |
| 10-29       | first overnight mining run, Python                | ~110   |        |
| 10-30       | mining round 8: k-cutter groups                   | 112.17 | +2.0   |
| 10-31       | round 9, polish on every instance                 | 113.10 | +0.9   |
| 11-13       | Rust port, same constructions                     | 113.57 | +0.5   |
| 11-13 night | `turns` and the first `lns` passes                | 132.29 | +18.7  |
| 11-14       | strategy tournament on 15 instances               | 132.60 | +0.3   |
| 11-14       | restarts from the other construction, k = 1 and 2 | 133.57 | +1.0   |
| 11-14       | edge-aimed `lns` move (a wash), merged            | 133.62 | +0.05  |
| 11-14       | faster `lns`: better repair anchors, pass 3       | 136.34 | +2.7   |
| 11-15       | runs of anchors stamped at once, pass 4           | 137.66 | +1.3   |
| 11-15 night | second restart round                              | 137.91 | +0.25  |
| 11-16       | pass 5, time weighted by history                  | 139.52 | +1.6   |
| 11-16       | pass 6, walk drift fixed                          | 140.05 | +0.5   |
| 11-17       | temperature capped in cutter spans (A/B merged)   | 140.63 | +0.6   |
| 11-17       | pass 7, capped                                    | 141.06 | +0.4   |
| 11-18       | pass 8, capped                                    | 141.18 | +0.12  |

Not yet merged: the slide move run on today's bests is shorter on 46 instances,
another +0.067.

## 1. Starting point (2026-08-18 to 08-25)

The repository began as the organizers' `pyutils` package: the instance and
solution formats and the official verifier. Every tour we ever kept passed that
verifier.

## 2. Python solver and mining (2026-10-28 to 10-31)

- **10-28, kickoff.** An eight-hour plan: catalog the 250 instances, a strip
  baseline, a split for k cutters, lower bounds, then a parallel miner on 15
  cores.
- **10-29.** Lane segments that skip gaps beat plain lanes by up to 32%. The
  first overnight run: 250 / 250 feasible, 1.1% shorter than the baseline,
  median gap to the bound 47.9%.
- **10-29, the measurements.** A fixed 31-instance bench, and `diagnose`: about
  half the lost length sits at lane ends and turns. An exact window re-solve
  (CP-SAT) found the tours already optimal at small scale, and is too slow at
  the scale of the waste. Plan rewritten: decompose, then search.
- **10-30.** Cells that follow the lawn's shape fail their gate. k-cutter group
  constructions (round 8) pay 15 times more per CPU hour than any round since
  round 2. Narrow lanes rejected. Score 112.17.
- **10-31.** Round 9 and window polish on every instance. Score 113.10.

**Lesson:** a miner that rebuilds tours from global settings had used itself up.
Rounds 3 to 6 bought 0.4 points for 64 CPU hours.

## 3. Port to Rust (2026-11-12 to 11-13)

The Python solver was replaced by a Cargo workspace laid out as a hexagon:
domain, plugins, adapters, app. Every piece was checked against fixtures
exported from Python before it was timed.

| piece      | matches Python                | Rust is                    |
| ---------- | ----------------------------- | -------------------------- |
| load       | 250 / 250 instances           | 8x faster, 55x less memory |
| coverage   | 248 / 248 uncovered counts    | 280x                       |
| edit check | equals a full recheck         | 2.25 / s to 78,000 / s     |
| order      | shorter on 7 / 7, same budget | 110x to 3,300x             |

The same mining took 85 CPU hours against Python's 307. Score 113.57.

**Lesson:** the port bought time, not points. The points had to come from
searches that 78,000 edit checks a second make possible.

## 4. Searches on the finished tours (2026-11-13 to 11-15)

- **11-13, five tracks in parallel**, each gated on the bench (better on 11 of
  31, at least 0.05 points per CPU hour):

  | track | idea                                    | result                    |
  | ----- | --------------------------------------- | ------------------------- |
  | A     | drop the rounds that no longer pay      | kept                      |
  | B     | tighter lower bounds (orthogonal hull)  | kept, as catalog columns  |
  | C     | `turns`: slide, shortcut, 2-opt, or-opt | 31 / 31, 3.15 pts / CPU h |
  | D     | lanes that hug the edge, joins, spirals | hug only                  |
  | E     | `lns`: ruin and recreate, adaptive      | 31 / 31, 4.50 pts / CPU h |
  | F     | split the lawn before touring           | dropped, 5 / 21           |

- **11-13 night, the rerun.** `turns` +0.42, the first 60 s `lns` pass **+9.53**
  (2.28 points per CPU hour, 300 times polish), a 300 s pass +3.24. Score 113.57
  to **132.29**, every best shorter than Python's.
- **11-14.** A tournament of four strategies on 15 instances: starting fresh
  from the other construction wins. Restarts on every k = 1 and 2 instance win
  92 of 149. A move aimed at the lawn's edge is a wash. Profiling makes `lns`
  2.6x faster (repair anchors, bared cells), and pass 3 gives 187 new bests.
- **11-15.** Runs of anchors stamped at once, up to 5x the moves on large
  cutters, then pass 4. Three side tracks: splitting costs only 1.2 points over
  all k > 1 instances; jointly chosen lane ends fail; time weighted by history
  ties an even split on the bench.

## 5. Tuning the search (2026-11-15 to 11-18)

- **11-16, pass 5**, nine hours weighted by history: +1.61. It found **the walk
  drifts**: on large tours the saved walk ends far above the best, up to 48%.
  Fix: resume from the best once the walk is more than 0.5% above it.
- **11-16, pass 6** with the fix: +0.53. The walk still climbed away within a
  run, because the temperature was a share of the whole tour, up to 11 cutter
  spans on the longest.
- **11-17, the temperature cap.** Held to 0.3 cutter spans: +0.58 against +0.01
  on the 39 hottest instances, A/B, same CPU. Stuck instances move again, by up
  to 6.8%.
- **11-17 and 11-18, passes 7 and 8**, capped, even time: +0.44, then +0.12.
  Pass 8 paid 0.0015 points per CPU hour, under the 0.002 stopping line.
- **11-18, slide measured** on the current bests: 0.61 points per CPU hour, 400
  times pass 8. That is the next lever.

## Points per CPU hour, the measure behind every decision

| run                     | points per CPU hour |
| ----------------------- | ------------------- |
| first 60 s `lns` pass   | 2.28                |
| first 300 s `lns` pass  | 0.16                |
| faster `lns`, pass 3    | 0.12                |
| mining round 8 (groups) | 0.073               |
| pass 4                  | 0.06                |
| temperature cap A/B     | 0.055               |
| pass 5                  | 0.012               |
| pass 6                  | 0.004               |
| pass 7                  | 0.003               |
| pass 8                  | 0.0015              |
| polish, all 250         | 0.007               |
| slide on today's bests  | 0.61                |
