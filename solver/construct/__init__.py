"""
Constructors build one open walk that sweeps (most of) the region. The rest of
the pipeline repairs, splits and polishes it, so a new idea only has to
produce a walk and be registered here.
"""

from collections.abc import Callable

from ..model import Problem
from ..tours import Point
from .cover_tsp import cover_tsp
from .lanes import lanes_horizontal, lanes_vertical

Constructor = Callable[[Problem], list[Point]]

CONSTRUCTORS: dict[str, Constructor] = {
    "lanes-h": lanes_horizontal,
    "lanes-v": lanes_vertical,
    "cover-tsp": cover_tsp,
}

__all__ = ["CONSTRUCTORS", "Constructor"]
