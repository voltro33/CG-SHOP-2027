# The problem in one screen

CG:SHOP 2027 asks: given a grid region to mow and `k` identical cutters, plan one closed tour per cutter so
that together they sweep every cell, and the **longest** tour is as short as possible.

```
input                                  output
------------------------------------   ------------------------------------
region:  a polyomino, maybe with holes   k closed tours of the cutter's center
cutter:  a small polyomino + center      consecutive points axis-parallel
k:       number of cutters (1, 2 or 3)   all cells swept by some cutter
```

Rules that matter:

- **Moves are axis-parallel.** Consecutive points share an x or a y. The last point returns to the first.
- **Cutters may leave the region** and do not interfere with each other.
- **A one-point tour** means that cutter stands still (it still sweeps its stamp).
- **Objective:** `solution.max_tour_length`. Minimize it. Balancing work across cutters matters.
- **Scoring:** per instance roughly `(best of all teams / ours)^2`, summed. Check the Rules tab on the site
  for the exact formula.
- **Deadline:** Jan 31, 2027, 11:59 p.m. AoE.

```python
from cgshop2027_pyutils.verify import check_for_errors

errors = check_for_errors(instance, solution)   # [] means feasible
score_value = solution.max_tour_length            # smaller is better
```

How the solver attacks it: `docs/concepts/strips.md`, `partitioning.md`, `ordering.md`, `parallel-mining.md`.
