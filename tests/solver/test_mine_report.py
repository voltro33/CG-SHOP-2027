"""
The mining report must credit improvements to the job that made them, and
price a config by what its absence would cost.
"""

import pytest

from solver.mine_report import config_of, config_stats, round_stats, size_band


def record(instance: str, label: str, round_number: int, objective: int | None, seed: int = 0) -> dict:
    return {
        "job_id": f"{instance}|{label}|s{seed}|v2",
        "instance": instance,
        "round": round_number,
        "label": label,
        "objective": objective,
        "seconds": 3600.0,
        "error": None,
    }


BOUNDS = {"a": 50, "b": 50}

# instance a: round 0 sets 100, round 1 improves to 80; instance b: both rounds tie at 100
LOG = [
    record("a", "segments-h", 0, 100),
    record("b", "segments-h", 0, 100),
    record("a", "segments-v", 1, 80),
    record("b", "segments-v", 1, 100),
    record("a", "lanes-h", 1, None),
]


def test_config_is_label_and_seed():
    assert config_of(record("a", "segments-h/gap_factor=0", 2, 1, seed=3)) == "segments-h/gap_factor=0|s3"
    assert config_of({"job_id": "a|segments-v|s0"}) == "segments-v|s0"


def test_only_strict_improvements_count_and_the_first_result_is_free():
    rounds = {stats.round: stats for stats in round_stats(LOG, BOUNDS)}

    assert rounds[0].improvements == 0
    assert rounds[0].points == 0.0

    # a goes from 100 to 80 against a bound of 50
    assert rounds[1].improvements == 1
    assert rounds[1].points == pytest.approx((50 / 80) ** 2 - (50 / 100) ** 2)
    assert rounds[1].jobs == 3
    assert rounds[1].cpu_hours == pytest.approx(3.0)


def test_points_at_stake_only_for_sole_winners():
    configs = {stats.config: stats for stats in config_stats(LOG, BOUNDS)}

    vertical = configs["segments-v|s0"]
    assert vertical.wins == 2
    assert vertical.sole_wins == 1
    assert vertical.points_at_stake == pytest.approx((50 / 80) ** 2 - (50 / 100) ** 2)

    # a tie on b costs nothing to drop
    assert configs["segments-h|s0"].wins == 1
    assert configs["segments-h|s0"].points_at_stake == 0.0

    # a config that only failed never wins
    assert configs["lanes-h|s0"].wins == 0


def test_size_bands_cover_every_size():
    assert size_band(10) == "<50k"
    assert size_band(50_000) == "50k-400k"
    assert size_band(10**13) == ">2M"
