"""Tests of the sets B and C scorer's pieces: python -B -m pytest -p no:cacheprovider test_scorebc.py"""
import sys

sys.dont_write_bytecode = True

import math  # noqa: E402

import numpy as np  # noqa: E402
import pytest  # noqa: E402

import scorebc as sb  # noqa: E402
import truthbed as tb  # noqa: E402
from test_truthbed import exp_bins  # noqa: E402


def test_classify_follows_the_prereg():
    # inside its range and close: not wrong-silent, covered
    assert sb.classify(5.0, 4.5, 5.5, 5.2, 1.0) == dict(answered=True, wrong_silent=False, covered=True)
    # 2 limens off but the truth inside the shown range: not wrong-silent (shown honestly), covered
    assert sb.classify(5.0, 2.0, 8.0, 7.0, 1.0)['wrong_silent'] is False
    # 2 limens off, the truth outside the range: wrong-silent, not covered
    r = sb.classify(5.0, 4.5, 5.5, 7.0, 1.0)
    assert r['wrong_silent'] and not r['covered']
    # just outside the range but within its 1/10-limen widening: covered, not wrong-silent
    r = sb.classify(5.0, 4.5, 5.5, 5.58, 1.0)
    assert r['covered'] and not r['wrong_silent']
    # no range: the value alone decides both
    assert sb.classify(5.0, None, None, 6.5, 1.0)['wrong_silent'] is True
    assert sb.classify(5.0, None, None, 5.09, 1.0)['covered'] is True
    assert sb.classify(5.0, None, None, 5.2, 1.0) == dict(answered=True, wrong_silent=False, covered=False)
    # refused: neither
    assert sb.classify(None, None, None, 5.0, 1.0) == dict(answered=False, wrong_silent=False, covered=False)


def test_read_param_shapes():
    assert sb.read_param({'value': 1.5, 'mc_sd': 0.1, 'status': 'wide', 'lo': 1.0, 'hi': 2.0}) == (1.5, 1.0, 2.0, 'wide', None)
    ref = {'not_evaluable': {'code': 'params_not_evaluable', 'message': 'x',
                             'error': {'kind': 'not_evaluable', 'why': {'why': 'monte_carlo_noise'}}}}
    assert sb.read_param(ref) == (None, None, None, 'refused', 'params_not_evaluable:monte_carlo_noise')
    assert sb.read_param({'truncated': {'code': 'truncated'}})[4] == 'truncated'
    assert sb.read_param(None)[3] == 'absent'


def test_exclusion():
    lim = lambda t: tb.limen('c80', t)                       # noqa: E731
    assert sb.exclusion(3.0, 0.05, lim) is None
    assert sb.exclusion(3.0, 0.11, lim) == 'truth_uncertain'
    assert sb.exclusion(float('nan'), 0.0, lim) == 'truth_nan'
    assert sb.exclusion(3.0, 0.0, lim, tail_ok=False) == 'truth_truncated'
    lim20 = lambda t: tb.limen('t20', t)                     # noqa: E731
    assert sb.exclusion(2.0, 0.009, lim20) is None           # 1/10 of 5 % of 2 s = 10 ms
    assert sb.exclusion(2.0, 0.011, lim20) == 'truth_uncertain'


def spps_like(dt, t0, h, Ed, k, gap, T):
    """A direct sound spread evenly over [t0 - h, t0 + h] plus an exponential from t0 + gap, binned exactly."""
    n = int(round(T / dt))
    e = np.arange(n + 1) * dt
    lo, hi = e[:-1], e[1:]
    ov = np.clip(np.minimum(hi, t0 + h) - np.maximum(lo, t0 - h), 0, None) / (2 * h)
    return Ed * ov + exp_bins(t0, k, dt, T, gap)


@pytest.mark.parametrize('t60', [0.3, 1.0, 3.0])
def test_metrics_spps_reads_the_direct_window_at_u0(t60):
    """Set B's truth on a 0.5 ms histogram whose reverberation starts after the ball's direct sound (gap 3 ms > R/c
    0.9 ms): every metric is the closed form's (direct at u = 0) within what even bins cost, 1/100 limen."""
    dt, t0, h, gap = 0.5e-3, 0.01234, 0.9e-3, 3e-3
    k = 6 * math.log(10) / t60
    Ed = 0.3 / k
    b = spps_like(dt, t0, h, Ed, k, gap, t0 + 30 * t60 / 6)
    got = sb.metrics_spps(b, dt, t0, h)
    want = tb.closed_form(Ed, gap, [1.0], [k])
    for m in tb.METRICS:
        assert abs(got[m] - want[m]) < tb.LIMEN[m] / 100, (m, got[m], want[m])
    assert got['t20'] == pytest.approx(t60, rel=1e-3)
    assert got['t30'] == pytest.approx(t60, rel=1e-3)


def test_truth_b_pools_and_halves_the_difference():
    dt, t0, h = 0.5e-3, 0.01, 0.9e-3
    k = 6 * math.log(10) / 0.5
    a = spps_like(dt, t0, h, 0.1, k, 2e-3, 5.0)
    tr = sb.truth_b(a, 2 * a, dt, t0, h)
    assert tr['spl'][1] == pytest.approx(10 * math.log10(2) / 2)
    assert tr['spl'][0] == pytest.approx(tb.binned(0.0, 1.5 * a, dt, t0, t0 + h)['spl'])
    for m in ('c50', 'c80', 'd50', 'ts', 't20'):
        assert tr[m][1] == pytest.approx(0.0, abs=1e-9), m
    # two different seeds: the pooled metric is the mean series', not the mean of the metrics
    b = spps_like(dt, t0, h, 0.2, k, 2e-3, 5.0)
    tr = sb.truth_b(a, b, dt, t0, h)
    m1, m2 = sb.metrics_spps(a, dt, t0, h), sb.metrics_spps(b, dt, t0, h)
    assert tr['c80'][1] == pytest.approx(abs(m1['c80'] - m2['c80']) / 2)
    assert tr['c80'][0] == pytest.approx(sb.metrics_spps(0.5 * (a + b), dt, t0, h)['c80'])


def test_score_counts_wrong_silent_coverage_and_exclusions():
    base = dict(room='G1', label='R000', band_hz=1000, seed=4101, metric='c80', lo=None, hi=None, status='ok',
                code=None, u=0.0, excluded=None, design_t60=0.5)
    rows = [dict(base, value=5.0, truth=5.05),                              # covered
            dict(base, value=5.0, truth=5.5, lo=4.6, hi=5.4),               # covered by the widening
            dict(base, value=5.0, truth=7.0, lo=4.5, hi=5.5),               # wrong-silent
            dict(base, value=None, truth=5.0, status='refused', code='truncated'),
            dict(base, value=5.0, truth=5.0, u=0.2, excluded='truth_uncertain')]
    s = sb.score(rows)['c80']
    assert (s['rows'], s['scored'], s['answered'], s['wrong_silent'], s['covered']) == (5, 4, 3, 1, 2)
    assert s['excluded'] == {'truth_uncertain': 1}
    assert s['refusals'] == {'truncated': 1}
    assert s['answered_share'] == pytest.approx(0.75)
    assert s['by_room']['G1']['flag_below_80'] is True
    assert s['pass_'] is False
