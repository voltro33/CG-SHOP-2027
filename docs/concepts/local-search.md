# Local search vs brute force

**Brute force** tries every possible solution. For ordering just 60 lane segments, each drivable two ways,
there are `60! * 2^60` orders, about `10^99`. No computer will ever finish.

**Local search** starts from a decent solution and repeatedly applies small changes ("moves") that make it
better. It is a hiker walking downhill in fog: always step down, stop when every step goes up.

```
length
  |\        /\
  | \  /\  /  \      <- the hiker stops in the first valley (local optimum)
  |  \/  \/    \___   while a deeper valley exists further on
  +-------------------- solutions
```

Ways out of the first valley, all used in this solver:

| technique | idea | where |
|---|---|---|
| **kicks (ILS)** | jump to a nearby random spot, walk down again, keep it if deeper | `construct/ordering.py` |
| **simulated annealing** | sometimes accept an uphill step, less often over time | `ordering.py`, `anneal` knob, mining round 6 |
| **diversity** | start from many different solutions and keep the best | the miner's config grid |

The moves in this solver:

- **Corner cut** (`improve.py`): replace `a -> b -> c` by the shortest way `a -> c` if every cell stays swept.
- **2-opt / Or-opt** (`ordering.py`): reorder and flip lane segments.
- **Rebalance** (`partition.py`): move cells from the longest tour's band to others.

Every accepted change keeps the solution feasible, and the official verifier checks the final result.
