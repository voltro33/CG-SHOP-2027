"""
The official verifier as a long-lived process, for the Rust `mow-verify` crate.

Reads one JSON request per line on stdin:

    {"instance": "<path to an instance json>", "solution": {...}}

and answers each with one JSON line on stdout:

    {"errors": [...], "objective": <int or null>}

An empty `errors` list means feasible. When the request itself cannot be
served (bad JSON, an instance that does not load), the answer carries a
`fault` instead, so the caller can tell a broken request from a bad solution.

Validators are cached by instance path: rasterizing a big instance is the
slow part, and a miner verifies many solutions of the same instance. Only the
last `CACHE_SIZE` instances stay cached, so memory stays flat however many
instances one process sees.
"""

import json
import sys
from collections import OrderedDict
from typing import Any

from pydantic import ValidationError

from cgshop2027_pyutils.io import read_instance
from cgshop2027_pyutils.schemas import CGSHOP2027Solution
from cgshop2027_pyutils.verify import SolutionValidator

# a validator of the largest instance holds about 300 MB
CACHE_SIZE = 4

_validators: OrderedDict[str, SolutionValidator] = OrderedDict()


def validator_for(path: str) -> SolutionValidator:
    """
    The cached validator of `path`, least recently used ones dropped first.
    """
    validator = _validators.get(path)
    if validator is not None:
        _validators.move_to_end(path)
        return validator

    validator = SolutionValidator(read_instance(path))
    _validators[path] = validator

    while len(_validators) > CACHE_SIZE:
        _validators.popitem(last=False)

    return validator


def answer(request: dict[str, Any]) -> dict[str, Any]:
    try:
        validator = validator_for(request["instance"])
    except (OSError, KeyError, TypeError, ValueError) as error:
        return {"fault": f"cannot load the instance: {error}"}

    try:
        solution = CGSHOP2027Solution.model_validate(request.get("solution"))
    except ValidationError as error:
        return {"errors": [f"invalid solution: {error}"], "objective": None}

    errors = validator.check_for_errors(solution)
    objective = solution.objective if not errors else None

    return {"errors": errors, "objective": objective}


def serve_line(line: str) -> dict[str, Any]:
    try:
        request = json.loads(line)
    except json.JSONDecodeError as error:
        return {"fault": f"bad request JSON: {error}"}

    if not isinstance(request, dict):
        return {"fault": "the request must be a JSON object"}

    # one bad request must never end the process the caller relies on
    try:
        return answer(request)
    except Exception as error:
        return {"fault": f"{type(error).__name__}: {error}"}


def main() -> int:
    for line in sys.stdin:
        if not line.strip():
            continue

        sys.stdout.write(json.dumps(serve_line(line)) + "\n")
        sys.stdout.flush()

    return 0


if __name__ == "__main__":
    sys.exit(main())
