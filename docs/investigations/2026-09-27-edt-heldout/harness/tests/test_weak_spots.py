"""T22 (HARNESS-PLAN.md section 4, P33): weak_spots.json reproduces from its sources, and every weak-spot
value that P33 says binds a fresh set lies inside that set's bounds.

T22a needs corpus.py, written in step 2, and passes before the generators exist: it is the half of
T22 that was built first. T22b needs synth_fresh.BOUNDS and ism_fresh.BOUNDS. T22a runs the frozen
method on corpus inputs only (P4), with 8 worker processes, in about two minutes.

P33's binding rule, as this test applies it:
- Synth-fresh is bound by every row; ISM-fresh by the rows with has_room true (ISM, z3, z3grid and
  the real rows and seeds, whose rooms are noise-cal rooms).
- A row binds only the quantities the set draws (SYNTH_OPEN, ISM_OPEN, in weak_spots.json's own
  vocabulary), not those the PREREG fixes (Synth-fresh's rate ratios, DRR, steps and run lengths,
  PREREG.md:37-41), not when the row's step lies outside the range of the set's steps, and not a
  value outside PHYSICS.md's limits (P25: T60 0.1-10 s, DRR -40 to +30 dB; nothing else the
  product refuses appears in a weak row).
- A bound may be wider than P21-P24's ranges, never narrower.
"""
import json

from conftest import HARNESS

from m8b import corpus, ism_fresh, synth_fresh

SYNTH_OPEN = ('t60_s', 'late_share_db', 'R_m', 'd_m', 'd_minus_R_m', 'gap_ms', 'delay_ms')              # P23, P24
ISM_OPEN = ('L1_m', 'L2_m', 'L3_m', 'V_m3', 'alpha_walls', 't60_design_125_s', 'R_m', 'd_m', 'd_minus_R_m',
            'rec_wall_minus_R_m', 'src_wall_m', 'band_hz')                                                # P21, P22
PLAN = {'synth': {'t60_s': (0.1, 10.0), 'late_share_db': (-20.0, -3.0), 'R_m': (0.1, 1.5), 'd_m': (0.4, 30.0),
                  'gap_ms': (0.0, 40.0), 'delay_ms': (0.0, 60.0)},
        'ism': {'L1_m': (4.0, 24.0), 'L2_m': (3.5, 14.0), 'L3_m': (2.6, 7.0), 'V_m3': (60.0, 2500.0),
                'alpha_walls': (0.02, 0.80), 't60_design_125_s': (0.1, 3.0), 'R_m': (0.1, 1.5), 'band_hz': (125, 20000)}}
PHYSICS = {'t60_s': (0.1, 10.0), 't60_design_125_s': (0.1, 10.0), 'drr_db': (-40.0, 30.0)}


def test_t22a_weak_spots_json_reproduces_from_its_sources(tmp_path, target_root):
    """T22, first half: corpus.py, run again on its sources, writes weak_spots.json byte for byte as
    committed, its `generated` block (versions and timings) aside."""
    committed = (HARNESS / 'weak_spots.json').read_bytes().replace(b'\r\n', b'\n')
    old = json.loads(committed)
    new = corpus.build_weak_spots(target_root, workers=8)
    assert list(new)[-1] == 'generated' and list(old)[-1] == 'generated'
    new['generated'] = old['generated']
    out = tmp_path / 'weak_spots.json'
    corpus.write_json(out, new)
    assert out.read_bytes() == committed, 'weak_spots.json no longer reproduces from its sources'
    assert len(new['rows']) == 575


def test_t22b_every_binding_weak_spot_lies_inside_the_fresh_sets_bounds():
    """T22, second half: P33's bounds hold every binding weak-spot value."""
    w = json.loads((HARNESS / 'weak_spots.json').read_text(encoding='utf-8'))
    failures = []
    for name, mod, open_q, needs_room in (('synth', synth_fresh, SYNTH_OPEN, False), ('ism', ism_fresh, ISM_OPEN, True)):
        B = mod.BOUNDS
        missing = [q for q in open_q if q not in B]
        assert not missing, '%s_fresh.BOUNDS has no bound for %s' % (name, missing)
        for q, (lo, hi) in PLAN[name].items():
            assert B[q][0] <= lo and B[q][1] >= hi, '%s %s: %s is narrower than the plan\'s %s' % (name, q, B[q], (lo, hi))
        s_lo, s_hi = min(mod.STEPS_MS), max(mod.STEPS_MS)
        for row in w['rows']:
            if needs_room and not row['has_room']:
                continue
            q = row['q']
            step = q.get('step_ms')
            if step is None or not (s_lo - 1e-9 <= step <= s_hi + 1e-9):
                continue
            for k in open_q:
                v = q.get(k)
                if v is None:
                    continue
                for x in (v if isinstance(v, list) else [v]):
                    if k in PHYSICS and not (PHYSICS[k][0] <= x <= PHYSICS[k][1]):
                        continue
                    lo, hi = B[k]
                    if not (lo - 1e-12 * max(1.0, abs(lo)) <= x <= hi + 1e-12 * max(1.0, abs(hi))):
                        failures.append('%s %s = %r outside %s: %s' % (name, k, x, B[k], row['id']))
    assert not failures, '%d weak-spot values outside their set\'s bounds (P33: widen the bound):\n%s' % (
        len(failures), '\n'.join(failures[:40]))
