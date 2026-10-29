# NP-hardness in plain terms

A problem is in **NP** if a proposed answer can be *checked* quickly. Ours is: the verifier checks a solution in
about a second, even for 8 million cells.

A problem is **NP-hard** if it is at least as hard as every problem in NP. Nobody knows a method that *solves*
every NP-hard instance quickly, and most researchers believe none exists (that is the P vs NP question).

Why mowing is NP-hard: with a 1x1 cutter and a region made of isolated cells far apart, a single cutter must
visit every cell and come back, which is exactly the **Travelling Salesman Problem** on a grid, a known
NP-hard problem. Our problem contains TSP as a special case, so it is at least as hard.

What that means in practice:

- **Exact methods** (CP-SAT, integer programming) can prove optimal answers on tiny instances only.
- **Heuristics** (strips, local search, mining) find good answers on big ones without proof.
- **Lower bounds** tell us how far from optimal we *might* be: the gap.

```
easy to check   ✔  verifier runs in seconds
easy to solve   ✘  no known fast exact method
so we           →  build good answers fast, improve them, and measure against a floor
```
