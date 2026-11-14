"""
Shared by the polish exporters: loading a stored best, the canonical text of
a CP-SAT model, and the hash both Python and Rust compute over text.

The canonical form of a model, one line each:

    v lo hi                          every variable, in order
    L <enforcement> | <terms> | <domain>
    B <enforcement> | <literals>
    O <terms> | <offset>
    H <var:value ...>                the hint, in the order it was given

Literals are written as in the proto (`var`, or `-var - 1` when negated),
enforcement literals and bool_or literals sorted. Terms are `var:coefficient`
with each variable once, sorted, zeros dropped. Constraints keep their order.
"""

from pathlib import Path

import numpy as np
from solver.model import Problem
from solver.tours import densify

from cgshop2027_pyutils.instance_database import InstanceDatabase
from cgshop2027_pyutils.io import read_solution

DATA = Path("data")
INSTANCES = DATA / "instances" / "challenge_v0" / "instances"
BEST = DATA / "solutions" / "best"

FNV_OFFSET = 0xCBF29CE484222325
FNV_PRIME = 0x100000001B3
MASK = (1 << 64) - 1


def load(uid: str, path: Path | None = None) -> tuple[Problem, list[np.ndarray], str]:
    """
    The problem, the dense closed tours of its stored best (or of the
    solution at `path`) and their corners as text: one line per tour.
    """
    database = InstanceDatabase(str(INSTANCES))
    problem = Problem.from_instance(database[uid])
    solution = read_solution(path or BEST / f"{uid}.solution.json")

    tours = []
    lines = []
    for tour in solution.tours:
        corners = list(zip(tour.x, tour.y, strict=True))
        tours.append(densify(corners, closed=True))
        lines.append(" ".join(f"{x} {y}" for x, y in corners))

    return problem, tours, "\n".join(lines) + "\n"


def fnv(text: str) -> str:
    value = FNV_OFFSET
    for byte in text.encode():
        value = ((value ^ byte) * FNV_PRIME) & MASK

    return f"{value:016x}"


def _terms(variables, coefficients) -> str:
    merged: dict[int, int] = {}
    for var, coefficient in zip(variables, coefficients, strict=True):
        merged[var] = merged.get(var, 0) + coefficient

    parts = [f"{var}:{c}" for var, c in sorted(merged.items()) if c != 0]
    return " ".join(parts)


def _literals(literals) -> str:
    return " ".join(str(literal) for literal in sorted(literals))


def canonical(proto) -> str:
    lines = ["v " + " ".join(str(b) for b in var.domain) for var in proto.variables]

    for constraint in proto.constraints:
        enforcement = _literals(constraint.enforcement_literal)

        if constraint.has_linear():
            linear = constraint.linear
            terms = _terms(linear.vars, linear.coeffs)
            domain = " ".join(str(bound) for bound in linear.domain)
            lines.append(f"L {enforcement} | {terms} | {domain}")
        elif constraint.has_bool_or():
            lines.append(f"B {enforcement} | {_literals(constraint.bool_or.literals)}")
        else:
            msg = f"unexpected constraint {constraint}"
            raise ValueError(msg)

    objective = proto.objective
    offset = int(objective.offset)
    lines.append(f"O {_terms(objective.vars, objective.coeffs)} | {offset}")

    hint = proto.solution_hint
    pairs = zip(hint.vars, hint.values, strict=True)
    lines.append("H " + " ".join(f"{var}:{value}" for var, value in pairs))

    return "\n".join(lines) + "\n"


def cells_text(cells: np.ndarray) -> str:
    """
    Points as `x y` lines, in the order given.
    """
    return "".join(f"{x} {y}\n" for x, y in cells.tolist())
