# Strips and boustrophedon sweeps

**Boustrophedon** ("as the ox turns") means mowing back and forth in parallel strips, like plowing a field.

```
 lane 4  <-----------------------
                                 |
 lane 3   ----------------------->
         |
 lane 2  <-----------------------
                                 |
 lane 1   ----------------------->
```

A cutter that is `h` cells tall, driven horizontally, sweeps a band of `h` rows. Stack the lanes `h` apart
and nothing is missed. For a cutter row spanning `lx..rx` around the center, driving the center from `x = a`
to `x = b` sweeps cells `a + lx .. b + rx` in that row, so a lane only needs to run as far as the region in
its band demands.

## Plain lanes vs lane segments

| | `lanes-h/v` (`solver/construct/lanes.py`) | `segments-h/v` (`solver/construct/segments.py`) |
|---|---|---|
| a lane is | one stretch, leftmost to rightmost region cell | several **segments**, one per stretch that has cells |
| gaps | always crossed | crossed only if shorter than `max_gap` (default 2 lane heights) |
| order | fixed snake, bottom to top | nearest neighbour + 2-opt, either driving direction |

```python
# a run of region cells s..e in cutter row dy needs the anchor interval
a = s - lx      # first anchor whose stamp reaches s
b = e - rx      # last anchor needed to reach e
```

On a sample of 33 challenge instances, segments beat plain lanes on most of them, by up to 32%
(see `PROGRESS.md`).

## Why turns cost length

Moving one unit along a lane sweeps up to `h` new cells. Moving one unit *across* to the next lane sweeps
almost nothing new (that band is about to be mowed anyway). So every lane change is length spent without
progress: about one lane height per turn. Fewer, longer lanes means less turn overhead, which is why the
miner tries both orientations: lanes along the long side of a shape usually win.
