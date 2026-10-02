"""Tests of score3b.py on synthetic rows and series: python -B -m pytest -p no:cacheprovider test_score3b.py"""
import sys

sys.dont_write_bytecode = True

import math  # noqa: E402

import numpy as np  # noqa: E402

import score3b  # noqa: E402

DT = 1e-4                     # the truth runs' step
H = 0.31 / 343.2              # half-width of the 0.31 m ball


def row(value, mc_sd, truth=1.0, truth_se=0.001, source='value', mode='random', room='G1', rec='R000', step=1.0,
        band=1000, t60=1.0):
    return dict(run_id='x', room=room, mode=mode, step_ms=step, particles=150000, seed=1, receiver=rec,
                freq_hz=band, blocked=False, design_t60_s=t60, R_m=0.31, value=value, mc_sd=mc_sd, source=source,
                truth=truth, truth_se=truth_se, truth_why=None, truth_singles='', truth_last10=0.0)


def exp_series(T, t_arr, n, direct_frac=0.2, scale=1.0):
    """A direct impulse at t_arr and an exactly integrated exponential from the end of the ball's window."""
    k = 6 * math.log(10) / T
    e = np.arange(n + 1) * DT
    t0 = (math.floor((t_arr + H) / DT) + 1) * DT          # the first bin wholly after the direct window
    lo = np.maximum(e[:-1], t0)
    refl = np.where(e[1:] > t0, (np.exp(-k * (lo - t0)) - np.exp(-k * (e[1:] - t0))) / k, 0.0)
    bins = refl.copy()
    bins[int(t_arr / DT)] += direct_frac / k
    return bins * scale


# ---- 1. a planted 6 % bias with a tight mc_sd is wrong-silent ---------------------------------------------------------
def test_planted_six_percent_bias_with_tight_mc_sd_is_wrong_silent():
    r = score3b.classify(row(1.06, 0.001))
    assert r['status'] == 'answered' and not r['covered'] and r['wrong_silent']
    # the same bias with an honest range is covered, and is a wide row, not wrong-silent
    r = score3b.classify(row(1.06, 0.03))
    assert r['covered'] and not r['wrong_silent'] and r['wide']
    # 4 % off and not covered is not wrong-silent (under the 5 % JND)
    r = score3b.classify(row(1.04, 0.001))
    assert not r['covered'] and not r['wrong_silent']


def test_planted_bias_fails_j2_and_j3():
    rows = [row(1.0, 0.002, rec='R%03d' % (i % 2), band=125 * (1 + i // 2 % 6)) for i in range(200)]
    s, _ = score3b.score([dict(r) for r in rows])
    assert s['j2']['random']['holds'] and s['j3']['random']['holds'] and s['j4']['random']['holds']
    planted = [dict(r, value=1.06) if r['receiver'] == 'R000' and i % 4 == 0 else dict(r) for i, r in enumerate(rows)]
    s, _ = score3b.score(planted)
    assert s['j2']['random']['wrong_silent'] == 50 and not s['j2']['random']['holds']
    assert s['j3']['random']['breaches'] == ['G1 1ms']          # room x step, receivers pooled
    assert s['j3']['random']['worst_room_receiver_step']['receiver'] == 'R000'
    assert s['j3']['random']['worst_room_receiver_step']['wrong_silent_rate'] == 0.5


# ---- 2. a refusal counts as unanswered, by code -----------------------------------------------------------------------
def test_refusal_is_unanswered_and_counted_by_code():
    noise = {'not_evaluable': {'code': 'params_not_evaluable', 'message': 'm',
                               'error': {'kind': 'not_evaluable', 'quantity': 't20',      # the schema's shape
                                         'why': {'why': 'monte_carlo_noise', 'value': 1.0, 'sd': 0.01}}}}
    other = {'not_evaluable': {'code': 'params_series_too_short', 'message': 'm',
                               'error': {'kind': 'series_too_short'}}}
    assert score3b.read_param({'value': 0.5, 'mc_sd': 0.01}) == (0.5, 0.01, 'value')
    assert score3b.read_param(noise) == (None, None, 'monte_carlo_noise')     # its own value is NOT used
    assert score3b.read_param(other) == (None, None, 'params_series_too_short')
    assert score3b.read_param(None) == (None, None, 'absent')
    rows = [row(1.0, 0.002) for _ in range(9)] + [row(None, None, source='monte_carlo_noise')]
    s, out = score3b.score(rows)
    assert out[-1]['status'] == 'unanswered'
    assert s['unanswered_by_code'] == {'monte_carlo_noise': 1}
    assert s['j2']['random']['answered'] == 9 and s['j2']['random']['coverage'] == 1.0
    assert s['j4']['random']['share'] == 0.9 and s['j4']['random']['holds']
    assert s['j4']['random']['refusals_by_code'] == {'monte_carlo_noise': 1}
    rows.append(row(None, None, source='monte_carlo_noise'))
    assert not score3b.score(rows)[0]['j4']['random']['holds']                 # 9 of 11 < 90 %


def test_product_reader_maps_every_receiver_band():
    prod = dict(exit=0, receiver_radius_m=0.31, t20={('R000', 125): (0.5, 0.01, 'value'),
                                                    ('R000', 250): (None, None, 'truncated')})
    truths = {k: dict(truth=0.5, truth_se=0.0, truth_why=None, singles=[0.5] * 4, last10_share=0.0, blocked=False)
              for k in (('R000', 125), ('R000', 250))}
    run = dict(run_id='r', room='G1', mode='random', step_ms=1.0, particles=150000, seed=1)
    rs = score3b.make_rows(run, prod, truths, dict(design_t60_s={125: 1.0, 250: 1.0}))
    assert [r['source'] for r in rs] == ['value', 'truncated']
    rs = score3b.make_rows(run, dict(prod, exit=3), truths, dict(design_t60_s={125: 1.0, 250: 1.0}))
    assert [r['source'] for r in rs] == ['results_failed'] * 2 and all(r['value'] is None for r in rs)


# ---- 3. the truth's standard error excludes at 0.5 % ------------------------------------------------------------------
def test_truth_se_exclusion_at_one_percent_gated_and_half_a_percent_prereg():
    G, P = score3b.GATED, score3b.PREREG_RULE
    assert score3b.classify(row(1.0, 0.002, truth_se=0.0099), rule=G)['status'] == 'answered'
    assert score3b.classify(row(1.0, 0.002, truth_se=0.010), rule=G)['status'] == 'answered'     # 1.0 % is kept
    assert score3b.classify(row(1.0, 0.002, truth_se=0.0101), rule=G)['status'] == 'excluded_truth_uncertain'
    assert score3b.classify(row(1.0, 0.002, truth_se=0.005), rule=P)['status'] == 'answered'     # 0.5 % is kept
    assert score3b.classify(row(1.0, 0.002, truth_se=0.0051), rule=P)['status'] == 'excluded_truth_uncertain'
    assert score3b.classify(row(1.0, 0.002, truth=float('nan')))['status'] == 'excluded_truth_nan'
    rows = [row(1.0, 0.002, truth_se=0.006), row(None, None, source='truncated', truth_se=0.006)] + [row(1.0, 0.002)] * 8
    s, out = score3b.score(rows)
    assert s['excluded']['count'] == 0 and s['j2']['random']['answered'] == 9
    assert s['prereg_rule']['excluded']['count'] == 2 and s['prereg_rule']['j2']['random']['answered'] == 8
    assert out[0]['status'] == 'answered' and out[0]['status_prereg'] == 'excluded_truth_uncertain'
    assert s['prereg_rule']['j4']['random']['rows'] == 10   # J4 keeps truth-excluded rows: answering needs no truth


def test_a_row_covered_only_thanks_to_the_truth_se():
    # |err| 3 %: mc_sd 0.005 alone gives 2.5*0.005 + 0.5 % = 1.75 %; with se 0.009, 2.5*hypot = 2.57 % + 0.52 %
    r = row(1.03, 0.005, truth_se=0.009)
    g = score3b.classify(dict(r), rule=score3b.GATED)
    assert g['covered'] and not g['wrong_silent']
    assert not score3b.covered(1.03, 0.005, 1.0)                       # mc_sd alone: not covered
    s, out = score3b.score([r] + [row(1.0, 0.002)] * 9)
    assert out[0]['covered'] and out[0]['status_prereg'] == 'excluded_truth_uncertain'
    # within the prereg's se limit too, the original rule leaves it uncovered and the gated rule covers it
    r = row(1.02, 0.005, truth_se=0.005)          # 2 %: 1.76 % prereg half-width, 2.28 % gated
    s, out = score3b.score([r])
    assert out[0]['covered'] and out[0]['covered_prereg'] is False
    # a 6 % row stays wrong-silent under the gated rule when both uncertainties are tight
    g = score3b.classify(row(1.06, 0.002, truth_se=0.004), rule=score3b.GATED)
    assert g['wrong_silent']


def test_more_than_a_quarter_excluded_is_inconclusive():
    good = [row(1.0, 0.002, room='G%d' % (1 + i % 7)) for i in range(75)]
    noisy = [row(1.0, 0.002, truth_se=0.02) for _ in range(25)]
    s, _ = score3b.score(good + noisy)
    assert s['excluded_share'] == 0.25 and s['verdict'] == 'PASS'                 # exactly 25 %: not more
    s, _ = score3b.score(good + noisy + [row(1.0, 0.002, truth_se=0.02)])
    assert s['excluded_share'] > 0.25 and s['inconclusive'] and s['verdict'] == 'INCONCLUSIVE'
    assert s['criteria_hold']                                                   # the J's held; the set did not
    s, _ = score3b.score(good + noisy, smoke=True)
    assert s['verdict'] == 'SMOKE'


def test_j3_pools_receivers_within_room_and_step():
    # 8 receivers x 3 rows: 24 in the room x step, 3 per receiver; 3 wrong-silent at one receiver = 12.5 % > 10 %
    rows = [row(1.0, 0.002, rec='R%03d' % i) for i in range(8) for _ in range(3)]
    for k in range(3):
        rows[k]['value'] = 1.06
    s, _ = score3b.score(rows)
    assert s['j3']['random']['subgroups_applying'] == 1 and s['j3']['random']['breaches'] == ['G1 1ms']
    w = s['j3']['random']['worst_room_receiver_step']
    assert (w['receiver'], w['answered'], w['wrong_silent']) == ('R000', 3, 3)
    s, _ = score3b.score(rows[:19])                     # 19 answered rows: J3 does not apply
    assert s['j3']['random']['subgroups_applying'] == 0 and s['j3']['random']['holds']


def test_truth_se_is_stdev_of_singles_over_two():
    Ts = (1.00, 1.01, 0.99, 1.02)
    series = [exp_series(T, 0.01, int(round(2.0 * T / DT))) for T in Ts]
    e = score3b.truth_entry(series, DT, 0.01, H, False)
    assert all(abs(s / T - 1) < 1e-6 for s, T in zip(e['singles'], Ts))
    assert abs(e['truth_se'] - np.std(e['singles'], ddof=1) / 2) < 1e-15
    assert e['truth_se'] / e['truth'] > score3b.TOL          # 0.65 %: this room's row would be excluded
    near = [exp_series(T, 0.01, 20000) for T in (1.0, 1.001, 0.999, 1.0)]
    e = score3b.truth_entry(near, DT, 0.01, H, False)
    assert e['truth_se'] / e['truth'] < score3b.TOL


# ---- 4. the truth on a pure exponential is exact to 1e-6 --------------------------------------------------------------
def test_truth_on_a_pure_exponential_is_exact():
    for T in (0.18, 0.7, 2.9):
        n = int(round(2.0 * T / DT))                  # -120 dB: nothing missing that the fit can see
        for direct_frac in (0.0, 0.2, 1.0):
            bins = exp_series(T, 0.0123, n, direct_frac)
            got, why = score3b.truth_of(bins, DT, 0.0123, H, False)
            assert why is None and abs(got / T - 1) < 1e-6, (T, direct_frac, got)
        # K = 4 identical histograms at different scales: the mean's T20 is the same exponential's
        e = score3b.truth_entry([exp_series(T, 0.0123, n, 0.2, s) for s in (1, 2, 3, 4)], DT, 0.0123, H, False)
        assert abs(e['truth'] / T - 1) < 1e-6 and e['truth_se'] / T < 1e-9


def test_blocked_receiver_reads_from_its_first_bin_with_energy():
    T = 0.6
    bins = exp_series(T, 0.02, int(round(2.0 * T / DT)), direct_frac=0.0)
    got, why = score3b.truth_of(bins, DT, 0.0, H, True)
    assert why is None and abs(got / T - 1) < 1e-6


def test_j4_filter():
    assert score3b.j4_in(row(1, 0.1, step=1.0000000474974513, t60=3.2))
    assert not score3b.j4_in(row(1, 0.1, step=2.0))
    assert not score3b.j4_in(row(1, 0.1, t60=3.21))
    assert not score3b.j4_in(dict(row(1, 0.1), R_m=0.6))
