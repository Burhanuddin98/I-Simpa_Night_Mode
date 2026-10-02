"""Tests of score3a.py on synthetic reports: python -B -m pytest -p no:cacheprovider test_score3a.py"""
import sys

sys.dont_write_bytecode = True

import copy  # noqa: E402

import pytest  # noqa: E402

import score3a  # noqa: E402

TRUTH = 1.0


def cell(cid, room, alpha, method, seeds, receivers=1, gated=True, air=False):
    return {
        "id": cid, "room": room, "alpha": alpha, "method": method, "air": air, "gated": gated,
        "bands_hz": [500],
        "reference": {"bands": [{"freq_hz": 500, "bed_air_m_per_metre": 0.001 if air else None}]},
        "t20": {"seeds": seeds, "extension": None},
    }


def seed(s, values, mc_sd, source="value"):
    """values[r] for one band."""
    return {"seed": s, "error": None,
            "t20": [[{"t": v, "mc_sd": mc_sd, "source": source}] for v in values]}


def report(cells, receivers_t20):
    rooms = {(c["room"], c["alpha"]) for c in cells}
    return {
        "cells": cells,
        "transports": [
            {"room": room, "alpha": alpha, "air_m_per_metre": None, "receivers_t20": receivers_t20}
            for room, alpha in sorted(rooms)
        ],
    }


def biased(mc_sd, bias=0.06, n=20, method="energetic"):
    seeds = [seed(s, [TRUTH * (1 + bias)], mc_sd) for s in range(1, n + 1)]
    return report([cell(f"r-a0.2-{method}-air-off", "r", 0.2, method, seeds)], [[TRUTH, 0.001]])


def test_a_planted_6_percent_bias_with_tight_mc_sd_is_wrong_silent():
    s, rows = score3a.score(biased(mc_sd=0.002))
    t = s["j2"]["energetic"]
    assert t["answered"] == 20
    assert t["wrong_silent"] == 20 and t["coverage"] == 0.0
    assert not t["holds"] and not s["pass"]
    assert s["j3"]["breaches"] == ["r-a0.2-energetic-air-off R000"]
    assert all(r["wrong_silent"] for r in rows)


def test_the_same_bias_with_a_wide_mc_sd_is_covered():
    # 2.5 * 0.03 + 0.005 * 1.06 = 0.0803 > 0.06
    s, rows = score3a.score(biased(mc_sd=0.03))
    t = s["j2"]["energetic"]
    assert t["coverage"] == 1.0 and t["wrong_silent"] == 0
    assert t["wide_covered_over_jnd"] == 20  # over 1 JND but covered: a wide range, reported
    assert s["pass"]


def test_the_boundary_is_z_times_mc_sd_plus_half_a_percent_of_the_value():
    # bias 6 %: half-width needed 0.06; 0.005 * 1.06 = 0.0053; mc_sd at the edge = (0.06 - 0.0053) / 2.5
    edge = (0.06 - 0.005 * 1.06) / 2.5
    assert score3a.covered(1.06, edge * 1.0001, TRUTH)
    assert not score3a.covered(1.06, edge * 0.9999, TRUTH)
    # Z sensitivity: covered at 3, not at 2.5 or 2.
    sd = 0.06 / 2.75
    s, _ = score3a.score(biased(mc_sd=sd))
    t = s["j2"]["energetic"]
    assert t["coverage"] == 0.0 and t["coverage_z2"] == 0.0 and t["coverage_z3"] == 1.0


def test_a_small_error_outside_the_range_is_not_covered_but_not_wrong_silent():
    s, _ = score3a.score(biased(mc_sd=0.001, bias=0.02))
    t = s["j2"]["energetic"]
    assert t["coverage"] == 0.0 and t["wrong_silent"] == 0


def test_truth_uncertainty_over_half_a_percent_leaves_the_denominators():
    r = biased(mc_sd=0.002)
    r["transports"][0]["receivers_t20"] = [[TRUTH, 0.0051]]
    s, _ = score3a.score(r)
    assert s["status_counts"] == {"excluded_truth_uncertain": 20}
    assert s["excluded_answered_rows"] == 20
    assert s["j2"]["energetic"]["answered"] == 0
    assert not s["pass"]  # no answered row: J2 cannot hold
    r["transports"][0]["receivers_t20"] = [[TRUTH, 0.0049]]
    s, _ = score3a.score(r)
    assert s["j2"]["energetic"]["answered"] == 20
    r["transports"][0]["receivers_t20"] = [None]
    s, _ = score3a.score(r)
    assert s["status_counts"] == {"excluded_truth_missing": 20}


def test_noise_refusals_are_answered_other_refusals_are_counted_by_code():
    seeds = [seed(1, [TRUTH], 0.01, "monte_carlo_noise"), seed(2, [TRUTH], 0.01, "noise_uncalibrated"),
             seed(3, [None], None, "truncated"), {"seed": 4, "error": "no run", "t20": []},
             seed(5, [TRUTH], None, "noise_uncalibrated")]
    r = report([cell("r-a0.2-random-air-off", "r", 0.2, "random", seeds)], [[TRUTH, 0.001]])
    s, _ = score3a.score(r)
    assert s["j2"]["random"]["answered"] == 3
    assert s["unanswered_by_code"] == {"truncated": 1, "run_error": 1}
    assert s["answered_without_mc_sd"] == 1


def test_per_receiver_truth_and_j3_per_cell_receiver_and_modes_apart():
    # Receiver 0's truth is 3 % low of receiver 1's; the product reads receiver 1's level at both.
    seeds = [seed(s, [1.03, 1.03], 0.002) for s in range(1, 21)]
    e = cell("big-a0.4-energetic-air-off", "big", 0.4, "energetic", seeds, gated=False)
    rnd = cell("big-a0.4-random-air-off", "big", 0.4, "random", copy.deepcopy(seeds), gated=False)
    r = report([e, rnd], [[0.97, 0.001], [1.03, 0.001]])
    s, rows = score3a.score(r)
    by = {(g["cell"], g["receiver"]): g for g in s["subgroups"]}
    assert by[("big-a0.4-energetic-air-off", 0)]["wrong_silent_rate"] == 1.0
    assert by[("big-a0.4-energetic-air-off", 1)]["wrong_silent_rate"] == 0.0
    assert s["j3"]["breaches"] == ["big-a0.4-energetic-air-off R000", "big-a0.4-random-air-off R000"]
    assert s["j2"]["energetic"]["coverage"] == 0.5 and s["j2"]["random"]["coverage"] == 0.5


def test_j3_needs_20_answered_rows():
    s, _ = score3a.score(biased(mc_sd=0.002, n=19))
    assert s["j3"]["holds"] and s["j3"]["subgroups_applying"] == 0
    assert not s["pass"]  # J2 still fails


def test_the_far_room_energetic_error_is_reported_per_receiver():
    seeds = [seed(s, [0.972, 1.0], 0.002) for s in range(1, 11)]
    r = report([cell("20x8x4-a0.4-energetic-air-off", "20x8x4", 0.4, "energetic", seeds, gated=False)],
               [[1.0, 0.001], [1.0, 0.001]])
    s, _ = score3a.score(r)
    far = s["far_room_energetic"]["per_receiver_all_alphas"]
    assert far["R000"]["mean_err"] == pytest.approx(-0.028)
    assert far["R001"]["mean_err"] == pytest.approx(0.0)


def test_the_air_on_band_reads_the_transport_at_the_bands_air():
    seeds = [seed(1, [1.2], 0.002)]
    c = cell("r-a0.2-energetic-air-on", "r", 0.2, "energetic", seeds, air=True)
    r = {"cells": [c], "transports": [
        {"room": "r", "alpha": 0.2, "air_m_per_metre": None, "receivers_t20": [[1.0, 0.001]]},
        {"room": "r", "alpha": 0.2, "air_m_per_metre": 0.001, "receivers_t20": [[1.2, 0.001]]},
    ]}
    s, rows = score3a.score(r)
    assert rows[0]["truth"] == 1.2 and rows[0]["covered"]


def test_a_report_without_the_per_receiver_field_is_refused():
    r = biased(mc_sd=0.002)
    del r["transports"][0]["receivers_t20"]
    with pytest.raises(score3a.ScoreError, match="receivers_t20"):
        score3a.score(r)
