"""T23-T31 and T38 (../../HARNESS-PLAN-2.md section 6, with section 9's resolutions): round 2's method pin,
the loader, the Z = 2.5 scorer, H4's new filter and denominator, the refusal table, the H1/H2/H3/H5 control
that no radius filter exists, the pairing with upstream, the R_m field and the planted dead room.

Written before the code they hold (RED-2.md records each one failing). A module under test is never its own
reference: round 2's hash is typed here, not read from m8b.corpus; the dead room is the corpus's own ISM
generator through make_row, whose truth is Definition A.
"""
import json
import math
import re
import subprocess

import pytest
from conftest import FROZEN, FROZEN2, FROZEN2_SHA256, FROZEN_SHA256, INVESTIGATION, sha256_bytes

from m8b import corpus, ism_fresh, method, score, spps_rows, synth_fresh
from m8b.corpus import VoidRun

import test_wiring as W                      # the SPPS-row fixtures of step 7b (a fake room, fake runs)
import test_score as TS                      # hand-made rows


def row(**k):
    """test_score.row plus R_m."""
    k.setdefault('R_m', 0.31)
    return TS.row(**k)


# ---- T23: the file, its hash, its Z and its line ends -------------------------------------------------------
def test_t23_frozen2_hash_z_and_line_ends():
    data = FROZEN2.read_bytes()
    assert sha256_bytes(data) == FROZEN2_SHA256, 'frozen2/method.py is not the file PREREG-2 pins'
    assert getattr(corpus, 'FROZEN2_SHA256', None) == FROZEN2_SHA256
    rec = (INVESTIGATION / 'frozen2.sha256').read_text(encoding='utf-8').split()    # m1: a constant derived from the file
    assert rec[0] == FROZEN2_SHA256 and rec[1].endswith('frozen2/method.py')
    m = method.load()
    assert m.Z == 2.5 and m.checked_sha256 == FROZEN2_SHA256 and m.checked_path == FROZEN2.resolve()
    ga = (INVESTIGATION / '.gitattributes').read_text(encoding='utf-8').splitlines()
    assert 'frozen2/** text eol=lf' in ga and 'harness2/** text eol=lf' in ga
    out = subprocess.run(['git', '-C', str(INVESTIGATION), 'ls-files', '--eol', 'frozen2/method.py'],
                         capture_output=True, text=True, timeout=60).stdout
    assert 'i/lf' in out and 'w/lf' in out and 'attr/text eol=lf' in out, out


# ---- T24: the pins -------------------------------------------------------------------------------------------
def test_t24_the_loader_is_pinned_per_round(tmp_path):
    data = FROZEN2.read_bytes()
    i = data.index(b'Z = 2.5') + len(b'Z = 2.')
    bad = bytearray(data)
    bad[i] = ord('6')                                         # 2.5 -> 2.6: one byte, still valid Python
    assert sum(a != b for a, b in zip(data, bad)) == 1
    p = tmp_path / 'method.py'
    p.write_bytes(bytes(bad))
    with pytest.raises(VoidRun) as e:
        method.load(p)
    assert e.value.checked_sha256 == sha256_bytes(bytes(bad))
    with pytest.raises(VoidRun):
        method.load(FROZEN2, pin=FROZEN_SHA256)                # round 1's pin rejects round 2's file
    with pytest.raises(VoidRun):
        method.load(FROZEN, pin=FROZEN2_SHA256)                # and round 2's pin rejects round 1's
    r1 = method.load(FROZEN, pin=FROZEN_SHA256)                # control: round 1's file, as round 1 loaded it
    assert r1.Z == 2.0 and r1.checked_sha256 == FROZEN_SHA256
    assert method.load().Z == 2.5


# ---- planted rows for evaluate -------------------------------------------------------------------------------
def synth_input(i, R=0.31, step_ms=1.0, family=1.5):
    k = 6 * math.log(10) / 0.8
    h = R / synth_fresh.C
    Ed = (1 / k) * 10 ** (-0.3)
    S = synth_fresh.generator()
    dt = step_ms * 1e-3
    bins = S.histogram(dt, 2.0, 0.02, h, Ed, 0.002, [1.0], [k])
    return dict(set='synth', id='t25-%d' % i, bins=bins, dt=dt, t_arrival=0.02, meta={'half_width': h},
                truth=S.truth_edt(0.02, Ed, 0.002, [1.0], [k]), truth_status='ok', family=family, step_ms=step_ms, R_m=R)


def dead_room_row():
    """T38's plant: a 5 x 4 x 2.6 m box at alpha 0.8, a 0.31 m ball, 0.1 ms steps (corpus ISM generator)."""
    room = dict(dims_m=[5.0, 4.0, 2.6], alpha_walls=[0.8] * 6, source_m=[1.0, 1.0, 1.2], image_time_s=0.8)
    rec = [3.5, 3.0, 1.3]
    r = ism_fresh.make_row(room, rec, 0.31, 1000.0, 0.1, run_s=0.6)
    return room, rec, r


def planted_inputs():
    """One synth row, plus a dead-room ISM row and a dead-room SPPS row of each mode (receiver_too_large under
    round 2's method), so every set and the refusal table are fed."""
    room, rec, r = dead_room_row()
    ism = dict(set='ism', id='ismf|t25|r00|1000|0.1ms', bins=r['bins'], dt=r['dt'], t_arrival=r['t_arrival'],
               meta={'half_width': r['half_width']}, truth=r['truth_edt'], truth_status=r['truth_status'], room='drawn:t25',
               d_m=math.dist(room['source_m'], rec), step_ms=0.1, band_hz=1000, design_t60_s=0.3, seed=1, R_m=0.31)
    spps = [dict(ism, set='spps', id='spps|t25|R000|1000|0.1ms|%s' % mode, mode=mode, room='G7', particles=150000, R_m=0.31)
            for mode in ('random', 'energetic')]
    return [synth_input(0), synth_input(1, R=1.2, step_ms=2.0, family=5.0), ism] + spps


def test_t25_evaluate_has_two_instances_and_no_z3(tmp_path, monkeypatch):
    assert not hasattr(method, 'load_z3'), 'round 2 loads no Z = 3 instance'
    monkeypatch.setattr(method, 'load_z3', lambda *a, **k: (_ for _ in ()).throw(AssertionError('load_z3 was called')),
                        raising=False)
    s = score.evaluate(planted_inputs(), out_dir=tmp_path / 'ev')
    assert score.INSTANCES == ('frozen', 'upstream')
    assert 'z3' not in s and set(s['criteria']) == {'frozen'}
    for lab, t in s['tables'].items():
        assert 'z3' not in t and {'frozen', 'upstream'} <= set(t), lab
    assert s['method']['Z'] == 2.5 and s['method']['sha256'] == FROZEN2_SHA256
    rec = json.loads((tmp_path / 'ev' / 'rows.jsonl').read_text(encoding='utf-8').splitlines()[0])
    assert 'z3' not in rec and 'upstream' in rec and 'frozen' in rec
    report = (tmp_path / 'ev' / 'REPORT.md').read_text(encoding='utf-8')
    assert 'Z = 3' not in report and 'Z = 2.5' in report and 'Z = 2)' not in report
    from m8b import score_heldout
    extra = dict(ism_seed=1, synth_seed=2, ism_rejections={}, synth_rejections={}, rows_built={})
    results = score_heldout.results_md(s, extra, 'run2', 'abc1234', False)
    assert 'Z = 3' not in results and 'Z = 2.5' in results and 'Z = 2)' not in results


# ---- T26, T27: H4 ---------------------------------------------------------------------------------------------
def test_t26_h4_filter_step_t60_and_radius():
    assert score._in_h4(row(R_m=0.5)) is True
    assert score._in_h4(row(R_m=0.5000001)) is False
    assert score._in_h4(row(R_m=0.31, step_ms=2.0)) is False
    assert score._in_h4(row(design_t60_s=3.0)) is True
    assert score._in_h4(row(design_t60_s=3.01)) is False
    assert score._in_h4(row(step_ms=1.0000000474974513)) is True          # float32's 1 ms
    r = row()
    del r['R_m']
    with pytest.raises(ValueError):
        score._in_h4(r)
    with pytest.raises(ValueError):
        score._check_inputs([dict(synth_input(0), set='ism', room='x', d_m=1.0, design_t60_s=1.0, R_m=None)], None)


def h4_rows(set_, n_ok, n_wide=0, n_refused=0, n_rtl=0, **k):
    out = [row(set=set_, id='%s-ok%d' % (set_, i), **k) for i in range(n_ok)]
    out += [row(set=set_, id='%s-wide%d' % (set_, i), status='wide', edt=1.0, lo=0.8, hi=1.25, **k) for i in range(n_wide)]
    out += [row(set=set_, id='%s-ref%d' % (set_, i), status='refused', edt=None, reason='run_too_short', **k)
            for i in range(n_refused)]
    out += [row(set=set_, id='%s-rtl%d' % (set_, i), status='refused', edt=None, reason='receiver_too_large', **k)
            for i in range(n_rtl)]
    return out


def test_t27_h4_denominator_drops_receiver_too_large_and_counts_it():
    ism = h4_rows('ism', 16, n_wide=2, n_refused=2, n_rtl=5)
    h = score.h4({'spps': h4_rows('spps', 10), 'ism': ism})
    p = h['per_set']['ism']
    assert p['n_before'] == 25 and p['n_receiver_too_large'] == 5 and p['n'] == 20
    assert p['share_receiver_too_large'] == 5 / 25
    assert p['n_usable'] == 18 and p['usable_share'] == 0.9 and p['pass'] is True      # 18 of 20 = exactly 90 %
    assert p['refused'] == {'run_too_short': 2}, 'other refusals stay in the denominator'
    assert p['ok_share'] == 16 / 20
    h = score.h4({'spps': h4_rows('spps', 10), 'ism': h4_rows('ism', 15, n_refused=3, n_rtl=5)})
    assert h['per_set']['ism']['usable_share'] == 15 / 18 and h['per_set']['ism']['pass'] is False
    # the ok share is reported, not gated
    h = score.h4({'spps': h4_rows('spps', 0, n_wide=10), 'ism': h4_rows('ism', 0, n_wide=10)})
    assert h['pass'] is True and h['per_set']['spps']['ok_share'] == 0.0
    # nothing left in the denominator fails (P27)
    h = score.h4({'spps': h4_rows('spps', 10), 'ism': h4_rows('ism', 0, n_rtl=6)})
    p = h['per_set']['ism']
    assert p['n'] == 0 and p['n_receiver_too_large'] == 6 and p['usable_share'] is None and p['pass'] is False
    # a radius above 0.5 m, or another step, is not in H4 at all, so a refusal there is not counted in it
    far = h4_rows('ism', 4, n_rtl=3, R_m=0.9) + h4_rows('ism', 4, n_rtl=3, step_ms=2.0)
    p = score.h4({'spps': h4_rows('spps', 10), 'ism': far + h4_rows('ism', 10)})['per_set']['ism']
    assert p['n_before'] == 10 and p['n_receiver_too_large'] == 0


# ---- T28: the refusal table ------------------------------------------------------------------------------------
def test_t28_refusals_by_reason_set_and_radius_class(tmp_path):
    rows = (h4_rows('ism', 6, n_rtl=2, R_m=0.2) + h4_rows('ism', 4, n_rtl=1, R_m=0.5) + h4_rows('ism', 3, n_rtl=4, R_m=0.75)
            + h4_rows('ism', 1, n_rtl=5, R_m=1.0) + h4_rows('ism', 1, n_rtl=6, R_m=1.4))
    t = score.rtl_by_r_class(rows)
    assert set(t) == {'<=0.5', '0.5-1.0', '>1.0'}
    n = lambda c: (t[c]['n_rows'], t[c]['n_receiver_too_large'], t[c]['share'])
    assert n('<=0.5') == (13, 3, 3 / 13)            # R 0.2 (8 rows) and R 0.5 (5 rows), 0.5 itself is in the first class
    assert n('0.5-1.0') == (13, 9, 9 / 13)          # R 0.75 (7 rows) and R 1.0 (6 rows), 1.0 itself is in the second
    assert n('>1.0') == (7, 6, 6 / 7)
    s = score.evaluate(planted_inputs(), out_dir=tmp_path / 'ev')
    assert set(s['tables']) == {'spps-random', 'spps-energetic', 'ism', 'synth', 'attack'}
    for lab in ('spps-random', 'spps-energetic', 'ism', 'synth'):
        assert set(s['tables'][lab]['rtl_by_r_class']) == {'<=0.5', '0.5-1.0', '>1.0'}, lab
    assert s['tables']['ism']['frozen']['refused'].get('receiver_too_large') == 1
    assert s['tables']['ism']['rtl_by_r_class']['<=0.5']['n_receiver_too_large'] == 1
    assert s['tables']['spps-random']['rtl_by_r_class']['<=0.5']['n_rows'] == 1          # Energetic is its own set (P11)
    assert s['tables']['spps-energetic']['rtl_by_r_class']['<=0.5']['n_rows'] == 1
    report = (tmp_path / 'ev' / 'REPORT.md').read_text(encoding='utf-8')
    assert 'receiver_too_large' in report and '0.5-1.0' in report


# ---- T29: no radius filter outside H4 ---------------------------------------------------------------------------
def wrong(set_, i, R):
    return row(set=set_, id='%s-w%d' % (set_, i), edt=1.2, lo=1.19, hi=1.21, R_m=R, room='I1', d_m=1.5, step_ms=1.0)


def good(set_, n, R=0.31, tag='g', **k):
    return [row(set=set_, id='%s-%s%d' % (set_, tag, i), edt=1.0, lo=0.99, hi=1.01, R_m=R, room='I1', d_m=1.5, step_ms=1.0, **k)
            for i in range(n)]


def test_t29_h1_h2_h3_h5_count_a_wrong_silent_row_at_any_radius():
    base = good('ism', 100) + good('synth', 100)
    assert score.h1({'ism': base[:100], 'synth': base[100:]})['pass'] is True                       # control
    big = score.h1({'ism': good('ism', 100) + [wrong('ism', 0, 1.4)], 'synth': good('synth', 100)})
    assert big['pass'] is False and big['per_set']['ism']['n_wrong_silent'] == 1
    assert score.h2(good('spps', 100))['pass'] is True                                              # control
    assert score.h2(good('spps', 96) + [wrong('spps', i, 1.4) for i in range(4)])['pass'] is False
    sub = good('ism', 16) + [wrong('ism', i, 1.4) for i in range(4)]                                # 4 of 20 = 20 %
    assert score.h3(sub)['pass'] is False
    assert score.h3(good('ism', 20))['pass'] is True
    sets = ('spps', 'ism', 'synth', 'attack')
    fz = {s: good(s, 30) for s in sets}
    up = {s: good(s, 30) for s in sets}
    for s in sets:
        ids = [r['id'] for r in fz[s]]
        fz[s][0] = dict(wrong(s, 0, 1.4), id=ids[0])
        up[s][0] = dict(wrong(s, 0, 1.4), id=ids[0])
        up[s][1] = dict(wrong(s, 1, 1.4), id=ids[1])
    h5 = score.h5(fz, up)
    assert h5['per_set']['ism']['n_wrong_silent'] == 1 and h5['per_set']['ism']['n_wrong_silent_upstream'] == 2
    assert h5['pass'] is True


# ---- T30, T31: pairing and the R_m field --------------------------------------------------------------------------
def test_t30_upstream_is_scored_on_the_same_ids_and_modes_stay_apart(tmp_path):
    inputs = planted_inputs()
    s = score.evaluate(inputs, out_dir=tmp_path / 'ev')
    recs = [json.loads(x) for x in (tmp_path / 'ev' / 'rows.jsonl').read_text(encoding='utf-8').splitlines()]
    assert [(r['set'], r['id']) for r in recs] == [(x['set'], x['id']) for x in inputs]
    for r, x in zip(recs, inputs):
        assert r['R_m'] == x['R_m'], 'the upstream and frozen rows share the input\'s R_m'
        assert r['upstream']['status'] in ('ok', 'wide', 'refused') and r['frozen']['status'] in ('ok', 'wide', 'refused')
    h5 = s['criteria']['frozen']['random']['H5']['per_set']
    for lab in ('ism', 'synth'):
        assert h5[lab]['n_paired'] == sum(1 for x in inputs if x['set'] == lab)
    assert h5['spps']['n_paired'] == 1                 # Random's own row: the Energetic row never pairs into it (P11)
    assert s['criteria']['frozen']['energetic']['H5']['per_set']['spps']['n_paired'] == 1
    mixed = [dict(r) for r in good('spps', 3)]
    mixed[0]['mode'], mixed[1]['mode'], mixed[2]['mode'] = 'random', 'energetic', 'random'
    with pytest.raises(ValueError):
        score.h2(mixed)


def test_t31_rows_carry_r_m_and_pass_check_inputs(monkeypatch, tmp_path):
    # SPPS: the ball's radius from the report (0.31 m), through rows_from_runs
    rows, _ = W._build(monkeypatch, tmp_path, edt=1.1, truth_val=1.0, split_result=False)
    monkeypatch.undo()
    assert rows[0]['R_m'] == 0.31
    score._check_inputs(rows, None)
    # ISM: the receiver's own R, whatever it is
    import numpy as np
    seen = []

    def fake(room, rec, R, F, step_ms, image_time_s=None, run_s=None, retry_truncated=False):
        seen.append(R)
        return dict(bins=np.array([1.0, 0.0]), dt=step_ms * 1e-3, t_arrival=0.01, half_width=R / 343.2,
                    truth_edt=1.0, truth_share=0.0, truth_status='ok')

    monkeypatch.setattr(ism_fresh, 'make_row', fake)
    monkeypatch.setattr(ism_fresh, 'BANDS_HZ', (125,))
    monkeypatch.setattr(ism_fresh, 'STEPS_MS', (1.0,))
    room = dict(id='drawn:1', dims_m=[20.0, 15.0, 10.0], alpha_walls=[0.2] * 6, source_m=[2.0, 2.0, 2.0], image_time_s=2.4,
                design_t60_s={125: 1.0}, receivers=[dict(position_m=[10.0, 8.0, 5.0], R_m=1.234,
                                                         d_m=math.dist([2.0, 2.0, 2.0], [10.0, 8.0, 5.0]), **{'class': 'mid'})])
    ir = ism_fresh.rows_for_scoring(dict(seed=1, rooms=[room], rejections={}, corpus_rooms_sha256='x'))
    assert ir[0]['R_m'] == 1.234
    score._check_inputs(ir, None)
    monkeypatch.undo()
    # Synth: the spec's R
    from m8b import score_heldout
    spec = synth_fresh.make_spec(id='s', ratio=1.5, step_ms=1.0, t60_s=1.0, late_share_db=-10.0, drr_db=0.0, R_m=0.77,
                                 d_m=3.0, gap_ms=5.0, delay_ms=0.0, run_over_t60=2.0)
    sr = score_heldout.build_synth([spec])
    assert sr[0]['R_m'] == 0.77
    score._check_inputs(sr, None)


# ---- T38: the planted dead room --------------------------------------------------------------------------------------
def test_t38_dead_room_is_refused_by_round_2_and_wrong_under_round_1():
    room, rec, r = dead_room_row()
    assert 0.06 < r['truth_edt'] < 0.07 and r['truth_status'] == 'ok'
    meta = {'half_width': r['half_width']}
    new = method.load().analyse(r['bins'], r['dt'], r['t_arrival'], meta)
    assert new['status'] == 'refused' and new['reason'] == 'receiver_too_large'
    old = method.load(FROZEN, pin=FROZEN_SHA256).analyse(r['bins'], r['dt'], r['t_arrival'], meta)    # control: round 1 as it was
    assert old['status'] == 'ok' and abs(old['edt'] / r['truth_edt'] - 1.0) > 0.05
    c = score.classify(dict(set='ism', id='x', status=old['status'], edt=old['edt'], edt_lo=old['edt_lo'], edt_hi=old['edt_hi'],
                            reason=old['reason'], truth=r['truth_edt'], truth_status='ok'))
    assert c['wrong_silent'] is True
