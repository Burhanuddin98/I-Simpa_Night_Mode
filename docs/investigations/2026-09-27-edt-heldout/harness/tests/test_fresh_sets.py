"""T15-T16 (HARNESS-PLAN.md section 4): Synth-fresh and ISM-fresh drawn with a dev seed.

A dev seed is not held out: the held-out seeds are SeedSequence(2026100101) for Synth-fresh and
SeedSequence(2026100102) for ISM-fresh (P21, P24), refused before A1 (T13). Bounds may be wider than
P21-P24's ranges where P33 widens them (T22), never narrower.
"""
import collections
import itertools
import json
import math

import numpy as np
from conftest import Union, p3_near_duplicate

from m8b import corpus, ism_fresh, synth_fresh

DEV_SEED = 20261001
C_SYNTH = 343.2
P24 = {'t60_s': (0.1, 10.0), 'late_share_db': (-20.0, -3.0), 'R_m': (0.1, 1.5), 'd_m': (0.4, 30.0),
       'gap_ms': (0.0, 40.0), 'delay_ms': (0.0, 60.0)}
PREREG_SYNTH = {'drr_db': (-20.0, 10.0), 'run_over_t60': (0.3, 3.0)}           # PREREG.md:39, :41
P21 = {'L1_m': (4.0, 24.0), 'L2_m': (3.5, 14.0), 'L3_m': (2.6, 7.0), 'V_m3': (60.0, 2500.0),
       'alpha_walls': (0.02, 0.80), 't60_design_125_s': (0.1, 3.0)}
P22 = {'R_m': (0.1, 1.5), 'band_hz': (125, 20000)}
ISM_BANDS = (125, 250, 500, 1000, 2000, 4000, 8000, 16000, 20000)
C_ISM = 343.20001220703125


def rel(a, b):
    return abs(a / b - 1.0)


def inside(x, box, tol=1e-12):
    lo, hi = box
    return lo - tol * max(1.0, abs(lo)) <= x <= hi + tol * max(1.0, abs(hi))


def canon(x):
    return json.dumps(x, sort_keys=True, default=lambda o: o.tolist() if hasattr(o, 'tolist') else float(o))


def test_t15_synth_fresh_draws_p24(synth):
    """T15: 500 rows per family x step, rate ratios 1.5 and 5 only, P24's ranges (the PREREG's DRR,
    steps and run lengths exactly), half of each cell undelayed, a delayed row equal to its undelayed
    twin shifted by whole steps, the truth synth.truth_edt's; reproducible from a dev seed."""
    rows = synth_fresh.draw(DEV_SEED)
    assert rows is not None, 'synth_fresh.draw returned nothing'
    assert len(rows) == 4000
    cells = collections.Counter((r['ratio'], r['step_ms']) for r in rows)
    assert cells == {(ratio, step): 500 for ratio in (1.5, 5.0) for step in (1.0, 2.0, 5.0, 10.0)}
    B = synth_fresh.BOUNDS
    for q, (lo, hi) in P24.items():
        assert B[q][0] <= lo and B[q][1] >= hi, 'P24 %s: %s narrower than %s' % (q, B[q], (lo, hi))
    assert B['d_minus_R_m'][0] <= 0.2
    for r in rows:
        dt = r['step_ms'] * 1e-3
        assert abs(r['dt'] - dt) <= 1e-15
        k1, k2 = r['k']
        assert rel(k1, 6 * math.log(10) / r['t60_s']) <= 1e-12 and rel(k2, k1 / r['ratio']) <= 1e-12
        A1, A2 = r['A']
        assert A1 == 1.0 and rel((A2 / k2) / (A1 / k1), 10 ** (r['late_share_db'] / 10)) <= 1e-9
        assert rel(r['Ed'], (A1 / k1 + A2 / k2) * 10 ** (r['drr_db'] / 10)) <= 1e-12
        for q, box in PREREG_SYNTH.items():
            assert inside(r[q], box), (r['id'], q, r[q])
        for q in P24:
            assert inside(r[q], B[q]), (r['id'], q, r[q])
        assert r['d_m'] - r['R_m'] >= B['d_minus_R_m'][0] - 1e-12
        assert rel(r['half_width'], r['R_m'] / C_SYNTH) <= 1e-12
        steps = math.ceil(float(np.float32(r['delay_ms'] * 1e-3) / np.float32(dt)))      # results/spps.rs:49-52
        assert r['emission_steps'] == steps and abs(r['emission_s'] - steps * dt) <= 1e-12
        assert abs(r['t_arrival'] - (r['emission_s'] + r['d_m'] / C_SYNTH)) <= 1e-12
        assert abs(r['run_s'] - (r['t_arrival'] + r['run_over_t60'] * r['t60_s'])) <= 1e-9
    # the draws reach across P24's ranges (log-uniform T60 and R, uniform otherwise)
    for q, (lo, hi), log in (('t60_s', P24['t60_s'], True), ('R_m', P24['R_m'], True), ('d_m', P24['d_m'], False),
                             ('gap_ms', P24['gap_ms'], False), ('late_share_db', P24['late_share_db'], False),
                             ('drr_db', PREREG_SYNTH['drr_db'], False), ('run_over_t60', PREREG_SYNTH['run_over_t60'], False)):
        v = np.array([r[q] for r in rows], float)
        f = (lambda x: math.log(x)) if log else (lambda x: x)
        span = f(hi) - f(lo)
        assert f(v.min()) <= f(lo) + 0.05 * span and f(v.max()) >= f(hi) - 0.05 * span, (q, v.min(), v.max())
    delayed = [r for r in rows if r['delay_ms'] > 0]
    assert max(r['delay_ms'] for r in delayed) >= 0.95 * 60.0
    for cell, n in cells.items():
        assert sum(1 for r in rows if (r['ratio'], r['step_ms']) == cell and r['delay_ms'] == 0) == n // 2, cell
    # a delayed row is its undelayed twin shifted by whole steps
    for r in [r for r in delayed if r['emission_steps'] > 0 and r['step_ms'] in (1.0, 10.0)][:6]:
        tw = synth_fresh.twin(r)
        assert tw['delay_ms'] == 0 and tw['emission_steps'] == 0 and abs(tw['t_arrival'] - r['d_m'] / C_SYNTH) <= 1e-12
        b, bt, s = synth_fresh.histogram(r), synth_fresh.histogram(tw), r['emission_steps']
        assert len(b) == len(bt) + s and not np.any(b[:s])
        assert np.allclose(b[s:], bt, rtol=1e-9, atol=1e-12 * float(np.max(bt))), r['id']
    for r in rows[:3] + delayed[:3]:
        want = synth.truth_edt(r['t_arrival'], r['Ed'], r['gap_ms'] * 1e-3, r['A'], r['k'])
        assert rel(synth_fresh.truth(r), want) <= 1e-12
    assert canon(synth_fresh.draw(DEV_SEED)) == canon(rows), 'not reproducible from its seed'
    assert canon(synth_fresh.draw(DEV_SEED + 1)) != canon(rows)


def farthest(box_dims, src, margin):
    lo = [margin] * 3
    hi = [d - margin for d in box_dims]
    return max(math.dist(src, c) for c in itertools.product(*zip(lo, hi)))


def n_images(dims, lmax):
    """The images ism.images builds for a box (ism.py:47-65): 2 (2 nmax + 1) per axis, nmax = ceil(lmax / 2L) + 1."""
    return math.prod(2 * (2 * (math.ceil(lmax / (2 * L)) + 1) + 1) for L in dims)


def test_t16_ism_fresh_draws_p21_p22(corpus_rooms):
    """T16: with a dev seed, four relatives (one per corpus ISM room, every axis 12-33 % up, every wall's
    alpha 0.9-1.1 times, the source scaled), eight drawn rooms in their bounds, every room fresh per
    P3 and at most 1e8 images, rejections counted, receivers filling their classes, and the image
    set P22 asks for; reproducible from the seed."""
    D = ism_fresh.draw(DEV_SEED)
    assert D is not None, 'ism_fresh.draw returned nothing'
    R12 = D['rooms']
    assert len(R12) == 12
    assert isinstance(D['rejections'], dict) and all(isinstance(k, str) and int(v) >= 0 for k, v in D['rejections'].items())
    B = ism_fresh.BOUNDS
    for q, (lo, hi) in {**P21, **P22}.items():
        assert B[q][0] <= lo and B[q][1] >= hi, 'P21/P22 %s: %s narrower than %s' % (q, B[q], (lo, hi))
    assert B['rec_wall_minus_R_m'][0] <= 0.04 and B['src_wall_m'][0] <= 1.0 and B['d_minus_R_m'][0] <= 0.2
    assert tuple(ism_fresh.STEPS_MS) == (1.0, 2.0, 5.0) and tuple(ism_fresh.BANDS_HZ) == ISM_BANDS
    assert ism_fresh.RUN_S == 2.0

    parents = {r['id'].split(':', 1)[1]: r for r in corpus_rooms['rooms'] if r['set'] == 'ism'}
    relatives = [r for r in R12 if r['kind'] == 'relative']
    drawn = [r for r in R12 if r['kind'] == 'drawn']
    assert len(relatives) == 4 and len(drawn) == 8
    assert sorted(r['parent'] for r in relatives) == ['corridor', 'dead', 'deader', 't1']
    for r in relatives:
        p = parents[r['parent']]
        f = [a / b for a, b in zip(r['dims_m'], p['dims_m'])]
        assert all(1.12 <= x <= 1.33 for x in f), (r['id'], f)
        g = [a / b for a, b in zip(r['alpha_walls'], p['alpha_walls'])]
        assert all(0.9 <= x <= 1.1 for x in g), (r['id'], g)
        assert all(abs(s - ps * x) <= 1e-9 for s, ps, x in zip(r['source_m'], p['source_m'], f)), r['id']
    for r in drawn:
        L1, L2, L3 = sorted(r['dims_m'], reverse=True)
        for q, v in (('L1_m', L1), ('L2_m', L2), ('L3_m', L3), ('V_m3', L1 * L2 * L3),
                     ('t60_design_125_s', corpus.design_t60(r['dims_m'], r['alpha_walls'], 125.0))):
            assert inside(v, B[q]), (r['id'], q, v)
        assert all(inside(a, B['alpha_walls']) for a in r['alpha_walls']), r['id']

    for r in R12:
        dims, src = r['dims_m'], r['source_m']
        U = Union([((0.0, 0.0, 0.0), dims)])
        for band in ISM_BANDS:
            got = r['design_t60_s'].get(band, r['design_t60_s'].get(str(band)))
            assert rel(got, corpus.design_t60(dims, r['alpha_walls'], float(band))) <= 1e-9, (r['id'], band)
        for c in corpus_rooms['rooms']:
            assert not p3_near_duplicate({'kind': 'box', 'dims_sorted_m': sorted(dims, reverse=True)}, c), (r['id'], c['id'])
        assert U.clearance(src) >= B['src_wall_m'][0] - 1e-12, r['id']
        recs = r['receivers']
        assert len(recs) == 12
        for x in recs:
            d = math.dist(src, x['position_m'])
            assert abs(x['d_m'] - d) <= 1e-9
            assert x['class'] == ('near' if d < 2.0 else ('far' if d > 10.0 else 'mid')), (r['id'], x)
            assert inside(x['R_m'], B['R_m']) and inside(x['R_m'], P22['R_m'])
            assert d - x['R_m'] >= B['d_minus_R_m'][0] - 1e-12
            assert U.clearance(x['position_m']) - x['R_m'] >= B['rec_wall_minus_R_m'][0] - 1e-12, (r['id'], x)
        n = collections.Counter(x['class'] for x in recs)
        assert n['near'] == 4 and n['far'] in (0, 4) and n['mid'] == 8 - n['far'], (r['id'], dict(n))
        if farthest(dims, src, 1.54) > 10.5:
            assert n['far'] == 4, '%s allows far receivers' % r['id']
        if farthest(dims, src, 0.14) <= 10.0:
            assert n['far'] == 0, r['id']
        t_arr = max(x['d_m'] for x in recs) / C_ISM
        t60max = max(corpus.design_t60(dims, r['alpha_walls'], float(b)) for b in ISM_BANDS)
        want = max(1.2 * ism_fresh.RUN_S, t_arr + 1.1 * t60max)
        assert want - 1e-9 <= r['image_time_s'] < want + 0.1 + 1e-9, (r['id'], r['image_time_s'], want)
        assert n_images(dims, C_ISM * r['image_time_s'] + max(x['R_m'] for x in recs)) <= 1e8, r['id']
    assert canon(ism_fresh.draw(DEV_SEED)) == canon(D), 'not reproducible from its seed'
