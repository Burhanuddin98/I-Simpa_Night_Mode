"""T6-T9 (HARNESS-PLAN.md section 4): the ISM generator against the corpus, and the truth of P15-P19.

The rows are chosen by conftest.pick_rows's rule (first, middle and last finite-truth row of a room,
by id), fixed before any harness code existed. A scratch check on 2026-10-01 (corpus rows only, no
harness code) found T6 reproducible bit for bit and T7's split within +0.18 % on these rows.
"""
import math
import statistics

import numpy as np
from conftest import parse_ism_id, pick_rows

from m8b import ism_fresh, truth

R_CORPUS = 0.3100000023841858        # attack_ism.py:29, float32(0.31)
C_CORPUS = 343.20001220703125        # attack_ism.py:28, float32(343.2)


def rel(a, b):
    return abs(a / b - 1.0)


def test_t06_ism_rows_reproduce_bins_and_truth(ism_rows, corpus_rooms):
    """T6: 12 rows of ism_rows.pkl, 3 from each corpus room, reproduced through ism_fresh (P20):
    bins and truth to 1e-9 relative, and the same step, arrival and R/c."""
    rooms = {r['id'].split(':', 1)[1]: r for r in corpus_rooms['rooms'] if r['set'] == 'ism'}
    picked = [r for name in ('t1', 'corridor', 'dead', 'deader') for r in pick_rows(ism_rows, name)]
    assert len(picked) == 12
    for row in picked:
        name, rec, band, step_ms = parse_ism_id(row['id'])
        room = rooms[name]
        got = ism_fresh.make_row(room, rec, R_CORPUS, band, step_ms, image_time_s=room['image_time_s'])
        assert got is not None, '%s: ism_fresh.make_row returned nothing' % row['id']
        b = np.asarray(got['bins'], float)
        ref = np.asarray(row['bins'], float)
        assert len(b) == len(ref), '%s: %d bins, corpus %d' % (row['id'], len(b), len(ref))
        assert np.all(np.abs(b - ref) <= 1e-9 * np.abs(ref)), '%s: bins differ' % row['id']
        assert rel(got['truth_edt'], row['truth_edt']) <= 1e-9, row['id']
        assert rel(got['dt'], row['dt']) <= 1e-12
        assert rel(got['t_arrival'], row['t_arrival']) <= 1e-12
        assert rel(got['half_width'], row['meta']['half_width']) <= 1e-12


def test_t07_split_rule_on_corpus_echograms(ism_rows, corpus_ism):
    """T7: echograms of the corpus rooms t1 and corridor, as a 0.1 ms SPPS reference records them (air
    per step), summed (K = 4) and read by P15's split agree with truth_ideal on the true split (the
    corpus's truth, 0.02 ms, continuous air) within 0.3 %. The copied truth_ideal reproduces that
    truth exactly on the true split."""
    I = corpus_ism
    dl = I.C * I.DT_F
    k01 = 5                                                    # 0.1 ms = 5 fine bins of 0.02 ms
    cache = {}
    picked = [r for name in ('t1', 'corridor') for r in pick_rows(ism_rows, name)]
    assert len(picked) == 6
    for row in picked:
        name, rec, band, _ = parse_ism_id(row['id'])
        L, alpha, src, T = I.ROOMS[name]
        if (name, rec) not in cache:
            cache[(name, rec)] = I.ism.echogram(L, src, rec, I.R, alpha, I.C * T, dl)
        direct, refl = cache[(name, rec)]
        m = I.ism.m_energy(band)
        t = math.dist(src, rec) / I.C
        h = I.R / I.C
        cont = I.ism.air_factor(len(direct), dl, m, I.C, None)
        exact = truth.truth_ideal(direct * cont, refl * cont, t, I.DT_F)
        assert exact is not None, 'truth.truth_ideal returned nothing'
        assert rel(exact[0], row['truth_edt']) <= 1e-12, '%s: the copied truth_ideal is not the corpus truth' % row['id']
        v = (direct + refl) * I.ism.air_factor(len(direct), dl, m, I.C, k01 * I.DT_F)
        n = len(v) // k01
        ref01 = v[:n * k01].reshape(n, k01).sum(1)
        got = truth.read(4.0 * ref01, k01 * I.DT_F, t, h)
        assert got is not None and math.isfinite(got), row['id']
        assert rel(got, row['truth_edt']) <= 0.003, '%s: P15 %.6g, true split %.6g' % (row['id'], got, row['truth_edt'])


def test_t08_blocked_rule_on_the_occluded_construction(synth):
    """T8: the critique's occluded construction (no direct sound, the first energy `gap` after the
    geometric arrival; critique/scan_i1_occluded.py:12-23) at 0.1 ms, read by P15's blocked rule
    (direct 0, t at the start of the first bin with energy), agrees within 0.3 % with
    truth_edt(t_arr + gap, 0, ...)."""
    dt = 1e-4
    h = 0.31 / synth.C
    for T60 in (0.4, 0.8, 1.5, 3.0):
        for delay_ms in (2.0, 5.0, 10.0, 20.0, 40.0):
            k = 6 * math.log(10) / T60
            t_arr = 0.030 + 0.41 * dt
            gap = delay_ms * 1e-3
            b = synth.histogram(dt, t_arr + gap + 2.0 * T60, t_arr, h, 0.0, gap, [1.0], [k])
            want = synth.truth_edt(t_arr + gap, 0.0, 0.0, [1.0], [k])
            sp = truth.split(b, dt, t_arr, h, blocked=True)
            assert sp is not None, 'truth.split returned nothing'
            direct, refl, t = sp
            first = int(np.nonzero(b > 0)[0][0])
            assert not np.any(np.asarray(direct)) and np.array_equal(np.asarray(refl), b)
            assert abs(t - first * dt) <= 1e-12
            got = truth.read(b, dt, t_arr, h, blocked=True)
            assert rel(got, want) <= 0.003, 'T60 %g, %g ms late: %.6g against %.6g' % (T60, delay_ms, got, want)


def test_t09_planted_references_uncertainty_and_exclusions(synth):
    """T9: four planted references give u as P19 defines it (SD, ddof 1, of the K EDTs / sqrt(K) / the
    summed truth); rows above 1 % are excluded as truth_uncertain; a last-10 % share above 1e-6 gives
    truth_truncated (P18); no energy gives truth_nan. The edges themselves are kept."""
    assert truth.verdict(1.0, 0.01, 1e-6) == 'ok'
    assert truth.verdict(1.0, 0.0100001, 1e-7) == 'truth_uncertain'
    assert truth.verdict(1.0, 0.001, 1.0001e-6) == 'truth_truncated'
    assert truth.verdict(float('nan'), 0.0, 0.0) == 'truth_nan'
    edts = [1.00, 1.02, 0.98, 1.01]
    assert rel(truth.uncertainty(edts, 1.0025), statistics.stdev(edts) / 2.0 / 1.0025) <= 1e-12

    dt = 1e-4
    h = 0.31 / synth.C
    t_arr, gap = 0.02, 0.002

    def planted(T60s, run_s):
        out, exact = [], []
        for T60 in T60s:
            k = 6 * math.log(10) / T60
            Ed = (1.0 / k) * 10 ** (-3.0 / 10)
            out.append(synth.histogram(dt, run_s, t_arr, h, Ed, gap, [1.0], [k]))
            exact.append(synth.truth_edt(t_arr, Ed, gap, [1.0], [k]))
        return out, exact

    def check_u(res, refs):
        n = min(len(r) for r in refs)
        total = sum(np.asarray(r[:n], float) for r in refs)
        assert rel(res['truth'], truth.read(total, dt, t_arr, h)) <= 1e-12
        for e, r in zip(res['edts'], refs):
            assert rel(e, truth.read(r, dt, t_arr, h)) <= 1e-12
        assert rel(res['u'], statistics.stdev(res['edts']) / math.sqrt(4) / res['truth']) <= 1e-12
        return total

    refs, exact = planted((0.800, 0.802, 0.798, 0.801), 2.0)
    a = truth.assess(refs, dt, t_arr, h)
    assert a is not None, 'truth.assess returned nothing'
    check_u(a, refs)
    for e, x in zip(a['edts'], exact):
        assert rel(e, x) <= 0.003
    assert 0.0 < a['u'] < 0.01 and a['share'] <= 1e-6 and a['status'] == 'ok'

    refs, _ = planted((0.70, 0.80, 0.90, 1.00), 2.0)
    b = truth.assess(refs, dt, t_arr, h)
    check_u(b, refs)
    assert b['u'] > 0.01 and b['status'] == 'truth_uncertain'

    refs, _ = planted((0.8, 0.8, 0.8, 0.8), 0.25)
    c = truth.assess(refs, dt, t_arr, h)
    total = check_u(c, refs)
    own = float(total[len(total) - len(total) // 10:].sum() / total.sum())
    assert rel(c['share'], own) <= 1e-9 and rel(truth.last10_share(total), own) <= 1e-9
    assert c['share'] > 1e-6 and c['status'] == 'truth_truncated'

    d = truth.assess([np.zeros(2000)] * 4, dt, t_arr, h)
    assert d['status'] == 'truth_nan'
