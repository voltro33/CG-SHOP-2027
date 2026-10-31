"""
Round 9 holds group variants for k > 1 only, and gives the big instances
twice the usual time cap.
"""

from solver.jobs import LONG_CAP_CELLS, Job, configs_for


def test_group_variants_only_for_several_cutters():
    assert configs_for(1, 50_000, 9) == []

    variants = configs_for(2, 50_000, 9)
    methods = {config["method"] for config in variants}

    assert methods == {"groups-h", "groups-v"}
    assert len(variants) == 10


def test_group_variants_never_repeat_round_8():
    round_8 = configs_for(3, 50_000, 8)
    round_9 = configs_for(3, 50_000, 9)

    def job_id(config: dict) -> str:
        return Job(instance="x", source="", cells=0, k=3, round=0, **config).job_id

    assert not {job_id(c) for c in round_8} & {job_id(c) for c in round_9}


def test_long_cap_starts_above_the_round_8_cap_problems():
    assert LONG_CAP_CELLS == 400_000
