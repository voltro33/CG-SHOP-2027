# What the instances look like

Source: `python -m solver.catalog` -> `data/catalog.csv` (one row per instance).

## Are cutters uniform across instances?

**No.** The last part of an instance name encodes `k` and the cutter, e.g. `bck2217_18k_1d6`
means `k = 1`, cutter family `d`, size 6. There are five families:

```
s  square           l  diamond         d  clipped square   p  plus            x  X shape
   ######              ..#..              .####.              ..###..            ##...##
   ######              .###.              ######              ..###..            ###.###
   ######              #####              ######              #######            .#####.
   ######              .###.              ######              #######            ..###..
   ######              ..#..              ######              #######            .#####.
   ######                                 .####.              ..###..            ###.###
                                                              ..###..            ##...##
```

Every cutter is symmetric and its center sits in the middle. Only the `s` family (and a few
small `x`/`d` ones that degenerate to full squares) are rectangles; **80% of challenge cutters
are not rectangles**, so code must never assume a box-shaped cutter.

| | examples (75) | challenge v0 (250) |
|---|---|---|
| `k` | 1: 22, 2: 23, 3: 30 | 1: 85, 2: 83, 3: 82 |
| cells, min / median / max | 305 / 12k / 100k | 6.3k / 104k / **8.7M** |
| instances > 1M cells | 0 | 40 |
| with holes | 60 | 178 (up to 8577 holes) |
| cutter width | 2 to 6 | 3 to 72 |
| median fill (cells / bounding box) | | 0.42 |

## What this means for the solver

- **Scale matters.** Anything quadratic in the number of cells or anchors will not finish on the
  largest instances; lanes (linear) are the safe baseline.
- **Sparse, holey regions.** With a median fill of 0.42, lanes that blindly span gaps waste length.
- **Wide cutters.** A 72-cell-wide cutter covers a lot per step; the instance is "coarse" and few lanes are needed.
