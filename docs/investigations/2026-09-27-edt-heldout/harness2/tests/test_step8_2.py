"""New tests for HARNESS-PLAN.md section 8.2's four technical calls (2026-10-01), which settle
section 8.1's open points before A1. This file is new: T1-T22 (tests/test_*.py, frozen) are not
touched. Each test below is self-contained and does not depend on the others.

Call 1 (no code change): the synth edge at DRR 9.54 dB. T01 proves NaN truths are excluded as
truth_nan (P27) and finite rows just below the edge stay in, both at the synth_fresh level and
through score.py's exclusion machinery.

Call 2: Synth-fresh excludes sources inside the receiver ball; draws redraw while d < R + 0.2 m
(P24, as first stated, not P33's since-retracted -0.1384 m widening), with the reason counted under
'd_near_source' when the caller asks for it. The weak-spot row that used to widen D_M and
D_MINUS_R_MIN_M (scan1's d 0.17 m case) no longer binds them (corpus.histogram_holds_truth): it
stays in weak_spots.json (575 rows, T22a) with its d_m and d_minus_R_m blanked to null.

Call 3: a P15 split-borderline row (first reflection within 2R/c of the direct sound, and its error
within the split's measured bound of the 5 % line) is reported apart as 'truth_split_borderline' and
leaves H1-H3 and H5, staying in the ok/usable shares and H4 (P27's pattern, extended).

Call 4: an ISM-fresh row P18 flags truncated is kept (truth_status 'ok') if lengthening its image set
1.5x moves the truth by under 1e-3 relative, else stays truth_truncated. Dev-scale rooms only (no
large-ball case): both T04 cases below run in seconds and tear down nothing, since make_row writes no
files.
"""
import json
import math

import numpy as np
from conftest import HARNESS

from m8b import corpus, score, synth_fresh, truth


# ---- call 1: the synth edge at DRR 9.54 dB (no code change; P27 already excludes a NaN truth) ------
def _edge_spec(drr_db):
    return synth_fresh.make_spec(id='t01|%g' % drr_db, ratio=1.5, step_ms=1.0, t60_s=1.0, late_share_db=-20.0,
                                 drr_db=drr_db, R_m=0.31, d_m=5.0, gap_ms=0.0, delay_ms=0.0, run_over_t60=1.0)


def test_call1_synth_edge_drr_9_54_db_truth_nan():
    # PREREG.md's DRR range tops out at +10 dB (:39); the edge HARNESS-PLAN.md 8.1 names is the pure
    # single-slope value 10 log10(9) = 9.542425... dB, where the direct sound alone would take the
    # level past -10 dB. This row's S_rev also carries the (small, -20 dB) late part 8.2's call allows
    # to stay in the draw, which shifts the row's own crossing a few hundredths of a dB below that
    # value (checked here, not asserted to the theoretical figure): what matters for call 1 is the
    # qualitative behaviour below vs. above, which is what score.py's P27 exclusion acts on.
    edge = 10.0 * math.log10(9.0)
    assert abs(edge - 9.542425094393249) < 1e-9             # the theoretical figure HARNESS-PLAN.md 8.1 names

    below = synth_fresh.truth(_edge_spec(edge - 1.0))        # 8.54 dB: clearly below
    just_below = synth_fresh.truth(_edge_spec(edge - 0.05))  # just below this row's own crossing
    above = synth_fresh.truth(_edge_spec(edge + 1.0))        # 10.54 dB: clearly above (PREREG's own top)
    just_above = synth_fresh.truth(_edge_spec(edge + 0.01))  # just above this row's own crossing

    assert math.isfinite(below) and below > 0
    assert math.isfinite(just_below) and just_below > 0
    assert math.isnan(above)
    assert math.isnan(just_above)

    # P27, through score.py: a NaN truth is excluded as truth_nan and leaves every truth-based count;
    # a finite truth just below the edge is scored, however large, and is not itself wrong-silent here
    # (edt == truth by construction, so the method need not even run to prove the row is counted).
    def row(id_, truth_val):
        # the method always reports a finite edt when it is 'ok', whether or not the row's truth is
        # known (the method does not see the truth); here edt equals the truth when there is one, so
        # a finite-truth row is never itself wrong-silent, keeping the test's own point narrow.
        ts = 'ok' if (truth_val is not None and math.isfinite(truth_val)) else 'truth_nan'
        edt = truth_val if ts == 'ok' else 1.0
        return dict(set='synth', id=id_, status='ok', edt=edt, edt_lo=edt, edt_hi=edt, reason='',
                   truth=(truth_val if ts == 'ok' else None), truth_status=ts, room=None, d_m=5.0,
                   step_ms=1.0, band_hz=None, particles=None, seed=None, family=1.5, design_t60_s=1.0)

    rows = [row('below', below), row('just_below', just_below), row('above', above), row('just_above', just_above)]
    t = score.tally(rows)
    assert t['n'] == 4 and t['n_ok'] == 4 and t['n_usable'] == 4         # P27: stay in ok/usable shares
    assert t['excluded'] == {'truth_nan': 2}
    assert t['n_ok_truth'] == 2 and t['n_wrong_silent'] == 0             # the two NaN rows left every truth count
    h = score.h1({'ism': [], 'synth': rows})
    assert h['per_set']['synth']['n_ok_truth'] == 2 and h['per_set']['synth']['excluded'] == {'truth_nan': 2}


# ---- call 2: Synth-fresh excludes sources inside the receiver ball -------------------------------
def test_call2_synth_fresh_redraws_d_near_source():
    assert synth_fresh.D_MINUS_R_MIN_M == 0.2          # P24 as first stated, not P33's -0.1384 m
    assert synth_fresh.D_M[0] == 0.4                   # the one weak row that widened it no longer binds
    assert synth_fresh.BOUNDS['d_minus_R_m'][0] == 0.2

    rejections = {}
    rows = synth_fresh.draw(20261001, rejections=rejections)
    assert len(rows) == 4000                           # the bare-list contract (T15) is unchanged
    assert rejections.get('d_near_source', 0) > 0, 'no redraw was ever needed: the test draw is too tame'
    for r in rows:
        assert r['d_m'] - r['R_m'] >= 0.2 - 1e-9, r['id']      # no row has its source inside R + 0.2 m
        assert r['d_m'] >= r['R_m'], r['id']                    # in particular, never inside the ball itself

    # the default call (no rejections kwarg) is unaffected: T15 calls it this way
    same = synth_fresh.draw(20261001)
    assert len(same) == 4000
    assert [r['id'] for r in same] == [r['id'] for r in rows]


def test_call2_weak_spot_exception_in_weak_spots_json():
    """corpus.histogram_holds_truth and its use in corpus.scan1: the one weak-spot row with d_m < R_m
    (scan1's d 0.17 m case) stays in weak_spots.json, 575 rows total (T22a), with d_m and
    d_minus_R_m null so it no longer binds those bounds (T22b), while every other field (truth, cls,
    status, other quantities) is untouched."""
    assert corpus.histogram_holds_truth({'d_m': 0.5, 'R_m': 0.31}) is True       # d >= R: holds
    assert corpus.histogram_holds_truth({'d_m': 0.31, 'R_m': 0.31}) is True      # d == R: holds (not <)
    assert corpus.histogram_holds_truth({'d_m': 0.17, 'R_m': 0.31}) is False     # d < R: does not
    assert corpus.histogram_holds_truth({'d_m': 0.17, 'R_m': None}) is True      # no R: nothing to judge
    assert corpus.histogram_holds_truth({'d_m': 0.17, 'R_m': 0.0}) is True       # R = 0: a point receiver

    w = json.loads((HARNESS / 'weak_spots.json').read_text(encoding='utf-8'))
    assert len(w['rows']) == 575
    hit = [r for r in w['rows'] if r['id'].startswith('scan1|V200|T3|d0.7|gap')]
    assert len(hit) == 2, 'expected exactly the two scan1 d 0.17 m rows (gap5 and gap10)'
    for r in hit:
        assert r['q']['d_m'] is None and r['q']['d_minus_R_m'] is None
        assert r['q']['R_m'] == 0.31                      # every other quantity is untouched
        assert r['cls'] in ('wrong_silent', 'near_miss')  # still recorded as the generator's weak spot
        assert 'generator_defect' in r

    # the current BOUNDS, narrower on this axis than the old (pre-8.2) widening, is still satisfied:
    # T22b (frozen) already proves this on the committed file; this re-checks it directly, against
    # the function under test rather than against the committed JSON's numbers.
    for r in w['rows']:
        if r['q'].get('d_m') is not None and r['q'].get('R_m'):
            assert corpus.histogram_holds_truth(r['q']) is True, r['id']


# ---- call 3: P15's split-borderline rows -----------------------------------------------------------
def test_call3_split_borderline_detection():
    R, C = 0.31, 343.2
    h = R / C                     # 2R/c = 2h
    # inside the 2R/c gap, error exactly on the 5 % line: always borderline, any band
    truth_val = 1.0
    edt_on_line = truth_val * 1.05
    assert truth.split_borderline(edt_on_line, truth_val, gap_s=0.5 * h, h=h, band_hz=500) is True
    assert truth.split_borderline(edt_on_line, truth_val, gap_s=0.5 * h, h=h, band_hz=16000) is True
    # just outside the low-band bound (1.2 %): not borderline at 500 Hz, but still within the
    # wider high-band bound (2.1 %) at 16 kHz
    edt_far = truth_val * (1.05 + 0.013)
    assert truth.split_borderline(edt_far, truth_val, gap_s=0.5 * h, h=h, band_hz=500) is False
    assert truth.split_borderline(edt_far, truth_val, gap_s=0.5 * h, h=h, band_hz=16000) is True
    # outside the 2R/c gap: never borderline, however close the error is to 5 %
    assert truth.split_borderline(edt_on_line, truth_val, gap_s=3.0 * h, h=h, band_hz=500) is False
    # exactly at the gap boundary (gap_s == 2h) is not "within": strict less-than
    assert truth.split_borderline(edt_on_line, truth_val, gap_s=2.0 * h, h=h, band_hz=500) is False
    # error far from the 5 % line: not borderline even inside the gap
    assert truth.split_borderline(truth_val * 1.5, truth_val, gap_s=0.5 * h, h=h, band_hz=500) is False
    assert truth.split_borderline(truth_val * 1.0, truth_val, gap_s=0.5 * h, h=h, band_hz=500) is False
    # non-finite or non-positive inputs: never borderline (judged exactly, no exception, P28/P33)
    assert truth.split_borderline(float('nan'), truth_val, gap_s=0.5 * h, h=h, band_hz=500) is False
    assert truth.split_borderline(edt_on_line, truth_val, gap_s=0.5 * h, h=0.0, band_hz=500) is False
    assert truth.split_borderline(edt_on_line, 0.0, gap_s=0.5 * h, h=h, band_hz=500) is False

    assert truth.split_borderline_bound(4000) == 0.012 and truth.split_borderline_bound(4000.0) == 0.012
    assert truth.split_borderline_bound(8000) == 0.021

    # score.py: 'truth_split_borderline' behaves exactly as P27's other three exclusions (T19's pattern)
    assert 'truth_split_borderline' in score.TRUTH_STATUSES

    def row(id_, ts, truth_val=1.0, edt=1.0):
        return dict(set='spps', id=id_, status='ok', edt=edt, edt_lo=edt, edt_hi=edt, reason='',
                   truth=truth_val, truth_status=ts, room='F1', d_m=1.5, step_ms=1.0, band_hz=500,
                   particles=150000, seed=1, family=None, design_t60_s=1.0)

    rows = [row('a', 'ok'), row('b', 'ok'), row('c', 'truth_split_borderline'), row('d', 'truth_split_borderline')]
    t = score.tally(rows)
    assert t['n'] == 4 and t['n_ok'] == 4 and t['n_usable'] == 4           # stays in ok/usable (P27)
    assert t['excluded'] == {'truth_split_borderline': 2}
    assert t['n_ok_truth'] == 2                                            # leaves H1-H3, H5's truth-based counts
    h4 = score.h4({'spps': [dict(r, status='ok' if r['id'] in ('a', 'c') else 'wide') for r in rows]})
    assert h4['per_set']['spps']['n'] == 4 and h4['per_set']['spps']['n_usable'] == 4   # H4 does not exclude it


# ---- call 4: ISM-fresh truncated truths, kept when the retry converges ----------------------------
def test_call4_ism_fresh_truncated_retry():
    from m8b import ism_fresh

    # A dev-scale, slowly decaying box (large enough that n_images stays small: ~3e4-1e5 at these
    # image times, per m8b.ism_fresh.n_images; confirmed under a second to a few seconds below).
    # No corpus or held-out room is touched; nothing here is written to disk.
    room_marginal = dict(dims_m=[20.0, 15.0, 10.0], alpha_walls=[0.2] * 6, source_m=[2.0, 2.0, 2.0])
    room_short = dict(dims_m=[20.0, 15.0, 10.0], alpha_walls=[0.03] * 6, source_m=[2.0, 2.0, 2.0])
    rec = (10.0, 8.0, 5.0)

    # baseline (retry_truncated defaults False): both are flagged truncated, no retry attempted
    base = ism_fresh.make_row(room_marginal, rec, 0.31, 125.0, 1.0, image_time_s=1.5)
    assert base['truth_status'] == 'truth_truncated' and 'truth_truncated_retry' not in base

    # converges at 1.5x: kept as 'ok', with a receipt under 1e-3
    kept = ism_fresh.make_row(room_marginal, rec, 0.31, 125.0, 1.0, image_time_s=1.5, retry_truncated=True)
    assert kept['truth_status'] == 'ok'
    retry = kept['truth_truncated_retry']
    assert abs(retry['image_time_s'] - 1.5 * 1.5) <= 1e-9
    assert retry['moved_rel'] is not None and retry['moved_rel'] < ism_fresh.IMAGE_RETRY_REL_TOL
    assert kept['truth_edt'] == retry['truth_edt']                 # the kept row reports the retried truth

    # does not converge at 1.5x (short image set against a very reverberant room): stays truncated
    stays = ism_fresh.make_row(room_short, rec, 0.31, 125.0, 1.0, image_time_s=0.5, retry_truncated=True)
    assert stays['truth_status'] == 'truth_truncated'
    retry2 = stays['truth_truncated_retry']
    assert abs(retry2['image_time_s'] - 1.5 * 0.5) <= 1e-9
    assert retry2['moved_rel'] is not None and retry2['moved_rel'] >= ism_fresh.IMAGE_RETRY_REL_TOL
    assert stays['truth_edt'] != retry2['truth_edt']                # the original truth is kept, not the retry's

    # a row that was never truncated carries no retry key, retry_truncated or not
    ok_row = ism_fresh.make_row(room_marginal, rec, 0.31, 125.0, 1.0, image_time_s=3.0, retry_truncated=True)
    assert ok_row['truth_status'] == 'ok' and 'truth_truncated_retry' not in ok_row
