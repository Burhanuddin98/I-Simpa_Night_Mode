"""T44-T46 (audit fixes 1 and 3; ../RED-2.md): the P6 truth-feature report and the R_m rule.

Written before m8b/features.py exists. The planted truths are analytic energy decays: a reference of reverberation time
T has energy e^(-k t), k = 6 ln 10 / T, so its ISO 3382-1 T30 is T by construction (the test's own reference, not the
module's). Each feature is shown met on one planted room and missed on another.
"""
import math

import numpy as np
import pytest

from m8b import features, rooms2, score, score_heldout

DT = 1e-3
N = 8000
T_ARR = 0.02
H = 0.31 / 343.2


def decay(T, n=N):
    t = (np.arange(n) + 0.5) * DT
    s = np.exp(-6 * math.log(10) / T * t) * DT
    s[: int(T_ARR / DT)] = 0.0
    s[int(T_ARR / DT)] = 10 * s[int(T_ARR / DT) + 1]            # a direct sound in the arrival bin
    return s


def double_slope(n=N):
    t = (np.arange(n) + 0.5) * DT
    k1, k2 = 6 * math.log(10) / 0.4, 6 * math.log(10) / 2.4
    s = (0.9 * np.exp(-k1 * t) + 0.1 * np.exp(-k2 * t)) * DT
    s[: int(T_ARR / DT)] = 0.0
    s[int(T_ARR / DT)] = 10 * s[int(T_ARR / DT) + 1]
    return s


def rec(label, series, band=1000.0, blocked=False, k=4):
    return dict(label=label, band_hz=band, refs=[series * (1 + 0.001 * i) for i in range(k)], dt=DT, arrival_s=T_ARR,
                half_width=H, blocked=blocked)


def by(rows, room):
    return {r['feature']: r for r in rows if r['room'] == room}


def test_t44_t30_is_iso_3382_1_on_the_schroeder_curve():
    for T in (0.2, 1.0, 2.7):
        assert features.t30(decay(T), DT) == pytest.approx(T, rel=0.03)
    short = decay(2.0)[:800]                                    # 0.8 s: the curve never reaches -35 dB
    assert math.isnan(features.t30(short, DT))
    assert math.isnan(features.t30(np.zeros(100), DT))


def test_t44_g2_feature_met_and_missed_and_reads_1_khz_only():
    met = features.truth_features({'G2': [rec('R000', decay(3.0)), rec('R000', decay(1.0), band=500.0)]})
    f = by(met, 'G2')['T30_1kHz']
    assert f['met'] is True and f['value'] == pytest.approx(3.0, rel=0.03) and f['target'] == '>= 2.5 s'
    miss = features.truth_features({'G2': [rec('R000', decay(1.5)), rec('R000', decay(3.0), band=500.0)]})
    f = by(miss, 'G2')['T30_1kHz']
    assert f['met'] is False and f['value'] == pytest.approx(1.5, rel=0.03)


def test_t44_g7_feature_met_and_missed():
    met = by(features.truth_features({'G7': [rec('R000', decay(0.2))]}), 'G7')['T30_1kHz']
    assert met['met'] is True and met['target'] == '<= 0.25 s' and met['value'] == pytest.approx(0.2, rel=0.05)
    miss = by(features.truth_features({'G7': [rec('R000', decay(0.5))]}), 'G7')['T30_1kHz']
    assert miss['met'] is False


def test_t44_g3_ratio_at_half_the_receivers():
    def room(n_double, n=8):
        return [rec('R%03d' % i, double_slope() if i < n_double else decay(1.8)) for i in range(n)]
    for n_double, want in ((8, True), (4, True), (3, False), (0, False)):
        f = by(features.truth_features({'G3': room(n_double)}), 'G3')['T30_over_EDT_1kHz']
        assert f['met'] is want, n_double
        assert f['n_met'] == n_double and f['n_receivers'] == 8 and f['target'].startswith('>= 1.25')
        assert len(f['per_receiver']) == 8
    d = by(features.truth_features({'G3': room(8)}), 'G3')['T30_over_EDT_1kHz']['per_receiver'][0]
    assert d['ratio'] >= 1.25 and d['t30'] > d['edt'] > 0
    single = by(features.truth_features({'G3': room(0)}), 'G3')['T30_over_EDT_1kHz']['per_receiver'][0]
    assert single['ratio'] == pytest.approx(1.0, abs=0.1)


def test_t44_unreadable_truth_is_missed_not_crashed():
    flat = rec('R000', np.zeros(N))
    rows = features.truth_features({'G2': [flat], 'G7': [flat], 'G3': [flat]})
    assert len(rows) == 3 and all(r['met'] is False for r in rows)


def test_t44_report_text_and_results_md():
    rows = features.truth_features({'G2': [rec('R000', decay(3.0))], 'G7': [rec('R000', decay(0.5))],
                                    'G3': [rec('R%03d' % i, decay(1.8)) for i in range(8)]})
    text = features.format_features(rows)
    for want in ('G2', 'G3', 'G7', '2.5 s', '0.25 s', '1.25', 'met', 'missed'):
        assert want in text
    assert text.count('missed') >= 2 and ' met' in text
    results = score_heldout.results_md(_summary(), dict(features=text, ism_seed=1, synth_seed=2, ism_rejections={}, synth_rejections={}, rows_built={}), 'run2', 'abc1234', False)
    assert text in results and 'not checked in this run' not in results


def _summary():
    import test_r2_scorer as T
    return score.evaluate(T.planted_inputs(), out_dir=_summary.dir)


def test_t44_from_runs_reads_the_summed_truth_runs_of_g2_g3_g7_only():
    asked = []

    def fake_read(run_dir):
        name = run_dir.name
        asked.append(name)
        room = name.split('-')[1]
        n = len(rooms2.rooms()[room]['receivers'])
        T = {'G2': 3.0, 'G3': 1.8, 'G7': 0.2}.get(room, 1.0)
        return [dict(label='R%03d' % i, band_hz=1000.0, energy_pa2=decay(T), arrival_s=T_ARR, half_width=H, dt=DT)
                for i in range(n)]
    rows = features.from_runs('X:/none', read=fake_read)
    assert sorted(set(a.split('-')[1] for a in asked)) == ['G2', 'G3', 'G7'] and len(asked) == 12
    assert {r['room'] for r in rows} == {'G2', 'G3', 'G7'} and len(rows) == 3
    assert by(rows, 'G2')['T30_1kHz']['met'] and by(rows, 'G7')['T30_1kHz']['met']
    assert not by(rows, 'G3')['T30_over_EDT_1kHz']['met']


# ---- T46: R_m is required on every set ------------------------------------------------------------------------
def test_t46_missing_r_m_is_an_error_on_every_set():
    import test_r2_scorer as S
    base = S.synth_input(0)
    del base['R_m']
    with pytest.raises(ValueError, match='R_m'):
        score._check_inputs([base], None)
    with pytest.raises(ValueError, match='R_m'):
        score.evaluate([base], out_dir=_summary.dir2)
    att = dict(S.synth_input(1), set='attack', class_id='c')
    del att['R_m']
    with pytest.raises(ValueError, match='R_m'):
        score._check_inputs([att], None)


@pytest.fixture(autouse=True)
def _dirs(tmp_path):
    _summary.dir, _summary.dir2 = tmp_path / 'a', tmp_path / 'b'
