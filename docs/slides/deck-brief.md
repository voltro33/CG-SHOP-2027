# Deck brief: Mowing the Lawn, Shorter

A brief for a 15 to 20 minute talk on our CG:SHOP 2027 work, written so that
Claude Fable can build the deck from it. The real-instance numbers come from
[`../timeline.md`](../timeline.md) and [`../../PROGRESS.md`](../../PROGRESS.md).

## The idea

**Explain the problem so anyone gets it, then show every approach on one small
lawn.** The audience sees the same toy lawn on almost every slide, so the only
thing that changes is the idea. Each approach slide pairs one picture of the toy
lawn with one number: the length of the tour on it. Then the real-instance
result says whether the idea paid off at scale.

The toy lawn's story, in one line:

| step                                 | longest tour, 1 cutter | 2 cutters |
| ------------------------------------ | ---------------------- | --------- |
| plain lanes (naive)                  | 212                    |           |
| lane segments                        | 194                    | 112       |
| corner moves (turns)                 | 180                    | 110       |
| ruin and recreate                    | **160**                | **84**    |
| lower bound (no tour can be shorter) | 156                    | 78        |

From 36% above the bound to 2.6% above it.

**Audience:** a graduate Analysis of Algorithms class. They know NP-hardness,
TSP, local search and simulated annealing by name. They do not know the contest.

## The figures

All in [`figures/`](figures/), clean pictures on white with no titles or text
except where noted, so the slide supplies the words. Each is an SVG, and a PNG
at twice the size for Google Slides, which cannot import SVG. They are
references: redraw them more beautifully if you like, but keep what they show,
since the tours are real. They are drawn by `tools/slides/figures.py`:

- **Real tours:** every tour on the toy lawn is a real solver result on
  [`toy/instances/`](toy/instances/), and each passed the contest's official
  verifier. The script checks again that each one mows every cell.
- **Concept pictures:** the stamp, slide, narrow lanes and cover then tour are
  built in the script, and each is checked to mow its whole lawn.

**The toy lawn:** an L-shaped lawn of 476 grid cells, 31 by 20, with a pond in
it, cut by a 3 × 3 square cutter.

**One colour legend for every figure.** Put it once on slide 6 and keep it small
in a corner afterwards:

| colour         | means                                                     |
| -------------- | --------------------------------------------------------- |
| light green    | lawn                                                      |
| light blue     | pond: no need to mow it, but you may drive over it        |
| dark blue line | the tour of cutter 1 (dot: its start)                     |
| red line       | the tour of cutter 2                                      |
| orange cells   | mowed but not lawn: off the edge or over the pond (waste) |
| yellow cells   | lawn mowed a second time (waste)                          |
| red cells      | lawn left unmowed (only in the ruin and recreate figure)  |

## Animations

The multi-step ideas also come as **frames** in `figures/frames/`: the same
picture one step at a time, every frame the same size, so a slide can build it
up. In Google Slides, stack a slide's frames exactly on top of each other and
give each one a "Fade in" (on click), or make one slide per frame with a short
dissolve transition between them.

| slide                   | frames, in order                                                         | what the audience sees                                                            |
| ----------------------- | ------------------------------------------------------------------------ | --------------------------------------------------------------------------------- |
| 2. A lawn and a cutter  | `lawn`, then the full `01-lawn-and-cutter`                               | the lawn appears, then the cutter beside it                                       |
| 3. Driving mows         | `stamp-sweep-0` to `stamp-sweep-6`                                       | the cutter steps right, a new column of 3 squares at each step                    |
| 5. More than one cutter | `two-cutters-1`, `two-cutters-2`                                         | cutter 1's tour, then cutter 2's joins it                                         |
| 9. Mining               | `mining-1-lanes-v` to `mining-4-segments-h`                              | four settings one by one, numbers 228, 210, 212, 194; 194 is kept                 |
| 12. Ideas that failed   | `narrow-1-three-apart`, `narrow-2-two-apart`                             | lanes 3 apart, then 2 apart with the overlap in yellow                            |
| 14. Corner moves        | `slide-1-before`, `slide-2-after`                                        | the U-turn sticking out past the edge slides back in                              |
| 15. Ruin and recreate   | `ruin-1-tour`, `ruin-2-window`, `ruin-3-torn-out`, `ruin-4-after-search` | the tour; a window; the piece torn out, 55 squares red; the tour after the search |
| 17. The whole journey   | `journey-1-lanes-h` to `journey-4-lns`                                   | the same lawn four times: the waste shrinks, 212 → 194 → 180 → 160                |

Elsewhere keep motion light: bring a slide's bullet points in one by one, and
count the toy number up (or fade it in) after its figure.

## Look and feel

- Clean, light and calm. Lots of white space; the figure is the slide.
- **One idea per slide.** Headline states the idea in plain words; at most two
  short lines of text beside the figure; detail in the speaker notes.
- **The toy number** sits large next to each toy figure, e.g. "**194** steps
  (was 212)", with the lower bound 156 shown faintly for scale.
- Keep the figure colours as they are. Use the dark blue (`#1f3a5f`) as the
  deck's accent.
- Avoid jargon on the slides. Say "the longest tour", not "the objective";
  "lanes", not "boustrophedon".

## Slides

### Part 1: the problem in simple terms

#### 1. Title

**Mowing the Lawn, Shorter.** CG:SHOP 2027 · course, names, date.

Figure: `14-lns.svg`, faint, as a background on the right.

#### 2. A lawn and a cutter

Headline: _Mow every square of the lawn._

- The lawn is a grid of squares; some have ponds or holes.
- The cutter is a small shape. Here a 3 × 3 square.

Figure: `01-lawn-and-cutter.svg` (the toy lawn, the cutter beside it).

Notes: the 250 contest lawns range from a few thousand to 8.7 million squares.
There are five cutter shapes: square, clipped square, plus, diamond and X.

#### 3. Driving mows

Headline: _The cutter mows whatever is under it as it drives._

- It moves one square at a time: up, down, left or right.
- Each step brings at most 3 new squares under a 3 × 3 cutter.

Figure: `02-stamp-and-sweep.svg` (its two labels are in the figure).

Notes: this is also where the lower bound comes from. The toy lawn has 476
squares; the first position mows 9, every step at most 3 more, so any tour needs
at least (476 − 9) / 3 ≈ 156 steps. Our final toy tour has 160.

#### 4. The goal

Headline: _One closed tour that mows everything, as short as possible._

- Start anywhere, drive, come back to the start.
- Driving off the lawn or over the pond is allowed, but it is wasted time.

Figure: `03-one-tour.svg` (the best toy tour, 160 steps).

#### 5. More than one cutter

Headline: _With 2 or 3 cutters, the longest tour counts._

- The cutters share the work; the job ends when the last one is back.
- So we balance them: here 84 and 82 steps.

Figure: `04-two-cutters.svg`.

Notes: the score per lawn is (lower bound / our longest tour)², summed over the
250 lawns, so every percent shorter is worth about two percent of that lawn's
points.

#### 6. Why it is hard

Headline: _It contains the travelling salesman problem._

- With a 1 × 1 cutter and scattered squares, it is exactly TSP: NP-hard.
- The lawns go up to 8.7 million squares, so: fast heuristics, then search,
  measured against the lower bound.

Figure: none, or a small icon; put the colour legend here for what follows.

### Part 2: what we tried, from the naive approach up

Each slide: the idea in one line, the toy figure, the toy number, and one line
on what it did on the real 250.

#### 7. The naive approach: plain lanes

Headline: _Mow in straight rows, like a real lawn._

Figure: `10-plain-lanes.svg`. Toy: **212** steps.

- Rows across the whole lawn, turning at the ends.
- It mows over the pond (orange) and drives back along the left edge mowing it
  again (yellow).

Notes: always valid and quick, and the baseline every later idea is measured
against.

#### 8. Lane segments

Headline: _Skip what is not lawn, and order the pieces well._

Figure: `11-lane-segments.svg`. Toy: **194** steps (was 212).

- Cut each row where it leaves the lawn; skip the gaps.
- Visit the pieces in a good order: a TSP over the pieces, solved by local
  search.

Notes: on the real lawns, segments beat plain lanes by up to 32%.

#### 9. Mining: try many settings, keep the best

Headline: _No single setting wins everywhere._

Figure: `09-mining.svg`, four panels: lanes up and down, segments up and down,
lanes across, segments across. Toy: **228, 210, 212, 194**; keep 194.

- Every lawn is solved with dozens of settings on every core.
- Only the shortest verified tour is kept.

Notes: the first overnight run: 250 / 250 valid, 1.1% shorter than the baseline.
Later rounds paid less and less: 0.4 points for 64 CPU hours.

#### 10. Two or three cutters

Headline: _Cut one good tour into fair pieces._

Figure: `12-split-walk.svg` (2 cutters). Toy: **112** steps for the longer of
the two.

- Build one tour, split it into k pieces of about equal length, close each.
- Also tried: cutting the lawn into bands first, and building groups of lanes
  per cutter. Each wins on some lawns, so the miner runs all of them.

#### 11. Where the length goes

Headline: _A quarter of every tour is waste._

Figure: `11-lane-segments.svg` again, the orange and yellow highlighted.

Real numbers, all 250 best tours: 75.4% mows new lawn; 11.1% mows past the edge
(most of it at the ends of lanes); 6.8% mows lawn again; 6.7% drives over mowed
lawn. Show as one horizontal stacked bar.

Notes: this measurement decided what to try next.

#### 12. Ideas that failed

Headline: _Most ideas failed their test, and that was useful._

Two figures side by side:

- `21-cover-then-tour.svg`: choose cutter positions that cover the lawn, then
  visit them in TSP order. Toy: **222** steps, worse than plain lanes. It never
  won a single real lawn.
- `20-narrow-lanes.svg`: lanes 2 apart instead of 3, so they overlap (yellow).
  Toy strip: **72** against **54** steps.

Also dropped, as one short list: an exact solver on small windows (too slow, and
the windows were already optimal), cells shaped to the lawn, diagonal lanes (our
bound proves they cannot be faster), spirals, splitting the lawn before touring.

Notes: every idea had to pass the same test on a fixed set of 31 lawns before it
ran on all 250.

#### 13. A faster engine

Headline: _Rewriting in Rust bought time, not points._

Figure: a simple bar chart, no toy picture: coverage check 280 × faster; edit
checks 2.25 → 78,000 per second; ordering 110 to 3,300 × faster.

- Same tours as before, 3.6 × less CPU.
- The speed matters only because it lets us search.

#### 14. Corner moves

Headline: _Pull each U-turn back to the edge._

Figures: `16-slide.svg` (the idea: a turn that overshoots the edge, orange, slid
back in; **24 → 20** steps), then `13-turns.svg` (the toy tour after all corner
moves: **180**, was 194).

- Slide a U-turn inward, take shortcuts, swap two links (2-opt), move a piece
  elsewhere (or-opt).
- Every move is checked to keep the lawn fully mowed.

Notes: on the real lawns, 0.44 points per CPU hour, 60 times what polish paid.

#### 15. Ruin and recreate

Headline: _Tear out a piece of the tour, then lay it again._

Figure: `17-ruin-and-recreate.svg`, three panels left to right:

1. the tour, a window marked (dashed box);
2. the tour inside the window removed: **55** squares left unmowed (red);
3. the tour after thousands of such moves: **160** steps (was 180).

- Repair lays new tour over the red squares; keep the result if it is shorter,
  and sometimes even if slightly longer (simulated annealing).
- Several ways to tear and to repair; the ones that win more get picked more
  (adaptive large neighbourhood search).

Notes: on the real lawns this was the breakthrough: **+18.7 points in one
night**, 300 times what polish paid per CPU hour.

#### 16. The same for two cutters

Headline: _The search also moves work between cutters._

Figure: `18-lns-two-cutters.svg`. Toy: **84** steps (was 112), bound 78.

- A move can hand a stretch of lawn from one cutter to the other.

#### 17. The whole journey on one lawn

Headline: _212 → 194 → 180 → 160, against a floor of 156._

Figure: `15-journey.svg`: plain lanes, segments, corner moves, ruin and
recreate, left to right. Write each number under its panel.

Notes: the waste (orange, yellow) disappears panel by panel.

### Part 3: tuning, results, lessons

#### 18. Tuning the search on real lawns

Headline: _Measure every change with an A/B test._

- Faster moves: up to 5 × on large cutters, then 187 new bests in one pass.
- A bug: on long tours the search wandered far from its best (up to 48% longer).
  The "temperature" was a share of the whole tour; capping it at a fraction of
  one cutter width freed stuck lawns: **+0.58 against +0.01** points in the same
  CPU time.

Figure: two bars for the cap A/B (no cap +0.0111, capped +0.5776).

#### 19. The score over time

Headline: _From 110 to 141 in three weeks._

Figure: line chart of the score:

| date  | score  | label                     |
| ----- | ------ | ------------------------- |
| 10-29 | 110.0  | first mining run          |
| 10-30 | 112.17 | groups                    |
| 10-31 | 113.10 | polish                    |
| 11-13 | 113.57 | Rust port                 |
| 11-13 | 132.29 | turns + ruin and recreate |
| 11-14 | 136.34 | faster search             |
| 11-15 | 137.91 | pass 4, restarts          |
| 11-16 | 140.05 | passes 5 and 6            |
| 11-17 | 141.06 | temperature cap           |
| 11-18 | 141.18 | pass 8                    |

Notes: the score is pessimistic: it assumes someone reaches every lower bound,
so the real score is higher.

#### 20. Knowing when to stop

Headline: _Points per CPU hour decided every next step._

Figure: horizontal bars, log scale: first ruin and recreate pass 2.28; 300 s
pass 0.16; pass 4 0.06; pass 6 0.004; pass 8 0.0015; corner moves on today's
bests 0.61.

- Below 0.002 points per CPU hour a pass is not worth the machine.

#### 21. Lessons and next steps

- Measure before building: a fixed test set, a waste breakdown, a gate.
- Speed paid only once a search could spend it.
- Scale the search to the size of a move, not the size of the tour.
- Next: corner moves inside ruin and recreate.

Closing figure: `15-journey.svg` again, small.

## Prompt for Fable

> Build a Google Slides presentation from `deck-brief.md`, slide by slide as
> listed. Each figure it names is in `figures/png/` (and `figures/svg/`); use
> them as the main visual of each slide, or as references to redraw more
> beautifully while keeping what they show. Build the animations listed under
> "Animations" from `figures/frames/png/`. Put each slide's "Notes" in the
> speaker notes. Explain the problem in plain words with no jargon, keep body
> text to two short lines per slide, and show each toy number large next to its
> figure. Draw the charts from the data given. For background, read
> `docs/timeline.md`, `docs/how-it-works.md`, `docs/python-vs-rust.md` and
> `docs/PROGRESS.md`.
