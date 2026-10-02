"""Step 6's own fault-planting (C:/tmp/m8b-edt/step6-truth/mut/truth_mut.py and
step6-ism/mut/ism_mut.py) found five faults T1-T22 did not catch, each recorded in
C:/tmp/m8b-edt/progress.log: truth.py's verdict() checking 'uncertain' before 'truncated' still
passed T9 (no T9 case has both conditions true at once), and a split() that puts every bin in
'refl' still passed T7/T9 (the rows' direct sound is too weak to move the truth past tolerance).
ism_fresh.py's MAX_IMAGES cap being ignored, draw()'s 'rejections' being emptied, and image_time()
dropping its t_arr term all passed T16/T22b (the single dev-seed draw those tests read never
happens to exercise the cap, a real rejection, or a room whose T60 term dominates image_time's
2.4 s floor). Each test below is built to differ between the correct code and exactly one of
those planted faults, confirmed by temporarily applying the fault from
step6-{truth,ism}/mut/*.py (never committed) and watching the new test fail.
"""
import math

import numpy as np

from m8b import ism_fresh, truth

DEV_SEED = 20261001
C_ISM = 343.20001220703125


# ---- truth.py -------------------------------------------------------------------------------------
def test_truth_verdict_truncated_checked_before_uncertain():
    """P18's truncated check applies before P19's uncertain check (truth.verdict's contract, docstring
    line 26): a row that fails both reads 'truth_truncated', not 'truth_uncertain'. T9's own cases
    never have both conditions true at once (progress.log 108:08:09), so a verdict() that checks them
    in the opposite order still passes T9; this constructs that case directly (plants: verdict_order,
    step6-truth/mut/truth_mut.py)."""
    assert truth.verdict(1.0, 0.5, 0.5) == 'truth_truncated'
    # each rule alone, so the swapped order cannot pass by coincidence
    assert truth.verdict(1.0, 0.5, 0.0) == 'truth_uncertain'
    assert truth.verdict(1.0, 0.0, 0.5) == 'truth_truncated'
    assert truth.verdict(1.0, 0.0, 0.0) == 'ok'
    assert truth.verdict(float('nan'), 0.0, 0.0) == 'truth_nan'


def test_truth_split_keeps_direct_energy_out_of_reflected():
    """P15's split(): bins inside [t_arr - h, t_arr + h) are 'direct', the rest 'refl' (docstring line
    18-19). A split that reads every bin as reflected still passed T7/T9 because those rows' direct
    sound is too weak to move the truth past tolerance (why_no_direct-run1.txt: at most 0.21 % without
    DRR, -9.7 % with it); this checks the two arrays directly, on bins with real energy on both sides
    of the window (plants: no_direct, step6-truth/mut/truth_mut.py)."""
    bins = np.zeros(15)
    bins[8] = 5.0     # inside [8, 12), the window for t_arr=10, h=2, dt=1
    bins[9] = 3.0      # inside
    bins[0] = 2.0      # outside, well before
    bins[13] = 4.0     # outside, after
    direct, refl, t = truth.split(bins, 1.0, 10.0, 2.0)
    assert t == 10.0
    assert direct.sum() == 8.0, 'the direct window energy went missing'
    assert refl.sum() == 6.0, 'energy outside the window leaked into, or out of, refl'
    assert direct[8] == 5.0 and direct[9] == 3.0 and direct[0] == 0.0 and direct[13] == 0.0
    assert refl[0] == 2.0 and refl[13] == 4.0 and refl[8] == 0.0 and refl[9] == 0.0


# ---- ism_fresh.py -----------------------------------------------------------------------------------
def _n_images(dims, lmax):
    """ism.py:47-65's image count (as test_fresh_sets.py's own n_images), independent of ism_fresh's."""
    return math.prod(2 * (2 * (math.ceil(lmax / (2 * L)) + 1) + 1) for L in dims)


def _forced_long_t60_draw(monkeypatch):
    """P21's alpha narrowed to 0.02-0.06 and the 125 Hz T60 range widened to 30 s, in this process
    only (as check_ism.py's part_e does), so some drawn rooms' design T60 is long enough that
    image_time()'s T60 term exceeds the 2.4 s floor and the image cap actually fires. One draw, no
    echogram: 0.01 s (verify_ism.py scratch run, 2026-10-01)."""
    monkeypatch.setattr(ism_fresh, 'ALPHA', (0.02, 0.06))
    monkeypatch.setattr(ism_fresh, 'T60_125_S', (0.1, 30.0))
    return ism_fresh.draw(DEV_SEED)


def test_ism_fresh_image_cap_enforced(monkeypatch):
    """P21/P22: a room whose images would exceed MAX_IMAGES is rejected
    ('drawn_over_image_cap'/'relative_over_image_cap'; ism_fresh.py docstring lines 84-86, 101-102),
    never kept. T16/T22b read a single dev-seed draw that never happens to hit the cap
    (progress.log 135:09:32: 'no cap rejection' on 400 draws); this forces long T60s so it fires
    (plants: no_image_cap, MAX_IMAGES = math.inf, step6-ism/mut/ism_mut.py)."""
    D = _forced_long_t60_draw(monkeypatch)
    cap = D['rejections']['drawn_over_image_cap'] + D['rejections']['relative_over_image_cap']
    assert cap > 0, 'forcing long T60s (low alpha) should make the image cap reject at least one room'
    for r in D['rooms']:
        n = _n_images(r['dims_m'], C_ISM * r['image_time_s'] + max(x['R_m'] for x in r['receivers']))
        assert n <= ism_fresh.MAX_IMAGES, (r['id'], n)


def test_ism_fresh_rejections_reflect_the_draw(monkeypatch):
    """draw()'s 'rejections' dict holds every reason in ism_fresh.REJECTIONS, each a real count of what
    happened during the draw (docstring line 34, 75-77), not an empty dict. T16 only checks that
    'rejections' is a dict of non-negative ints (test_fresh_sets.py:120), which an empty dict also
    satisfies vacuously (plants: rejections_empty, which replaces D['rejections'] with {} after a real
    draw, step6-ism/mut/ism_mut.py)."""
    D = _forced_long_t60_draw(monkeypatch)
    assert set(D['rejections']) == set(ism_fresh.REJECTIONS), D['rejections']
    assert sum(D['rejections'].values()) > 0, 'forcing long T60s/low alpha should trigger a rejection'


def test_ism_fresh_image_time_includes_t_arrival(monkeypatch):
    """P22's image_time_s = max(1.2 RUN_S, t_arr + 1.1 * max(design T60)) (docstring lines 100, 357-360),
    not max(1.2 RUN_S, 1.1 * max(design T60)) with t_arr dropped. On the dev seed's single draw the
    T60 term never exceeds the 2.4 s floor by enough for the missing t_arr term to show up within
    T16's tolerance (progress.log 137:09:38's E3 needed the same alpha-narrowing to expose it); this
    forces that regime and checks the exact formula (plants: image_time_no_tarr, step6-ism/mut/ism_mut.py)."""
    D = _forced_long_t60_draw(monkeypatch)
    want = [max(ism_fresh.IMAGE_RUN_FACTOR * ism_fresh.RUN_S,
                max(x['d_m'] for x in r['receivers']) / C_ISM
                + ism_fresh.IMAGE_T60_FACTOR * max(r['design_t60_s'].values()))
            for r in D['rooms']]
    no_tarr = [max(ism_fresh.IMAGE_RUN_FACTOR * ism_fresh.RUN_S,
                   ism_fresh.IMAGE_T60_FACTOR * max(r['design_t60_s'].values()))
               for r in D['rooms']]
    assert any(w > ism_fresh.IMAGE_RUN_FACTOR * ism_fresh.RUN_S for w in want), \
        'need at least one room whose T60 term dominates the floor, or this test has no power'
    assert any(abs(w - nt) > 1e-6 for w, nt in zip(want, no_tarr)), \
        'need the t_arr term to actually move image_time_s, or a dropped t_arr would not be caught'
    for r, w in zip(D['rooms'], want):
        assert abs(r['image_time_s'] - w) <= 1e-9, (r['id'], r['image_time_s'], w)
