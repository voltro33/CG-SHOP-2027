# Deck brief: Mowing the Lawn, Shorter

A brief for a 15 to 20 minute talk on our CG:SHOP 2027 work, written so that
Claude Fable can build the deck from it. All the numbers come from
[`../timeline.md`](../timeline.md) and [`../../PROGRESS.md`](../../PROGRESS.md).
The figures it uses are in [`../figures/`](../figures/).

## The idea

**One story, told by a single number.** The score climbs from 110 to 141 in
three weeks. Each section of the talk is one bend in that curve, and the
question every section answers is the one we asked every day: _what pays the
most points per CPU hour?_

The arc:

1. The problem is easy to state and NP-hard. We need heuristics and a floor to
   measure them against.
2. Build good tours fast, then mine: many settings on every core. It works, then
   it stops paying.
3. Measure where the length goes. Half of it is lane ends and turns.
4. Port to Rust. The port alone buys no points, but it buys 78,000 checks a
   second.
5. Spend that speed on searches over the finished tours. Ruin and recreate gives
   the biggest jump: +18.7 in one night.
6. Tune the search with A/B tests. One bug (the walk drifts) and one fix (cap
   the temperature in cutter spans) release instances that were stuck.
7. Know when to stop: under 0.002 points per CPU hour, a pass is not worth the
   machine time. What comes next.

**Audience:** a graduate Analysis of Algorithms class. They know NP-hardness,
TSP, local search and simulated annealing by name. They do not know the contest.

## Look and feel

- Clean and technical. One idea per slide, short headlines that state the
  finding ("The port bought time, not points"), not topic labels ("Rust port").
- **The score curve is the recurring visual.** It appears whole on slide 2, then
  each section repeats it small in a corner with that section's stretch
  highlighted.
- One accent colour for "kept", a muted grey for "tried and dropped".
- Grid and cell motifs suit the subject: a lawn of square cells, a cutter stamp.
- No more than about 30 words of body text per slide. The detail goes in the
  speaker notes.

## Slides

### 1. Title

**Mowing the Lawn, Shorter.** CG:SHOP 2027: planning k cutter tours over a grid
lawn. Course, name, date.

Visual: `figures/viz/pcb1173_41k_1x11.svg` (a whole stored tour) as a faint
background.

### 2. The whole story in one chart

Headline: _From 110 to 141 in three weeks_.

Visual: a line chart of the score over time, with these points:

| date  | score  | label            |
| ----- | ------ | ---------------- |
| 10-29 | 110.0  | first mining run |
| 10-30 | 112.17 | k-cutter groups  |
| 10-31 | 113.10 | polish           |
| 11-13 | 113.57 | Rust port        |
| 11-13 | 132.29 | turns + lns      |
| 11-14 | 136.34 | faster lns       |
| 11-15 | 137.91 | pass 4, restarts |
| 11-16 | 140.05 | passes 5 and 6   |
| 11-17 | 141.06 | temperature cap  |
| 11-18 | 141.18 | pass 8           |

Band the background into the four stretches: Python and mining, port, searches,
tuning. The cliff at 11-13 should be the obvious feature.

Notes: the score is pessimistic. Each instance counts
`(lower bound / our length)^2`, so it is what we would get if someone matched
every bound. The real score is higher.

### 3. The problem

Headline: _Cover every cell; make the longest tour short._

- A lawn of grid cells, often with holes. A cutter shape. k = 1 to 3 cutters.
- One closed tour per cutter, moving in unit steps up, down, left, right.
- Together the stamps must cover the lawn. Minimise the longest tour.

Visual: `figures/cutters.png` (the five cutter families) and
`figures/solution-k2.png`.

Notes: 250 instances, up to 8.7 million cells. Scoring is about
`(best known / ours)^2`, so 1% shorter is worth about 2% of an instance.

### 4. Why heuristics, and why a floor

Headline: _NP-hard, so we measure against a lower bound._

- A 1 x 1 cutter on scattered cells is exactly TSP.
- Lower bounds: area (cells over the cells a step can add), extent, and an
  orthogonal hull bound we proved.
- The gap to the bound tells us where room is left.

Visual: a bar chart of the median gap by cutter family today: square 21.0,
clipped square 22.0, plus 28.9, diamond 29.3, X 69.3 (%).

Notes: most of the X gap is the bound, not the tours. Against the hull bound the
X cutters have no more room than the others.

### 5. The pipeline

Headline: _Build fast, verify always, keep the best._

Visual: a left-to-right flow: instance, then lower bound, lane segments, order
(local search), repair, split among k, official verifier, best store, `turns`,
`lns`, and back to the verifier.

Notes: one run of the construction with one set of knobs is a job. `mine` runs
grids of jobs on every core. Nothing is kept unless the organizers' verifier
passes it: 250 / 250, every time.

### 6. Mining, and when it ran dry

Headline: _Many settings on every core, until it stopped paying._

- Lanes cut at gaps: up to 32% shorter than plain lanes.
- Groups for k cutters: 15 times the points per CPU hour of the rounds before.
- Then rounds 3 to 6: 0.4 points for 64 CPU hours.

Visual: a small table or bars of points per CPU hour by mining round.

### 7. Where the length goes

Headline: _Half the waste is at lane ends and turns._

Visual: `figures/waste.png`.

Notes: an exact CP-SAT window re-solve found the tours already optimal at small
scale, and it was far too slow at the scale of the waste. That ruled out exact
local polishing and pushed us to search instead.

### 8. The Rust port

Headline: _The port bought time, not points._

| piece      | Rust is                 |
| ---------- | ----------------------- |
| load       | 8x faster, 55x less RAM |
| coverage   | 280x                    |
| edit check | 2.25 / s to 78,000 / s  |
| order      | 110x to 3,300x          |

Notes: checked piece by piece against fixtures exported from Python. Same tours:
113.57 against Python's 113.10, at 85 CPU hours instead of 307.

### 9. Five tracks, one gate

Headline: _Every idea had to pass the same bench._

Visual: the track table, with kept in the accent colour and dropped in grey: A
rounds pruned (kept), B hull bound (kept), C `turns` (kept), D edge lanes (hug
only), E `lns` (kept), F split first (dropped).

Notes: the gate was better than the stored best on 11 of 31 bench instances and
at least 0.05 points per CPU hour.

### 10. Ruin and recreate

Headline: _+18.7 points in one night._

- Take a piece of tour out (destroy), lay it again (recreate), accept by
  simulated annealing.
- Operators chosen by weights learned from their wins (adaptive LNS, Ropke and
  Pisinger 2006).
- First 60 s pass: +9.53 points, 300 times what polish paid per CPU hour.

Visual: a three-panel sketch: tour, hole torn out, hole re-covered.

### 11. Making it faster pays too

Headline: _More moves a second, more points._

- Repair anchors chosen by coverage: 2.6x the moves, then 187 new bests.
- Runs of anchors stamped at once: up to 5x on large cutters.

Visual: before and after bars of moves in 60 s (isoa22945_987k: 16k to 80k;
octa6025_952k: 26k to 115k; smo64863_393k: 72k to 283k).

### 12. A bug in the walk

Headline: _On long tours the walk drifted away from the best._

- The temperature was 0.0003 of the whole tour: up to 11 cutter spans.
- A move changes a few spans, whatever the tour's length.
- Cap it at 0.3 spans. A/B on the 39 hottest instances, same CPU:

| arm     | points | better  |
| ------- | ------ | ------- |
| no cap  | +0.01  | 7 / 39  |
| cap 0.3 | +0.58  | 25 / 39 |

Notes: instances no pass had moved gained up to 6.8%. The general lesson is to
scale the temperature to the size of a move, not the size of the solution.

### 13. Knowing when to stop

Headline: _Points per CPU hour decided every next step._

Visual: a log-scale bar chart of points per CPU hour: first `lns` pass 2.28, 300
s pass 0.16, pass 4 0.06, pass 6 0.004, pass 8 0.0015, slide on today's bests
0.61.

Notes: we stop a kind of run below 0.002 points per CPU hour. Pass 8 fell under
it. Slide, a corner move from `turns`, still pays 400 times as much on the same
tours. That points to the next step.

### 14. What we tried and dropped

Headline: _Most ideas failed their gate, and that was the point._

Two columns. Kept: lane segments, groups, hull bound, `turns`, `lns`, faster
repair, drift fix, temperature cap. Dropped: cover then TSP, shape-following
cells, narrow lanes, diagonal lanes (ruled out by the hull bound), spirals,
split first, edge-aimed move, a hotter walk.

### 15. Lessons and next steps

- Measure before you build: a bench, a waste breakdown, a gate.
- Speed is a means. It paid only once a search could spend it.
- Fix the scale of the search, not just its length.
- Next: slide inside `lns` as a finishing step, then on the walk itself.

Closing visual: the score curve again, ending at 141.18.

## Prompt for Fable

> Build a 15-slide presentation deck from `docs/slides/deck-brief.md`. Follow
> its slide list, headlines, tables and chart data exactly, and put the "Notes"
> text in the speaker notes. Use the images it names from `docs/figures/`. Keep
> body text under about 30 words a slide, and repeat the score curve small on
> each section's slides, with that section's stretch highlighted.
