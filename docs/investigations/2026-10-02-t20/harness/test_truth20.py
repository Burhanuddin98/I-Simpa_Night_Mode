"""Tests of the harness's own pieces: python -B -m pytest -p no:cacheprovider test_truth20.py"""
import sys

sys.dont_write_bytecode = True

import math  # noqa: E402

import numpy as np  # noqa: E402

import t20p2  # noqa: E402  (puts harness2 on sys.path)
import truth20  # noqa: E402

DT = t20p2.DT_FINE


def literal(direct, refl, t, dt, top_db=-5.0, bottom_db=-25.0):
    """harness2/m8b/_truth_ideal.py's truth_ideal, typed out with the range changed and nothing else."""
    M = truth20.mirror()
    nb = len(refl)
    S = np.concatenate([np.cumsum(refl[::-1])[::-1], [0.0]])
    top = direct.sum() + S[0]
    ia = int(math.floor(t / dt))
    frac = t / dt - ia
    s_t = S[ia + 1] + (1 - frac) * refl[ia]
    pieces = []
    u1 = (ia + 1) * dt - t
    shape = lambda s1: 'log' if s1 > 0 else 'lin'
    if u1 > 0:
        pieces.append((0.0, u1, s_t, S[ia + 1], shape(S[ia + 1])))
    last = nb
    while last > 0 and refl[last - 1] <= 0:
        last -= 1
    for k in range(ia + 1, last):
        pieces.append((k * dt - t, (k + 1) * dt - t, S[k], S[k + 1], shape(S[k + 1])))
    res, why = M.line(top, pieces, dt, top_db=top_db, bottom_db=bottom_db)
    return -60.0 / res[0] if res else float('nan')


def test_say_no_e_pure_exponential_to_1e_6():
    r = t20p2.say_no_e()
    assert r['pass_'], r


def test_vectorised_wrapper_equals_the_literal_copy():
    rng = np.random.default_rng(7)
    for _ in range(5):
        n = 20000
        t = rng.uniform(0.001, 0.01)
        k = 6 * math.log(10) / rng.uniform(0.05, 0.3)
        tt = np.arange(n) * DT
        refl = np.where(tt > t, np.exp(-k * (tt - t)) * rng.uniform(0.2, 1.8, n), 0.0) * DT
        refl[rng.integers(0, n, 50)] = 0.0
        direct = np.zeros(n)
        direct[int(t / DT)] = rng.uniform(0, 0.05)
        for top, bot in ((-5.0, -25.0), (0.0, -20.0)):
            a = truth20.t20(direct, refl, t, DT, top, bot)[0]
            b = literal(direct, refl, t, DT, top, bot)
            assert a == b or abs(a / b - 1) < 1e-12, (a, b)


def test_reflected_anchor_moves_the_truth_when_the_direct_sound_is_strong():
    n, t, T = 60000, 0.002, 0.5
    k = 6 * math.log(10) / T
    tt = np.arange(n) * DT
    refl = np.where(tt > t, np.exp(-k * (tt - t)), 0.0) * DT
    refl = refl * (1 + 0.5 * np.exp(-tt / 0.05))           # a curved early decay
    direct = np.zeros(n)
    direct[int(t / DT)] = refl.sum()
    a = truth20.t20(direct, refl, t, DT)[0]
    b = truth20.t20(direct, refl, t, DT, anchor='reflected')[0]
    assert abs(b / a - 1) > 0.005


def _row(R, step, t20, truth, t60=1.0, status='ok'):
    return dict(id='x', R=R, step_ms=step, t20=t20, truth=truth, t60=t60, status=status, reason='' if t20 else 'r')


def test_score_flags_a_wrong_silent_cell_and_a_planted_scale():
    rows = [_row(R, s, 1.0, 1.0) for R in t20p2.RADII for s in t20p2.STEPS_MS for _ in range(300)]
    sc = t20p2.score(rows, False)
    assert sc['J1a']['pass_'] and sc['J1b']['pass_'] and sc['J4']['pass_']
    rows[0]["t20"] = rows[1]["t20"] = 1.06     # two wrong-silent rows in R0.1|1ms: 2/300 > 0.5 %
    sc = t20p2.score(rows, False)
    assert not sc['J1a']['pass_']
    planted = [dict(r, t20_planted=r['t20'] * 1.02) for r in rows]
    assert t20p2.j1b_share(planted, value='t20_planted')[0] < t20p2.J1B_MIN


def test_score_j4_counts_refusals_and_truncation_makes_inconclusive():
    rows = [_row(0.31, 1, None if i < 20 else 1.0, 1.0) for i in range(100)]
    sc = t20p2.score(rows, False)
    assert not sc['J4']['pass_'] and sc['J4']['share'] == 0.8
    rows += [_row(0.31, 1, 1.0, 1.0, status='truth_truncated') for _ in range(20)]
    assert t20p2.score(rows, False)['inconclusive_truncation']
