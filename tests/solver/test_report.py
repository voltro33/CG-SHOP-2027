"""
The report must score instances the way the contest does, using the lower
bound in place of the best team.
"""

import pytest

from solver.report import family, median_gap_by_family, pessimistic_score, summary


def make_row(uid: str, bound: int, best: int, baseline: int | None = None) -> dict:
    gap = round(100.0 * (best - bound) / bound, 1)

    return {
        "instance": uid,
        "lower_bound": bound,
        "best": best,
        "baseline": baseline,
        "gap_pct": gap,
        "vs_baseline_pct": None,
        "score_gain": None,
    }


def test_family_reads_the_cutter_letter():
    assert family("ara238025_353k_1l5") == "diamond"
    assert family("bby34656_1372k_3x3") == "X"
    assert family("fht47608_34k_2p4") == "plus"
    assert family("srpg2839_25k_1d7b") == "clipped square"
    assert family("spg-c-poly6002_28k_1s4") == "square"
    assert family("no_suffix_here") == "unknown"


def test_pessimistic_score_is_squared_bound_ratio():
    assert pessimistic_score(50, 100) == pytest.approx(0.25)
    assert pessimistic_score(100, 100) == pytest.approx(1.0)
    assert pessimistic_score(0, 100) is None


def test_summary_sums_pessimistic_scores():
    rows = [
        make_row("a_1s2", 50, 100),
        make_row("b_1s2", 90, 100),
        make_row("c_2x3", 10, 40),
    ]

    result = summary(rows, total_instances=3)

    # 0.25 plus 0.81 plus 0.0625
    assert result["pessimistic_score"] == 1.1


def test_median_gap_by_family_sorts_widest_first():
    rows = [
        make_row("a_1s2", 100, 110),
        make_row("b_1s2", 100, 130),
        make_row("c_2x3", 100, 300),
    ]

    medians = median_gap_by_family(rows)

    assert list(medians) == ["X", "square"]
    assert medians["square"] == 20.0
    assert medians["X"] == 200.0
