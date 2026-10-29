# Witness-based lower bounds (idea, not implemented yet)

**Pins analogy.** Stick a few pins in the lawn, far apart. The cutter must touch every pin. However cleverly
you mow, the tour is at least as long as the shortest closed route that touches all the pins. Pins are
*witnesses*.

How it becomes a bound:

1. Choose witness cells `w1 .. wm`, spread out, such that no single cutter stamp covers two of them.
2. For each witness, the anchors that can sweep it form a small set (the cell minus the cutter offsets).
3. Any tour must visit one anchor from each set, so its length is at least the shortest closed tour
   through one anchor per set: a *generalized TSP*.
4. Any lower bound on that generalized TSP (for example a minimum spanning tree over the sets,
   or an exact solve when `m` is small) is a lower bound on our objective.

With `k` cutters, the witnesses are shared out, so the floor applies to the best split of witnesses among
cutters; the max of the `k` tours is at least the total floor divided by `k` (loose) or found with a small
exact model.

Why it helps: the area bound ignores travel through empty space. On sparse, ragged instances (median fill 0.42),
witnesses at the far ends of the region force the long drives that the area bound misses.
This is listed as next step 5 in `PLAN` section 8.
