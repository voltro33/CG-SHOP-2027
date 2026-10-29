# Lower bounds: the floor no solution can go below

**Paint roller analogy.** A paint roller of width `w` covers at most `w` new square units per unit
it is rolled. To paint `A` units, starting with one roller-print already on the wall, you must roll at
least `(A - print) / w`. Our cutter is the roller.

## Area bound (`solver/bounds.py: area_bound`)

- Each cutter starts already covering `|C|` cells (its stamp where it stands).
- One unit step in direction `e` newly sweeps at most `gain = max over e of |(C + e) minus C|` cells.
  For a solid rectangle this is the side perpendicular to the move; for the X-shaped cutter it is larger
  than its height, because the gaps inside the X count as new cells on a single step.
- With `k` cutters, each of length at most `L`:

```
k * |C|  +  k * L * gain  >=  cells        =>        L  >=  (cells - k * |C|) / (k * gain)
```

- Closed tours on a grid have even length, so the bound rounds up to the next even number.

## Extent bound (`extent_bound`, only for `k = 1`)

A single cutter must reach the leftmost, rightmost, lowest and highest cells and come back:
at least twice the span of the forced anchor box.

## Assumptions to watch

- The bound uses the **official coverage rule** (stamps at integer points, see `coverage.md`). That rule
  is confirmed by `tests/solver/test_coverage.py`.
- It ignores that cutters may need to travel through empty space to reach separated pieces, so it is
  loose on sparse, holey instances. Gaps of 30% to 90% are therefore not all room to improve.

`tests/solver/test_bounds.py` asserts the bound never exceeds any verified solution, shipped or mined.

## Gap

```
gap = (ours - bound) / bound
```
