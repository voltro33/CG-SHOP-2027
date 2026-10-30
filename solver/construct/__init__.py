"""
Constructors build one open walk that sweeps (most of) the region. The rest of
the pipeline repairs, splits and polishes it, so a new idea only has to
produce a walk and be registered here.
"""

from collections.abc import Callable

from ..model import Problem
from ..tours import Point
from .cells import segments_cells
from .cover_tsp import cover_tsp
from .groups import groups_horizontal, groups_vertical
from .lanes import lanes_horizontal, lanes_vertical
from .mixed import segments_mixed
from .segments import segments_horizontal, segments_vertical

Constructor = Callable[[Problem], list[Point]]

CONSTRUCTORS: dict[str, Constructor] = {
    "lanes-h": lanes_horizontal,
    "lanes-v": lanes_vertical,
    "cover-tsp": cover_tsp,
    "segments-h": segments_horizontal,
    "segments-v": segments_vertical,
    "segments-mixed": segments_mixed,
    "segments-cells": segments_cells,
}

# these build the k closed tours themselves instead of one walk to split
GroupConstructor = Callable[[Problem], list[list[Point]]]

GROUP_CONSTRUCTORS: dict[str, GroupConstructor] = {
    "groups-h": groups_horizontal,
    "groups-v": groups_vertical,
}

__all__ = ["CONSTRUCTORS", "GROUP_CONSTRUCTORS", "Constructor", "GroupConstructor"]
