"""
Better segment orders by local search over spatial neighbours.

The links between lane segments are pure overhead: 14 to 44% of a walk on
the challenge instances we measured. The nearest neighbour order plus a
windowed 2-opt leaves a lot on the table, mostly long jumps back to
stragglers. Two classic moves fix much of that:

- **2-opt**: reverse a block of segments, which also flips the driving
  direction of each one. Only the two links at the block ends change.
- **Or-opt**: take one segment out and put it back between two others,
  driven whichever way is shorter.

Instead of trying every pair, each move only looks at segments whose ends
are among the few nearest ones in space, which is what makes this fast
enough for tens of thousands of segments.

With `closed=True` the order is a cycle, the drive from the last segment
back to the first counts too. That is the right goal when the walk becomes
a single closed tour.
"""

import random
import time

import numpy as np

__all__ = ["improve_order", "nearest_ends", "order_length"]

Pt = tuple[int, int]

NEIGHBOURS = 8

# how far apart, in positions, the cuts of one double bridge kick may be
DOUBLE_BRIDGE_WINDOW = 30


def d(a: Pt, b: Pt) -> int:
    return abs(a[0] - b[0]) + abs(a[1] - b[1])


def order_length(entries: list[Pt], exits: list[Pt], closed: bool) -> int:
    """
    Total link length, not counting the segments themselves.
    """
    total = 0
    for i in range(len(entries) - 1):
        total += d(exits[i], entries[i + 1])

    if closed and len(entries) > 1:
        total += d(exits[-1], entries[0])

    return total


def nearest_ends(left: np.ndarray, right: np.ndarray, count: int) -> list[list[int]]:
    """
    For every segment, the `count` other segments with the closest end
    point to either of its own ends. Computed in chunks to keep memory flat.
    """
    n = len(left)
    count = min(count, n - 1)
    if count <= 0:
        return [[] for _ in range(n)]

    ends = np.concatenate([left, right])
    owner = np.concatenate([np.arange(n), np.arange(n)])

    result: list[list[int]] = []
    chunk = max(1, 4_000_000 // (2 * n))

    for start in range(0, n, chunk):
        stop = min(n, start + chunk)

        # distance from each end of these segments to every end of all segments
        from_left = np.abs(left[start:stop, None, :] - ends[None, :, :]).sum(axis=2)
        from_right = np.abs(right[start:stop, None, :] - ends[None, :, :]).sum(axis=2)
        closest = np.minimum(from_left, from_right)

        # a segment is never its own neighbour
        rows = np.arange(stop - start)
        closest[rows, rows + start] = np.iinfo(closest.dtype).max
        closest[rows, rows + start + n] = np.iinfo(closest.dtype).max

        # a few extra candidates, since both ends of one segment may show up
        picks = min(2 * count + 2, 2 * n - 2)
        nearest = np.argpartition(closest, picks - 1, axis=1)[:, :picks]

        for row in range(stop - start):
            seen: list[int] = []
            ranked = nearest[row][np.argsort(closest[row, nearest[row]])]

            for end_index in ranked.tolist():
                segment = int(owner[end_index])
                if segment not in seen:
                    seen.append(segment)
                if len(seen) == count:
                    break

            result.append(seen)

    return result


def improve_order(
    entries: np.ndarray,
    exits: np.ndarray,
    *,
    closed: bool,
    deadline: float,
    kick_seconds: float = 3.0,
    seed: int = 0,
) -> tuple[np.ndarray, np.ndarray]:
    """
    Applies improving 2-opt and Or-opt moves until none is left, then keeps
    kicking the order and descending again for up to `kick_seconds`.
    Returns new arrays, the inputs stay untouched.
    """
    n = len(entries)
    if n < 3:
        return entries.copy(), exits.copy()

    # plain lists of tuples are much faster than numpy for one move at a time
    ent: list[Pt] = [tuple(p) for p in entries.tolist()]
    ext: list[Pt] = [tuple(p) for p in exits.tolist()]

    # segment ids stay fixed, positions change, so keep a map both ways
    seg_at = list(range(n))
    pos_of = list(range(n))

    lows = [min(a, b) for a, b in zip(ent, ext, strict=True)]
    highs = [max(a, b) for a, b in zip(ent, ext, strict=True)]
    neighbours = nearest_ends(np.array(lows), np.array(highs), NEIGHBOURS)

    state = _State(ent, ext, seg_at, pos_of, neighbours, closed)

    # first a plain descent from the given order
    state.descend(list(range(n)), deadline)
    state.remember_if_better()

    # then kick and descend again, keeping whatever is best
    stop = min(deadline, time.monotonic() + kick_seconds)
    state.iterate(stop, seed)

    ent, ext = state.best_ent, state.best_ext

    return np.array(ent, dtype=np.int64), np.array(ext, dtype=np.int64)


class _State:
    """
    The order being improved, plus the best order seen so far. This is
    working state owned by one `improve_order` call, so it changes in place.
    """

    def __init__(self, ent, ext, seg_at, pos_of, neighbours, closed) -> None:
        self.ent = ent
        self.ext = ext
        self.seg_at = seg_at
        self.pos_of = pos_of
        self.neighbours = neighbours
        self.closed = closed

        self.best_cost = order_length(ent, ext, closed)
        self.best_ent = list(ent)
        self.best_ext = list(ext)
        self.best_seg_at = list(seg_at)

    def descend(self, queue: list[int], deadline: float) -> None:
        """
        Applies improving moves until no queued segment has one left. A
        move puts the segments around it back on the queue, since their
        surroundings just changed.
        """
        n = len(self.ent)
        queued = set(queue)
        checks = 0

        while queue:
            checks += 1
            if checks % 256 == 0 and time.monotonic() >= deadline:
                return

            segment = queue.pop()
            queued.discard(segment)

            args = (self.ent, self.ext, self.seg_at, self.pos_of, self.neighbours, self.closed)
            touched = _try_two_opt(segment, *args)
            if touched is None:
                touched = _try_or_opt(segment, *args)
            if touched is None:
                continue

            # look again at the moved segment and everything next to the change
            again = [segment]
            for position in touched:
                again.append(self.seg_at[position % n])

            for other in again:
                if other not in queued:
                    queued.add(other)
                    queue.append(other)

    def remember_if_better(self) -> int:
        """
        Snapshots the current order if it beats the best one. Returns the
        current cost either way.
        """
        cost = order_length(self.ent, self.ext, self.closed)
        if cost < self.best_cost:
            self.best_cost = cost
            self.best_ent = list(self.ent)
            self.best_ext = list(self.ext)
            self.best_seg_at = list(self.seg_at)

        return cost

    def iterate(self, stop: float, seed: int) -> None:
        """
        Iterated local search: kick, descend, keep if better, else go back.
        """
        rng = random.Random(seed)
        n = len(self.ent)

        while time.monotonic() < stop:
            touched = self._kick(rng)
            self.descend(touched, stop)

            cost = self.remember_if_better()
            if cost > self.best_cost:
                # undo the kick by going back to the best order
                self.ent[:] = self.best_ent
                self.ext[:] = self.best_ext
                self.seg_at[:] = self.best_seg_at
                for position in range(n):
                    self.pos_of[self.seg_at[position]] = position

    def _kick(self, rng: random.Random) -> list[int]:
        """
        A random change the descent would not make on its own, because it
        does not pay off right away. Half the time a block reversal between
        spatial neighbours, half the time a local double bridge. Returns the
        segments to re-check.
        """
        if rng.random() < 0.5:
            return self._kick_reverse(rng)

        return self._kick_double_bridge(rng)

    def _kick_reverse(self, rng: random.Random) -> list[int]:
        n = len(self.ent)
        segment = rng.randrange(n)
        if not self.neighbours[segment]:
            return []

        other = rng.choice(self.neighbours[segment])
        a = min(self.pos_of[segment], self.pos_of[other])
        b = max(self.pos_of[segment], self.pos_of[other])
        if b - a < 2:
            return []

        _reverse(a + 1, b, self.ent, self.ext, self.seg_at, self.pos_of)

        return self._segments_at([a, a + 1, b, b + 1])

    def _kick_double_bridge(self, rng: random.Random) -> list[int]:
        """
        Swaps two neighbouring blocks, ..B C.. becomes ..C B.., both kept in
        their driving direction. No single 2-opt or Or-opt move undoes this
        when the blocks are longer than one segment.
        """
        n = len(self.ent)
        window = min(DOUBLE_BRIDGE_WINDOW, n - 1)
        if window < 3:
            return []

        start = rng.randrange(0, n - window)
        cuts = sorted(rng.sample(range(start + 1, start + window + 1), 3))
        first, middle, end = cuts

        block_b = slice(first, middle)
        block_c = slice(middle, end)

        self.ent[first:end] = self.ent[block_c] + self.ent[block_b]
        self.ext[first:end] = self.ext[block_c] + self.ext[block_b]
        self.seg_at[first:end] = self.seg_at[block_c] + self.seg_at[block_b]

        for position in range(first, end):
            self.pos_of[self.seg_at[position]] = position

        new_middle = first + (end - middle)
        return self._segments_at([first - 1, first, new_middle - 1, new_middle, end - 1, end])

    def _segments_at(self, positions: list[int]) -> list[int]:
        n = len(self.ent)

        segments = []
        for position in positions:
            segments.append(self.seg_at[position % n])

        return segments


def _try_two_opt(segment, ent, ext, seg_at, pos_of, neighbours, closed):
    """
    Tries reversing the block between this segment and each neighbour, so
    that their exits get linked directly.
    """
    n = len(ent)
    i = pos_of[segment]

    for other in neighbours[segment]:
        j = pos_of[other]
        a = min(i, j)
        b = max(i, j)

        # reversing a block of zero or all segments changes nothing
        if b - a < 1 or (a == 0 and b == n - 1):
            continue

        # the block is a+1..b, the links around it are a and b
        after_b = ent[(b + 1) % n] if (b + 1 < n or closed) else None

        old = d(ext[a], ent[a + 1]) + (d(ext[b], after_b) if after_b else 0)
        new = d(ext[a], ext[b]) + (d(ent[a + 1], after_b) if after_b else 0)

        if new < old:
            _reverse(a + 1, b, ent, ext, seg_at, pos_of)
            return [a, a + 1, b, b + 1]

    return None


def _reverse(first, last, ent, ext, seg_at, pos_of) -> None:
    """
    Reverses positions first..last in place and flips each segment.
    """
    block_ent = ent[first : last + 1]
    block_ext = ext[first : last + 1]
    block_ids = seg_at[first : last + 1]

    # driving the block backwards swaps every entry with its exit
    ent[first : last + 1] = block_ext[::-1]
    ext[first : last + 1] = block_ent[::-1]
    seg_at[first : last + 1] = block_ids[::-1]

    for position in range(first, last + 1):
        pos_of[seg_at[position]] = position


def _try_or_opt(segment, ent, ext, seg_at, pos_of, neighbours, closed):
    """
    Tries moving this segment next to one of its neighbours, either way round.
    """
    n = len(ent)
    p = pos_of[segment]
    s_ent, s_ext = ent[p], ext[p]

    # what we save by taking the segment out and closing the hole
    before = ext[p - 1] if (p > 0 or closed) else None
    after = ent[(p + 1) % n] if (p + 1 < n or closed) else None

    removed = 0
    if before is not None:
        removed += d(before, s_ent)
    if after is not None:
        removed += d(s_ext, after)
    if before is not None and after is not None:
        removed -= d(before, after)

    best_gain = 0
    best = None

    for other in neighbours[segment]:
        q = pos_of[other]

        # try inserting right after q and right before q
        for slot in (q, q - 1):
            if slot in (p, p - 1) or slot < -1 or slot >= n:
                continue
            if slot == -1 and closed:
                slot = n - 1
                if slot in (p, p - 1):
                    continue

            left = ext[slot] if slot >= 0 else None
            right_index = slot + 1
            if right_index >= n:
                right = ent[0] if closed else None
            else:
                right = ent[right_index]

            if right_index % n == p:
                continue

            # cost of the old link we break by inserting here
            broken = d(left, right) if (left is not None and right is not None) else 0

            for a, b in ((s_ent, s_ext), (s_ext, s_ent)):
                added = 0
                if left is not None:
                    added += d(left, a)
                if right is not None:
                    added += d(b, right)

                gain = removed - (added - broken)
                if gain > best_gain:
                    best_gain = gain
                    best = (slot, a, b)

    if best is None:
        return None

    slot, a, b = best
    _move(p, slot, a, b, ent, ext, seg_at, pos_of)
    return [p - 1, p, slot, slot + 1, slot + 2]


def _move(p, slot, a, b, ent, ext, seg_at, pos_of) -> None:
    """
    Moves the segment at position `p` to just after position `slot`, driven
    from `a` to `b`.
    """
    segment = seg_at[p]

    del ent[p]
    del ext[p]
    del seg_at[p]

    # taking the segment out shifts everything after it down by one
    if slot > p:
        slot -= 1

    ent.insert(slot + 1, a)
    ext.insert(slot + 1, b)
    seg_at.insert(slot + 1, segment)

    low = min(p, slot + 1)
    for position in range(low, len(seg_at)):
        pos_of[seg_at[position]] = position
