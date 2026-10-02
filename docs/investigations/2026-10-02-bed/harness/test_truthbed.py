"""Tests of the bed's truths (truthbed.py): python -B -m pytest -p no:cacheprovider test_truthbed.py

Hand-checked numbers are worked in the docstrings; the rest compare the binned truth with closed forms."""
import sys

sys.dont_write_bytecode = True

import math  # noqa: E402

import numpy as np  # noqa: E402
import pytest  # noqa: E402

import truthbed as tb  # noqa: E402

LN10 = math.log(10)


def exp_bins(t0, k, dt, T, gap=0.0, A=1.0):
    """Exact per-bin integrals of A exp(-k (t - t0 - gap)) for t >= t0 + gap, bins [n dt, (n+1) dt)."""
    n = int(round(T / dt))
    e = np.arange(n + 1) * dt
    ts = t0 + gap
    lo = np.maximum(e[:-1], ts)
    hi = np.maximum(e[1:], lo)
    return A / k * (np.exp(-k * (lo - ts)) - np.exp(-k * (hi - ts)))


def test_hand_checked_pure_exponential_t60_1s():
    """T60 1 s, k = 6 ln10 = 13.8155 /s, no direct sound, from t0:
    C80 = 10 lg(e^{0.08 k} - 1) = 10 lg(3.019952 - 1) = 3.0534 dB; C50 = 10 lg(e^{0.690776} - 1) = 10 lg(0.995262)
    = -0.0206 dB; D50 = 1 - e^{-0.690776} = 0.49881; Ts = 1/k = 72.382 ms."""
    k = 6 * LN10
    r = tb.closed_form(0.0, 0.0, [1.0], [k])
    assert r['c80'] == pytest.approx(3.0534, abs=1e-4)
    assert r['c50'] == pytest.approx(-0.0206, abs=1e-4)
    assert r['d50'] == pytest.approx(0.49881, abs=1e-5)
    assert r['ts'] == pytest.approx(0.072382, abs=1e-6)
    b = exp_bins(0.01234, k, 1e-5, 12.0)
    r2 = tb.binned(0.0, b, 1e-5, 0.01234)
    for m in tb.METRICS:
        assert r2[m] == pytest.approx(r[m], rel=1e-6, abs=1e-9), m


@pytest.mark.parametrize('t60', [0.1, 0.3, 1.0, 3.0, 10.0])
@pytest.mark.parametrize('drr_db', [-20.0, 0.0, 10.0])
def test_direct_plus_exponential_closed_forms(t60, drr_db):
    """Ed + exponential from t0: C_te = 10 lg((Ed + (1 - e^{-k te})/k) / (e^{-k te}/k)), D50 = (Ed + (1 - e^{-k 0.05})/k)
    / (Ed + 1/k), Ts = (1/k^2) / (Ed + 1/k), SPL = 10 lg((Ed + 1/k) / 4e-10); the binned truth on a 0.02 ms grid."""
    k = 6 * LN10 / t60
    Ed = 10 ** (drr_db / 10) / k
    tot = Ed + 1 / k
    want = dict(c50=10 * math.log10((Ed + (1 - math.exp(-k * .05)) / k) / (math.exp(-k * .05) / k)),
                c80=10 * math.log10((Ed + (1 - math.exp(-k * .08)) / k) / (math.exp(-k * .08) / k)),
                d50=(Ed + (1 - math.exp(-k * .05)) / k) / tot, ts=(1 / k ** 2) / tot,
                spl=10 * math.log10(tot / 4e-10))
    cf = tb.closed_form(Ed, 0.0, [1.0], [k])
    t0 = 0.0173
    b = exp_bins(t0, k, 2e-5, t0 + 30 * t60 / 6)          # 300 dB: nothing missing
    bn = tb.binned(Ed, b, 2e-5, t0)
    for m in tb.METRICS:
        assert cf[m] == pytest.approx(want[m], rel=1e-12, abs=1e-12), m
        assert bn[m] == pytest.approx(want[m], rel=1e-6, abs=1e-8), m


def test_gap_and_double_slope_closed_form_against_fine_bins():
    k1, k2 = 6 * LN10 / 0.5, 6 * LN10 / 2.5
    A = [1.0, 0.3]
    gap = 0.0123
    Ed = 0.02
    t0 = 0.0311
    b = exp_bins(t0, k1, 1e-5, 20.0, gap, A[0]) + exp_bins(t0, k2, 1e-5, 20.0, gap, A[1])
    cf = tb.closed_form(Ed, gap, A, [k1, k2])
    bn = tb.binned(Ed, b, 1e-5, t0)
    for m in tb.METRICS:
        assert bn[m] == pytest.approx(cf[m], rel=1e-6, abs=1e-8), m
    # a gap longer than a window: everything reverberant is late
    cf2 = tb.closed_form(1.0, 0.06, [1.0], [10.0])
    assert cf2['c50'] == pytest.approx(10 * math.log10(1.0 / 0.1))
    assert cf2['d50'] == pytest.approx(1.0 / 1.1)


def test_window_edge_splits_a_bin_in_proportion():
    """One bin [40, 60) ms holding 1, t0 = 0: half early at 50 ms, so C50 = 0 dB; Ts = 50 ms; D50 0.5.
    With a direct sound of 1: D50 = 1.5 / 2, Ts = 25 ms, C80 = +inf (nothing after 80 ms)."""
    b = np.zeros(5)
    b[2] = 1.0
    r = tb.binned(0.0, b, 0.02, 0.0)
    assert r['c50'] == pytest.approx(0.0, abs=1e-12)
    assert r['d50'] == pytest.approx(0.5)
    assert r['ts'] == pytest.approx(0.05)
    r = tb.binned(1.0, b, 0.02, 0.0)
    assert r['d50'] == pytest.approx(0.75)
    assert r['ts'] == pytest.approx(0.025)
    assert math.isnan(r['c80']) or r['c80'] == float('inf')


def test_energy_before_t_from_is_at_u0():
    """Set B's rule (energy before t_from = t0 + R/c is the direct sound, at u = 0), worked case by case below."""
    dt = 1e-3
    b = np.zeros(30)
    b[10] = 1.0                                   # [10, 11) ms
    r = tb.binned(0.0, b, dt, 0.0105, t_from=0.0115)   # wholly before t_from: all at u = 0
    assert r['ts'] == 0.0
    shifted = np.zeros(60)
    shifted[21] = 1.0                             # [10.5, 11) ms at dt 0.5 ms
    shifted[22] = 0.0
    r = tb.binned(0.0, shifted, 0.5e-3, 0.010, t_from=0.011)
    assert r['ts'] == pytest.approx(0.0, abs=1e-15)    # [10.5, 11) ends at t_from: all direct
    one = np.zeros(30)
    one[10] = 1.0                                 # [10, 11) ms at dt 1 ms, t_from 10.5 ms
    r = tb.binned(0.0, one, dt, 0.010, t_from=0.0105)
    assert r['ts'] == pytest.approx(0.5 * 0.00075)      # half after, centred at 10.75 ms
    # reflected energy before t0 is lumped at u = 0 (t_from defaults to t0)
    r = tb.binned(0.0, one, dt, 0.0105)
    assert r['ts'] == pytest.approx(0.5 * 0.00025)
    with pytest.raises(ValueError):
        tb.binned(0.0, one, dt, 0.0105, t_from=0.01)


def test_spl_scale_and_reference():
    b = np.zeros(10)
    b[3] = 4e-10
    assert tb.binned(0.0, b, 1e-3, 0.0)['spl'] == pytest.approx(0.0, abs=1e-12)
    r1 = tb.binned(1.0, exp_bins(0.002, 5.0, 1e-4, 10.0), 1e-4, 0.002)
    r2 = tb.binned(10.0, 10 * exp_bins(0.002, 5.0, 1e-4, 10.0), 1e-4, 0.002)
    assert r2['spl'] - r1['spl'] == pytest.approx(10.0, abs=1e-10)
    for m in ('c50', 'c80', 'd50', 'ts'):
        assert r2[m] == pytest.approx(r1[m], rel=1e-12), m


def test_says_no_a_time_zero_one_step_late():
    """Say-no: measuring from 1 ms after the true t0 (a bin's end instead of the arrival) moves C, D or Ts past 1/10
    of a limen on a T60 0.3 s decay with a direct sound: the truth is sensitive to its own time zero."""
    k = 6 * LN10 / 0.3
    b = exp_bins(0.01, k, 1e-5, 4.0)
    good = tb.binned(0.5 / k, b, 1e-5, 0.01)
    bad = tb.binned(0.5 / k, b, 1e-5, 0.011)
    moved = [m for m in ('c50', 'c80', 'd50', 'ts') if abs(bad[m] - good[m]) > tb.LIMEN[m] / 10]
    assert moved, (good, bad)


def test_limen():
    assert tb.limen('t20', 2.0) == pytest.approx(0.1)
    assert tb.limen('c80', 7.0) == 1.0
    assert tb.limen('ts', 0.1) == 0.01
    assert tb.limen('d50', 0.5) == 0.05
