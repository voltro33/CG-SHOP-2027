"""
The benchmark must pick the same set every time, read configs the way the
leaderboard writes them, and score a run against the bests before it.
"""

import pytest

from solver.bench import parse_config, pick_bench, score_run


def catalog_row(uid: str, cells: int, k: int, bound: int = 100) -> dict:
    return {
        "source": "instances",
        "instance": uid,
        "cells": str(cells),
        "k": str(k),
        "lower_bound": str(bound),
    }


def test_parse_config_reads_method_params_and_partition():
    config = parse_config("segments-h/gap_factor=1/shift=0.5/bands-x")

    assert config["method"] == "segments-h"
    assert config["params"] == (("gap_factor", 1.0), ("shift", 0.5))
    assert config["partition"] == "bands-x"
    assert config["seed"] == 0


def test_parse_config_takes_the_label_out_of_a_job_id():
    config = parse_config("abc_1s2|segments-v/seed=2/kick_seconds=30|s2|v2")

    assert config["method"] == "segments-v"
    assert config["seed"] == 2
    assert config["partition"] is None


def test_pick_bench_is_stratified_and_deterministic():
    catalog = []
    for index in range(6):
        catalog.append(catalog_row(f"a{index}_10k_1s2", 1000 * (index + 1), 1))
        catalog.append(catalog_row(f"b{index}_10k_2x3", 1000 * (index + 1), 2))

    # too big for the strata, but a candidate for the worst X list
    catalog.append(catalog_row("big_900k_3x5", 900_000, 3))
    leaderboard = [{"instance": "big_900k_3x5", "cells": "900000", "gap_pct": "200.0"}]

    first = pick_bench(catalog, leaderboard)
    second = pick_bench(catalog, leaderboard)

    assert first == second
    assert len(first) == 5
    assert first[-1] == "big_900k_3x5"
    assert sum(1 for uid in first if "_1s2" in uid) == 2


def test_score_run_counts_failures_as_zero():
    catalog = {
        "a_1s2": catalog_row("a_1s2", 10, 1, bound=50),
        "b_1s2": catalog_row("b_1s2", 10, 1, bound=50),
    }
    before = {"a_1s2": 100, "b_1s2": 100}
    ours = {"a_1s2": 50}

    result = score_run(["a_1s2", "b_1s2"], catalog, before, ours)

    # twice as short scores four, the failure scores nothing
    assert result["self_score"] == pytest.approx(4.0)
    assert result["feasible"] == 1
    assert result["pessimistic_score"] == pytest.approx(1.0)
    assert result["pessimistic_before"] == pytest.approx(0.5)
