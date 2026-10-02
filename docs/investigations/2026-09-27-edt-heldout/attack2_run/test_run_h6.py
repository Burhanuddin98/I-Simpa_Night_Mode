"""Tests for run_h6.py (ADDENDUM-B2.md). Written before run_h6.py; recorded failing then passing in RED-GREEN.md.
Run from this folder:  python -m pytest -p no:cacheprovider --basetemp=<scratch> test_run_h6.py
Nothing here runs a real attack class through the method."""
import json
import math
import sys
from pathlib import Path

sys.dont_write_bytecode = True
import numpy as np
import pytest

HERE = Path(__file__).resolve().parent
D = HERE.parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(D / 'harness2'))

import run_h6                                   # noqa: E402  (imports harness2 read-only)
from m8b import attack, score, synth_fresh      # noqa: E402

C = 343.2


def point_class(spec, noise=None):
    """A class whose box is one point: the draws are then exactly the Synth-fresh-2 spec's parameters."""
    cls = {'generator': 'synth',
           'params': {k: [spec[k], spec[k]] for k in ('t60_s', 'drr_db', 'd_m', 'delay_ms', 'gap_ms', 'ratio',
                                                      'late_share_db')},
           'noise': noise or {'kind': 'none'}, 'step_ms': spec['step_ms'],
           'run_over_t60': [spec['run_over_t60']] * 2, 'R_m': [spec['R_m']] * 2}
    assert attack.validate_class(cls) == (True, None)
    return cls


def a_spec(delayed=True, step_ms=1.0):
    for s in synth_fresh.preview_draw(2026100202):
        if (s['step_ms'] == step_ms and (s['delay_ms'] > 0) == delayed and s['drr_db'] < 9 and s['t60_s'] < 1.5
                and s['d_m'] < 20 and s['run_over_t60'] < 1.0):
            return s
    raise AssertionError('no spec')


def test_equivalence_with_synth_fresh_bit_for_bit():
    for delayed in (True, False):
        s = a_spec(delayed)
        cls = point_class(s)
        inst = attack.draws(cls)[0]
        row = run_h6.draw_to_row(cls, inst, 0)
        assert np.array_equal(row['bins'], synth_fresh.histogram(s))
        assert row['bins'].tobytes() == synth_fresh.histogram(s).tobytes()
        assert row['t_arrival'] == s['t_arrival']
        assert row['truth'] == synth_fresh.truth(s) and math.isfinite(row['truth'])
        assert row['dt'] == s['dt'] and row['meta'] == {'half_width': s['half_width']}
        assert row['truth_status'] == 'ok' and row['set'] == 'attack' and row['R_m'] == s['R_m']
        assert row['class_id'] == run_h6.class_id_of(cls) and row['id'].startswith('attack|')


def test_draws_reproducible_and_seeded():
    cls = json.loads((D / 'attack2/classes/c04.json').read_text())
    a, b = attack.draws(cls), attack.draws(cls)
    assert len(a) == 20 and a == b
    assert attack.ATTACK_SEED == 2026100203
    other = attack.draws(json.loads((D / 'attack2/classes/c05.json').read_text()))
    assert a != other
    # the seed enters the draw: a different attack seed gives different instances
    old = attack.ATTACK_SEED
    try:
        attack.ATTACK_SEED = 1
        assert attack.draws(cls) != a
    finally:
        attack.ATTACK_SEED = old


def test_delay_rounded_up_to_whole_step_as_arrival_s():
    # 2.3 ms at a 1 ms step emits at step 3; 3.0 ms at 1 ms is exactly step 3 (float32 as results/spps.rs:49-52)
    for delay_ms, step_ms in ((2.3, 1.0), (3.0, 1.0), (0.4, 0.25), (5.5, 2.0)):
        s = a_spec(True, 1.0)
        s = dict(s, delay_ms=delay_ms, step_ms=step_ms)
        cls = point_class(s)
        inst = attack.draws(cls)[0]
        row = run_h6.draw_to_row(cls, inst, 0)
        dt = step_ms * 1e-3
        steps = int(np.ceil(np.float32(delay_ms * 1e-3) / np.float32(dt)))
        assert steps * dt >= delay_ms * 1e-3 - 1e-12 and (steps - 1) * dt < delay_ms * 1e-3
        assert row['t_arrival'] == steps * dt + s['d_m'] / C        # the product's arrival_s, not d/c
        assert row['t_arrival'] > s['d_m'] / C
        assert not row['bins'][:steps].any()                        # nothing before the emission step
        assert row['bins'][steps:].any()


def test_noise_reproducible_and_mean_converges():
    base = a_spec(True, 1.0)
    cls = point_class(base, {'kind': 'compound_poisson', 'particles_per_source': [1e5, 1e5]})
    inst = attack.draws(cls)[0]
    clean_cls = point_class(base)
    clean_row = run_h6.draw_to_row(clean_cls, attack.draws(clean_cls)[0], 0)
    clean = clean_row['bins']
    r0a, r0b = run_h6.draw_to_row(cls, inst, 0), run_h6.draw_to_row(cls, inst, 0)
    r1 = run_h6.draw_to_row(cls, inst, 1)
    assert np.array_equal(r0a['bins'], r0b['bins'])                 # same class, same draw index: same noise
    assert not np.array_equal(r0a['bins'], r1['bins'])              # another draw index: other noise
    assert r0a['truth'] == clean_row['truth'] and r0a['t_arrival'] == clean_row['t_arrival']
    # statistical check: the mean over reps of block sums sits within 4.5 standard errors of the noiseless block
    # sums (4.0 for the total). A block's variance per rep is w_dir*direct + w_rev*reverb (compound Poisson).
    reps = 200
    acc = np.zeros_like(clean)
    for i in range(reps):
        acc += run_h6.draw_to_row(cls, inst, 100 + i)['bins']
    mean = acc / reps
    spec = run_h6.spec_of(cls, inst)
    w_dir, w_rev = run_h6.noise_weights(spec, inst['particles_per_source'])
    d_part, r_part = run_h6.split_histogram(spec)
    assert np.allclose(d_part + r_part, clean, rtol=1e-12, atol=0)
    for blk in np.array_split(np.arange(len(clean)), 10):
        var = (w_dir * d_part[blk].sum() + w_rev * r_part[blk].sum()) / reps
        z = (mean[blk].sum() - clean[blk].sum()) / math.sqrt(var)
        assert abs(z) < 4.5, z
    z = (mean.sum() - clean.sum()) / math.sqrt((w_dir * d_part.sum() + w_rev * r_part.sum()) / reps)
    assert abs(z) < 4.0, z


def test_ism_class_not_implemented():
    cls = {'generator': 'ism', 'params': {'t60_s': [0.5, 1.0]}, 'noise': {'kind': 'none'}, 'step_ms': 1.0,
           'run_over_t60': [1.0, 2.0], 'R_m': [0.3, 0.5]}
    with pytest.raises(NotImplementedError) as e:
        run_h6.draw_to_row(cls, {'t60_s': 0.7, 'R_m': 0.3, 'run_over_t60': 1.0, 'step_ms': 1.0}, 0)
    assert 'no ism class' in str(e.value).lower()


PLAUS = json.dumps({'verdict': 'plausible', 'reasons': 'inside the limits'})
IMPLA = json.dumps({'verdict': 'implausible', 'reasons': 'volume above 1e8'})


def test_h6_wiring_on_planted_classes():
    rows = [{'id': 'p1', 'n_wrong_silent': 7, 'votes': [PLAUS, PLAUS, PLAUS]},          # reproducible and plausible
            {'id': 'p2', 'n_wrong_silent': 20, 'votes': [IMPLA, IMPLA, IMPLA]},         # reproducible, 3/3 implausible
            {'id': 'p3', 'n_wrong_silent': 4, 'votes': [PLAUS, PLAUS, PLAUS]}]          # not reproducible
    r = run_h6.run_score_h6(rows)
    by = {c['id']: c for c in r['classes']}
    assert by['p1']['pass'] is False and by['p1']['reproducible'] and by['p1']['panel'] == 'plausible'
    assert by['p2']['pass'] is True and by['p2']['panel'] == 'implausible'
    assert by['p3']['pass'] is True and not by['p3']['reproducible']
    assert r['pass'] is False
    assert run_h6.run_score_h6([rows[1], rows[2]])['pass'] is True
    # a 2/3 split is plausible (P26), so a reproducible class with one dissent still fails
    assert run_h6.run_score_h6([{'id': 'p4', 'n_wrong_silent': 9, 'votes': [IMPLA, IMPLA, PLAUS]}])['pass'] is False


def test_vote_parsing_of_judge_files():
    votes = run_h6.read_votes([D / 'attack2/judge1.txt', D / 'attack2/judge2.txt', D / 'attack2/judge3.txt'])
    assert sorted(votes) == ['c%02d' % i for i in range(1, 12)]
    assert all(len(v) == 3 for v in votes.values())
    assert all(attack.parse_vote(raw) == 'plausible' for v in votes.values() for raw in v)    # all 33 plausible
    assert attack.panel(votes['c01']) == 'plausible'
    assert run_h6.parse_vote_line('c07.json: ' + IMPLA) == ('c07', IMPLA)
    assert run_h6.parse_vote_line('') is None
    with pytest.raises(ValueError):
        run_h6.parse_vote_line('c07: ' + IMPLA)


def test_classes_load_and_validate():
    cls = run_h6.load_classes(D / 'attack2/classes')
    assert sorted(cls) == ['c%02d' % i for i in range(1, 12)]
    assert all(attack.validate_class(c) == (True, None) for c in cls.values())
