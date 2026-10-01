"""T36 and T37 (../../HARNESS-PLAN-2.md sections 3 and 6, with section 9 M3 and m5): ISM-fresh-2 and Synth-fresh-2.

A held-out seed is drawn only through preview_draw(), which makes geometry and parameters and nothing else (no
echogram, no histogram, no method call), as the plan's own preview did and as preview_pin.json (M3) needs; draw()
itself stays shut before B1 and refuses round 1's seeds for good. The references are the tests' own: P3 typed in
test_r2_corpus_rooms.gap, the BOUNDS and counts typed from round 1's contract, run 1's record in counts.json.
"""
import hashlib
import json
import math
from pathlib import Path

import numpy as np
from conftest import HARNESS

from m8b import driver, ism_fresh, synth_fresh
from test_r2_corpus_rooms import gap
from test_r2_plan_guards import refused

COUNTS_JSON = Path(r'B:\data\m8b-edt\results\run1\counts.json')
PARENTS = ('t1', 'corridor', 'dead', 'deader')
REJECTIONS = ('relative_near_duplicate', 'relative_over_image_cap', 'drawn_V_outside', 'drawn_t60_125_outside',
              'drawn_near_duplicate', 'drawn_over_image_cap', 'receiver_R_no_room', 'receiver_position_outside_class',
              'receiver_R_position_cap')


def canon(x):
    return hashlib.sha256(json.dumps(x, sort_keys=True, separators=(',', ':')).encode('utf-8')).hexdigest()


def check_ism_draw(D, list2, counts):
    assert [r['id'] for r in D['rooms']][:4] == ['rel:' + p for p in PARENTS]
    assert [r['id'] for r in D['rooms']][4:] == ['drawn:%d' % i for i in range(1, 9)]
    assert D['corpus_rooms_sha256'] == hashlib.sha256((HARNESS / 'corpus_rooms_2.json').read_bytes()).hexdigest()
    assert set(D['rejections']) == set(REJECTIONS)
    run1_rel = {x['parent']: x for x in counts['ism_rooms'] if x['kind'] == 'relative'}
    assert sorted(run1_rel) == sorted(PARENTS)
    for room in D['rooms']:
        entry = dict(kind='box', dims_sorted_m=sorted(room['dims_m'], reverse=True))
        gaps = [(gap(entry, c), c['id']) for c in list2 if c['kind'] == 'box']
        assert min(g for g, _ in gaps) > 0.10, (room['id'], min(gaps))
        assert room['images'] <= 1e8 and len(room['receivers']) == 12
        for r in room['receivers']:
            assert 0.1 <= r['R_m'] <= 1.5 and math.isclose(r['d_m'], math.dist(room['source_m'], r['position_m']))
        assert [r['class'] for r in room['receivers']][:8] == ['near'] * 4 + ['mid'] * 4
        if room['kind'] == 'relative':       # fresh against run 1's relative of the same parent, pairwise
            theirs = dict(kind='box', dims_sorted_m=sorted(run1_rel[room['parent']]['dims_m'], reverse=True))
            assert gap(entry, theirs) > 0.10, room['id']


def test_t36_ism_fresh_2():
    refused('heldout_before_b1', ism_fresh.draw, driver.ISM_SEED, b1=False)
    refused('round1_seed', ism_fresh.draw, 2026100102, b1=True)
    refused('round1_seed', ism_fresh.draw, 2026100102, b1=False)
    list2 = json.loads((HARNESS / 'corpus_rooms_2.json').read_text(encoding='utf-8'))['rooms']
    counts = json.loads(COUNTS_JSON.read_text(encoding='utf-8'))
    dev = ism_fresh.draw(20261001)                     # a dev seed: allowed, same code path
    check_ism_draw(dev, list2, counts)
    real = ism_fresh.preview_draw(2026100201)          # the real seed, geometry only
    check_ism_draw(real, list2, counts)
    again = ism_fresh.preview_draw(2026100201)
    assert real['seed'] == 2026100201 and canon(again) == canon(real), 'reproducible'
    assert real['rejections'] == again['rejections'] and all(isinstance(v, int) for v in real['rejections'].values())


# ---- T37 -------------------------------------------------------------------------------------------------------------
BOUNDS_R1 = {'t60_s': (0.1, 10.0), 'late_share_db': (-20.0, -3.0), 'drr_db': (-20.0, 10.0), 'R_m': (0.1, 1.5),
             'd_m': (0.4, 30.8022), 'd_minus_R_m': (0.2, 30.8022 - 0.1), 'gap_ms': (0.0, 40.0),
             'delay_ms': (0.0, 60.00000284984708), 'run_over_t60': (0.3, 3.0)}
CONT = ('t60_s', 'R_m', 'd_m', 'gap_ms', 'delay_ms', 'run_over_t60')        # compared by relative 10 %
DB = {'late_share_db': 17.0, 'drr_db': 30.0}                                 # compared by 10 % of the range's width


def test_t37_synth_fresh_2():
    refused('heldout_before_b1', synth_fresh.draw, driver.SYNTH_SEED, b1=False)
    refused('round1_seed', synth_fresh.draw, 2026100101, b1=True)
    assert synth_fresh.BOUNDS == BOUNDS_R1 and synth_fresh.RATIOS == (1.5, 5.0)
    assert synth_fresh.STEPS_MS == (1.0, 2.0, 5.0, 10.0)
    specs = synth_fresh.preview_draw(2026100202)
    assert len(specs) == 4000 and len({s['id'] for s in specs}) == 4000
    for s in specs:
        for k, (lo, hi) in BOUNDS_R1.items():
            if k == 'd_minus_R_m':
                assert lo <= s['d_m'] - s['R_m'] <= hi, s['id']
            else:
                assert lo <= s[k] <= hi, (s['id'], k)
    cells = {}
    for s in specs:
        cells.setdefault((s['ratio'], s['step_ms']), []).append(s)
    assert sorted(len(v) for v in cells.values()) == [500] * 8
    assert all(sum(1 for s in v if s['delay_ms'] == 0.0) == 250 for v in cells.values()), 'half of each cell has no delay'
    again = synth_fresh.preview_draw(2026100202)
    assert canon(again) == canon(specs), 'reproducible from the seed'
    # no spec equals, or lies within P3's 10 % of, one of round 1's, parameter by parameter, in its own cell (m5)
    old = synth_fresh.preview_draw(2026100101)
    assert len(old) == 4000
    key = lambda s: tuple(s[k] for k in ('ratio', 'step_ms', 't60_s', 'late_share_db', 'drr_db', 'R_m', 'd_m', 'gap_ms',
                                         'delay_ms', 'run_over_t60'))
    assert not {key(s) for s in specs} & {key(s) for s in old}, 'a round-2 spec equals one of round 1\'s'
    near = 0
    for cell, new_cell in cells.items():
        old_cell = [s for s in old if (s['ratio'], s['step_ms']) == cell]
        A = {k: np.array([s[k] for s in new_cell]) for k in CONT + tuple(DB)}
        B = {k: np.array([s[k] for s in old_cell]) for k in CONT + tuple(DB)}
        ok = np.ones((len(new_cell), len(old_cell)), bool)
        for k in CONT:
            a, b = A[k][:, None], B[k][None, :]
            both0 = (a == 0) & (b == 0)
            with np.errstate(divide='ignore', invalid='ignore'):
                ok &= both0 | (np.abs(a / b - 1.0) <= 0.10)
        for k, width in DB.items():
            ok &= np.abs(A[k][:, None] - B[k][None, :]) <= 0.10 * width
        near += int(ok.sum())
    assert near == 0, '%d round-2 specs lie within 10 %% of a round-1 spec on every parameter' % near
