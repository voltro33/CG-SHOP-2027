"""
Partition first, then route: give every cutter its own band of the region.

The default pipeline builds one walk over everything and cuts it into `k`
pieces afterwards. Here we do it the other way round:

1. Cut the region into `k` bands by straight lines across one axis, so
   each band holds a chosen share of the cells (equal shares at first).
2. Solve each band as a single-cutter problem.
3. Rebalance: a band whose tour came out long gets a smaller share next
   round, a short one a bigger share. Keep the round with the shortest
   longest tour, since that is the objective.
"""

from collections.abc import Callable

import numpy as np

from cgshop2027_pyutils.grid import CellSet

from .model import Problem
from .tours import Point, route_length

__all__ = ["BandSolver", "band_problems", "solve_in_bands"]


# solves a single cutter problem and returns its closed tour corners
BandSolver = Callable[[Problem], list[Point]]


def band_problems(problem: Problem, shares: list[float], axis: str) -> list[Problem]:
    """
    Splits the region into `len(shares)` bands with roughly those shares of
    its cells. `axis="y"` cuts along rows into horizontal bands stacked
    bottom to top, `axis="x"` cuts along columns into vertical bands.
    """
    mask = problem.region.mask
    origin = problem.region.origin

    # cells per row for horizontal bands, cells per column for vertical bands
    if axis == "y":
        line_counts = mask.sum(axis=1)
    else:
        line_counts = mask.sum(axis=0)

    cumulative = np.cumsum(line_counts)
    total = int(cumulative[-1])

    # where each band ends, as a cell count reached so far
    targets = np.cumsum(shares)[:-1] / sum(shares) * total
    cut_lines = np.searchsorted(cumulative, targets, side="left") + 1

    bounds = [0, *cut_lines.tolist(), len(line_counts)]

    bands = []
    for start, stop in zip(bounds[:-1], bounds[1:], strict=True):
        # crop to the band so its bounds and frames stay small
        if axis == "y":
            band_mask = mask[start:stop, :].copy()
            band_origin = (origin[0], origin[1] + start)
        else:
            band_mask = mask[:, start:stop].copy()
            band_origin = (origin[0] + start, origin[1])

        # a band can come out empty on tiny or very uneven regions
        if not band_mask.any():
            continue

        region = CellSet(band_mask, band_origin)
        bands.append(Problem(uid=problem.uid, region=region, cutter=problem.cutter, k=1))

    return bands


def solve_in_bands(
    problem: Problem,
    solve_band: BandSolver,
    *,
    axis: str,
    rounds: int,
    out_of_time: Callable[[], bool],
) -> list[list[Point]]:
    """
    Closed tours, one per band, from the best of up to `rounds` rebalancing
    rounds. Always runs at least one round.
    """
    k = problem.k
    shares = [1.0] * k

    best_tours: list[list[Point]] = []
    best_longest = None

    for round_number in range(rounds):
        if round_number > 0 and out_of_time():
            break

        bands = band_problems(problem, shares, axis)

        tours = []
        for band in bands:
            tours.append(solve_band(band))

        lengths = []
        for tour in tours:
            lengths.append(max(1, route_length(tour, closed=True)))

        longest = max(lengths)
        if best_longest is None or longest < best_longest:
            best_longest = longest
            best_tours = tours

        # bands may have merged away, then shares no longer line up
        if len(bands) != k:
            break

        shares = _rebalanced(shares, lengths)

    return best_tours


def _rebalanced(shares: list[float], lengths: list[int]) -> list[float]:
    """
    New shares that move work away from long tours. Tour length grows
    roughly with area, so scaling a share by `mean / length` aims every
    band at the mean. The square root damps the step so rounds do not
    overshoot back and forth.
    """
    mean = sum(lengths) / len(lengths)

    new_shares = []
    for share, length in zip(shares, lengths, strict=True):
        factor = (mean / length) ** 0.5
        new_shares.append(share * factor)

    return new_shares
