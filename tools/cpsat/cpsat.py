"""
CP-SAT as a long-lived process, for the Rust `mow-cpsat` crate.

Reads one JSON request per line on stdin:

    {"model": <a CpModelProto in protobuf JSON>,
     "params": {"max_time_in_seconds": 1.0, "num_workers": 1}}

and answers each with one JSON line on stdout:

    {"status": "OPTIMAL", "objective": 2.0, "values": [2, 0]}

`values` holds one value per variable when the status is OPTIMAL or
FEASIBLE, and is empty otherwise. When the request itself cannot be served
(bad JSON, a field the proto does not have, an unknown parameter), the answer
carries a `fault` instead. A model the proto can hold but CP-SAT rejects is
not a fault: its status is MODEL_INVALID.

With `"echo": true` the model is not solved but sent back as Python read it,
so the caller can check that both sides mean the same model.
"""

import json
import math
import sys
from typing import Any

from google.protobuf import json_format, text_format
from ortools.sat import cp_model_pb2
from ortools.sat.python import cp_model

# the solver parameters a request may set, and their types
PARAMS = {"max_time_in_seconds": float, "num_workers": int}


def read_model(data: Any) -> cp_model_pb2.CpModelProto:
    return json_format.ParseDict(data, cp_model_pb2.CpModelProto())


def solve(proto: cp_model_pb2.CpModelProto, params: dict[str, Any]) -> dict[str, Any]:
    # the CpModel of OR-Tools 9.15 wraps a C++ proto that only text reaches
    model = cp_model.CpModel()
    if not model.proto.parse_text_format(text_format.MessageToString(proto)):
        return {"fault": "the model does not survive the text format"}

    solver = cp_model.CpSolver()
    for name, value in params.items():
        setattr(solver.parameters, name, PARAMS[name](value))

    status = solver.solve(model)
    response = solver.response_proto

    found = status in (cp_model.OPTIMAL, cp_model.FEASIBLE)
    objective = response.objective_value

    return {
        "status": solver.status_name(status),
        "objective": objective if math.isfinite(objective) else None,
        "values": list(response.solution) if found else [],
    }


def answer(request: dict[str, Any]) -> dict[str, Any]:
    try:
        proto = read_model(request["model"])
    except (KeyError, json_format.ParseError) as error:
        return {"fault": f"bad model: {error}"}

    if request.get("echo"):
        return {"model": json_format.MessageToDict(proto)}

    params = request.get("params", {})
    if not isinstance(params, dict):
        return {"fault": "params must be a JSON object"}

    unknown = sorted(set(params) - set(PARAMS))
    if unknown:
        return {"fault": f"unknown parameters {unknown}"}

    return solve(proto, params)


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
