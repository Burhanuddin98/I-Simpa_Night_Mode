"""The scorer, round 2 (HARNESS-PLAN-2.md sections 4 and 9; PREREG-2.md; round 1's HARNESS-PLAN.md P27-P32): definitions,
exclusions, H1-H6, and the evaluation run that writes REPORT.md (plain words first), summary.json and rows.jsonl.

Round 2 differs from round 1 in exactly these places (harness2/ is round 1's scorer edited in place; round 1's file is
under ../harness/ and is never run again):
- the method is frozen2/method.py (v2.1, Z = 2.5) and there is no Z = 3 instance: evaluate runs two instances, frozen and
  upstream, and summary has no 'z3' key;
- every SPPS and ISM row carries R_m, the receiver radius in metres (the SPPS ball's 0.31 m, ISM's drawn R); _check_inputs
  refuses such a row without it;
- H4 (PREREG-2.md): a row is in H4 when its step is 1 ms, its design T60 is at most 3.0 s and R_m is at most 0.5 m; rows
  the method refused as receiver_too_large leave the denominator and are reported by count, share and set (n_before,
  n_receiver_too_large, share_receiver_too_large, with n the rows left); usable is at least 90 % of those left, SPPS-fresh
  and ISM-fresh each; the ok share is reported, not gated; nothing left fails (P27);
- H1, H2, H3 and H5 have no radius filter: a wrong-silent row counts at any R_m (T29);
- the refusal table: rtl_by_r_class(rows) counts receiver_too_large over all radii in three classes (<= 0.5, 0.5-1.0,
  > 1.0 m), per set and per SPPS mode, in summary['tables'][label]['rtl_by_r_class'] and in REPORT.md.
The scorer prints H4's n per set (REPORT.md, RESULTS.md).

No-swap rule (HARNESS-PLAN-2.md section 9 M2, written before any round-2 run): every G room is scored whatever its truth
runs show. A designed feature that misses on the truth runs (G2 truth T30 under 2.5 s at 1 kHz, G7 truth T30 over 0.25 s, G3
not double-sloped) is reported as missing in RESULTS.md and VERDICT-2.md. No room is swapped, redesigned or dropped after any
truth run exists, and nothing in this scorer can do so: it scores the rows of the 172 planned runs and no others.

Built in section 6, step 6 (T18-T21). The score semantics are critique/common.py:18-26's (a row is
ok, wide or refused; its error is edt / truth - 1), with PREREG's definitions judged exactly and
P28's tolerance on coverage. REPORT.md's table per set has FINAL.md section 3's columns (usable %,
ok %, wrong-silent, truth outside the range, median half-width, benefit/regress against upstream)
and EVAL.md's error columns, whose 95th percentile re-types final/run_eval_final.py:53. Both sources
are committed files; nothing comes from target/, so provenance.json has no entry for this file. On
the corpus's logged results (final/res_*.pkl and the old evaluator's upstream rows) of all four sets,
and through evaluate on the corpus inputs of t1, ISM and the real rows, it gives every number that
final/eval_final.txt shows for the frozen method and for upstream
(C:/tmp/m8b-edt/step6-score/check_score.json, 2026-10-01).

Contract:
- A scored row is a dict: 'set' ('spps', 'ism', 'synth' or 'attack'), 'id', the method's
  'status', 'edt', 'edt_lo', 'edt_hi', 'reason', the row's 'truth' (float or None) and
  'truth_status' ('ok', 'truth_truncated', 'truth_uncertain', 'truth_nan' or
  'truth_split_borderline'; P18, P19, HARNESS-PLAN.md 8.2's third call), and the fields the H's
  group by: 'room', 'd_m', 'step_ms', 'band_hz', 'particles', 'seed', 'family', 'design_t60_s'.
  'truth_split_borderline' is set by whatever builds a set's rows, from truth.split_borderline(...)
  once the method's edt is known (truth.py), and the scorer treats it exactly as the other three
  truth_status exclusions: left out of every truth-based count (P27), kept in the ok and usable
  shares and in H4.
- classify(row) -> dict(ok, usable, has_truth, wrong_silent, covered): ok is status 'ok'; usable is
  'ok' or 'wide'; has_truth is truth_status 'ok' with a finite truth. wrong_silent is None without
  a truth, False for a row that is not ok, else |edt/truth - 1| > JND judged exactly (21/20 is 5 %
  and not wrong, whatever float division rounds it to). covered is None without a truth or for a
  row that is not usable, else edt_lo * (1 - 1e-9) <= truth <= edt_hi * (1 + 1e-9) (P28).
- tally(rows) -> dict: 'n', 'n_ok', 'n_usable', 'ok_share', 'usable_share' (every row, P27),
  'excluded' {truth_status: count} for the rows without a truth, 'refused' {reason: count},
  'n_ok_truth', 'n_wrong_silent', 'wrong_silent_share', 'n_usable_truth', 'n_covered', 'coverage'.
  A share whose denominator is 0 is None.
- rtl_by_r_class(rows) -> {'<=0.5', '0.5-1.0', '>1.0': dict(n_rows, n_receiver_too_large, share)}: R_m 0.5 is in the first
  class and 1.0 in the second; a row with no R_m is refused (ValueError).
- h1(rows_by_set), h2(rows), h3(rows), h4(rows_by_set), h5(frozen_by_set, upstream_by_set),
  h6(classes) -> dict with 'pass' (bool) and the counts behind it. A ratio whose denominator is 0
  fails its criterion (P27).
  - h1: each of 'ism' and 'synth': wrong-silent <= 0.5 % of ok rows with a truth. Returns
    'per_set' {set: dict(n_ok_truth, n_wrong_silent, share, pass)}.
  - h2: SPPS-fresh pooled (P29): coverage >= 90 % of usable rows with a truth, and wrong-silent
    <= 3 % of ok rows with a truth.
  - h3: subgroups room x distance class x step for 'spps' and 'ism', family x step for 'synth';
    the class from d_m on the straight line (near < 2, mid 2-10, far > 10 m), pooled over band,
    particles and seed. A subgroup with >= 20 ok rows with a truth fails above 10 % wrong-silent.
    Returns 'subgroups' {key: dict(n_ok_truth, n_wrong_silent, judged, pass)}, key (set, room,
    class, step_ms) or (set, family, step_ms).
  - h4: each of 'spps' and 'ism' separately (P30'): rows at step_ms 1 whose design_t60_s <= 3.0 and whose R_m <= 0.5 m,
    less those refused as receiver_too_large; usable >= 90 % of the rest; the ok share is reported. Rows without a truth
    stay in (P27). Returns 'per_set' {set: dict(n_before, n_receiver_too_large, share_receiver_too_large, n, n_usable,
    n_ok, usable_share, ok_share, refused, pass)}.
  - h5: each of 'spps', 'ism', 'synth' and 'attack' (P31): on the rows with a truth that both
    methods have (paired by id), the method's wrong-silent count strictly below upstream's; 0
    against 0 fails, and so does a set with no rows.
  - h6: classes [{'id', 'n_draws', 'n_wrong_silent', 'votes'}]: reproducible when 5 or more draws
    are wrong-silent; implausible only when attack.panel(votes) says so; fails when a
    reproducible class is plausible.
- evaluate(inputs, *, method_path=None, out_dir) -> summary dict: checks the method file's sha256
  first (method.load; VoidRun before anything is computed or written, PREREG-2.md), runs it and
  upstream on every input ({'set', 'id', 'bins', 'dt', 't_arrival', 'meta',
  'truth', 'truth_status', and the grouping fields}), and writes REPORT.md, summary.json and
  rows.jsonl into out_dir. summary['method'] = {'path', 'sha256', 'verified': True}.

Added to the contract here; no test reads it, and a later step that feeds evaluate must meet it:
- An SPPS-fresh row or input also carries 'mode', 'random' or 'energetic' (driver.MODES). P11 scores
  Energetic as its own set, so h2-h5 refuse SPPS rows of two modes in one call (ValueError), and
  evaluate gives each mode its own H1-H6. Random's is the PREREG's verdict. Energetic's decides only
  whether EDT is shown for energetic runs (P11); H1 and H6, which hold no SPPS row, are shared.
- An Attack input is one draw (P26) and carries 'class_id'. evaluate(..., votes=None) takes the
  panel's raw votes as {class_id: [vote, vote, vote]}; a class without votes counts as plausible
  (P26: a missing answer counts as plausible).
- The h's return more counts than the contract names, and evaluate's summary also holds the port it
  checked ('upstream'), the truth function of each set (PREREG.md:34: the evaluator names it), the tables
  per set and instance, the criteria for the frozen instance and both modes, and the verdict.

Where the contract is silent:
- classify refuses (ValueError) a status other than ok, wide and refused, a truth_status other than
  the four, and a usable row whose edt, edt_lo or edt_hi is not a finite number. A row whose
  truth_status is 'ok' but whose truth is None or not finite has no truth and is counted as
  'truth_nan' (P19, P27); a finite truth that is not positive is refused, since no EDT is.
- "Judged exactly": |edt - truth| > truth / 20 in the rationals, on the floats' exact values
  (fractions.Fraction), the 5 % being the decimal 0.05 = 1/20 that PREREG.md:48 states. Every bar
  of H1-H4 is compared the same way, as a ratio of whole numbers: 0.5 % of 200 rows allows exactly 1.
- A set missing from rows_by_set is a set with no rows. A row filed under another set's key, or a
  row of another set given to h2, is refused.
- h3 passes when no subgroup is judged (T20). It refuses a row of a set H3 does not name
  (PREREG.md:57 names SPPS-fresh, ISM-fresh and Synth-fresh) and a row without the fields of its
  subgroup. The distance class takes d_m as given: near d < 2, mid 2 <= d <= 10, far d > 10 m.
  step_ms is read rounded to 1e-6 ms, here and in h4, so that a step carried through float32 (1 ms
  reads 1.0000000474974513 ms) falls in the plan's step and not in a subgroup of its own.
- h4 counts a row whose step_ms so read is 1.0, and refuses one at 1 ms without a finite positive
  design_t60_s, which its filter needs.
- h5 pairs by id within a set. It refuses an id that appears twice in one method's rows, and a pair
  whose two rows carry a different truth, truth_status or mode, since both are read from one input.
- h6 refuses a class whose n_draws is not attack.DRAWS (P26's 20), whose n_wrong_silent is not a
  whole number from 0 to n_draws, or whose id repeats. With no class at all it fails: P27's reason,
  that a set which answers nothing cannot pass, holds for the attack set, as H5 already applies it.
- evaluate checks, in this order and before anything is computed or written: the method file
  (method.load), the upstream port (upstream.port), then every input (its set,
  a (set, id) no other input has, 'bins', a positive 'dt', 't_arrival' present, a step_ms equal to
  dt in ms to 1e-6, its truth as classify reads it, and the fields its H's need: an SPPS mode, the
  H3 subgroup, H4's design T60, and for the attack set exactly 20 draws per class and no vote for a
  class with no draw). Each instance then runs once on every input, with the input's bins, dt,
  t_arrival and a copy of its meta, in the order frozen, upstream. An exception from a method
  stops the evaluation: no row is turned into a refusal. out_dir is made only when everything has
  been computed, and the three files are written into it with LF line ends.
- HARNESS-PLAN.md 8.2's first call settles 8.1's open point on the Synth-fresh rows just under DRR
  9.54 dB: no change here. The scorer applies P27 as written: a NaN truth is excluded as truth_nan,
  and every finite truth below the edge is scored, however large; the rows test whether the method
  refuses or widens an ill-conditioned case, and a refusal is never wrong (PREREG.md:62).
"""
import datetime
import json
import math
import numbers
import platform
import statistics
from collections import Counter
from fractions import Fraction
from pathlib import Path

import numpy as np

from . import attack, method, upstream
from .corpus import clean
from .round2 import H4_R_MAX_M, H4_REFUSAL as H4_RTL

JND = 0.05
COVER_TOL = 1e-9

SETS = ('spps', 'ism', 'synth', 'attack')
MODES = ('random', 'energetic')                     # P11; driver.MODES
STATUSES = ('ok', 'wide', 'refused')                # frozen/method.py:24, 90; upstream.analyse
TRUTH_STATUSES = ('ok', 'truth_truncated', 'truth_uncertain', 'truth_nan',
                  'truth_split_borderline')                                  # P18, P19; HARNESS-PLAN.md 8.2, 3rd call
H_IDS = ('H1', 'H2', 'H3', 'H4', 'H5', 'H6')
H1_SETS = ('ism', 'synth')                          # PREREG.md:55
H3_SETS = ('spps', 'ism', 'synth')                  # PREREG.md:57
H4_SETS = ('spps', 'ism')                           # PREREG.md:58, P30
H5_SETS = ('spps', 'ism', 'synth', 'attack')        # P31
H1_WRONG_MAX = Fraction(5, 1000)                    # PREREG.md:55
H2_COVERAGE_MIN = Fraction(90, 100)                 # PREREG.md:56
H2_WRONG_MAX = Fraction(3, 100)
H3_MIN_OK = 20                                      # PREREG.md:57; P29: ok rows with a truth
H3_WRONG_MAX = Fraction(10, 100)
H4_STEP_MS = 1.0                                    # PREREG.md:58, P30
H4_T60_MAX_S = 3.0
R_CLASSES = ('<=0.5', '0.5-1.0', '>1.0')             # the refusal table's radius classes, metres
R_TOL = 1e-9
H4_USABLE_MIN = Fraction(90, 100)
NEAR_M, FAR_M = 2.0, 10.0                           # PREREG.md:57, P29
_JND = Fraction(repr(JND))                          # 1/20, the decimal PREREG.md:48 states

# A scored row's fields that come from its input: the contract's, P11's mode and P26's class.
ROW_FIELDS = ('set', 'id', 'mode', 'class_id', 'truth', 'truth_status', 'room', 'd_m', 'step_ms', 'band_hz',
              'particles', 'seed', 'family', 'design_t60_s', 'R_m')
INSTANCES = ('frozen', 'upstream')
LABELS = ('spps-random', 'spps-energetic', 'ism', 'synth', 'attack')
NAMES = {'spps': 'SPPS-fresh', 'spps-random': 'SPPS-fresh, Random', 'spps-energetic': 'SPPS-fresh, Energetic',
         'ism': 'ISM-fresh', 'synth': 'Synth-fresh', 'attack': 'Attack'}
H_TEXT = {                                          # PREREG-2.md, the pass criteria
    'H1': 'Noise-free sets (ISM-fresh-2, Synth-fresh-2): wrong-silent ≤ 0.5 % of ok rows, on each set, every receiver radius',
    'H2': 'SPPS-fresh-2: coverage ≥ 90 % of usable rows, and wrong-silent ≤ 3 % of ok rows',
    'H3': 'No subgroup with ≥ 20 ok rows has wrong-silent > 10 %, every receiver radius',
    'H4': 'At a 1 ms step, design T60 ≤ 3 s and receiver radius ≤ 0.5 m, usable ≥ 90 % of the rows not refused as receiver_too_large '
          '(SPPS-fresh and ISM-fresh)',
    'H5': "Wrong-silent count strictly lower than upstream's on every fresh set",
    'H6': 'Attack: no reproducible class of physically plausible SPPS inputs (≥ 5 instances) gives wrong-silent',
}
TRUTH_FUNCTIONS = {                                 # PREREG.md:34: the evaluator names it (P15)
    'spps': "P15's Definition A, the corpus's truth_ideal (target/agents/followup-design/skeptic-gf3/"
            "attack_ism.py:55-77, with rerun/mirror.py:20, 109-146), run from its text copies "
            "m8b/_truth_ideal.py and m8b/_mirror.py by truth.assess on the sum of the K = 4 references "
            "(P16-P19): bins overlapping [t_arr - R/c, t_arr + R/c) are direct, a blocked receiver's "
            "direct is 0",
    'ism': "P15's Definition A on the image-source split: truth_ideal at 0.02 ms with continuous air "
           "(P20, ism_fresh.make_row)",
    'synth': "P15's Definition A in closed form: critique/synth.py truth_edt",
    'attack': "computed by the harness for each draw from its generator, as for Synth-fresh and ISM-fresh (P26)",
}


# ---- definitions (PREREG.md:44-49, P28) -----------------------------------------------------------------
def _number(x):
    return None if x is None else float(x)


def _truth(row):
    """(truth, None) for a row with a truth, (None, the reason it has none) otherwise (P19, P27)."""
    ts = row.get('truth_status')
    if ts not in TRUTH_STATUSES:
        raise ValueError('row %r: truth_status %r is not one of %s' % (row.get('id'), ts, TRUTH_STATUSES))
    if ts != 'ok':
        return None, ts
    t = _number(row.get('truth'))
    if t is None or not math.isfinite(t):
        return None, 'truth_nan'
    if not t > 0:
        raise ValueError('row %r: a truth of %r s is no EDT' % (row.get('id'), t))
    return t, None


def _shown(row):
    """A usable row's edt, edt_lo and edt_hi, each a finite float."""
    out = []
    for k in ('edt', 'edt_lo', 'edt_hi'):
        v = _number(row.get(k))
        if v is None or not math.isfinite(v):
            raise ValueError('row %r: status %r with %s = %r' % (row.get('id'), row.get('status'), k, row.get(k)))
        out.append(v)
    return out


def wrong(edt, truth):
    """|edt / truth - 1| > JND for truth > 0, judged on the floats' exact values: |edt - truth| > truth / 20."""
    e, t = Fraction(float(edt)), Fraction(float(truth))
    return abs(e - t) > _JND * t


def covers(lo, hi, truth):
    """P28: edt_lo * (1 - 1e-9) <= truth <= edt_hi * (1 + 1e-9), the evaluator's tolerance."""
    return lo * (1 - COVER_TOL) <= truth <= hi * (1 + COVER_TOL)


def classify(row):
    status = row.get('status')
    if status not in STATUSES:
        raise ValueError('row %r: status %r is not one of %s' % (row.get('id'), status, STATUSES))
    ok = status == 'ok'
    usable = ok or status == 'wide'
    truth, _ = _truth(row)
    if usable:
        edt, lo, hi = _shown(row)
    if truth is None:
        return dict(ok=ok, usable=usable, has_truth=False, wrong_silent=None, covered=None)
    return dict(ok=ok, usable=usable, has_truth=True, wrong_silent=wrong(edt, truth) if ok else False,
                covered=covers(lo, hi, truth) if usable else None)


def _share(a, b):
    return None if b == 0 else a / b


def _reason(row):
    return str(row.get('reason') or '(none)')


def _p95(xs):
    """The old evaluator's 95th percentile (final/run_eval_final.py:53): the sorted value at index
    round(0.95 (n - 1))."""
    return sorted(xs)[max(0, int(round(0.95 * (len(xs) - 1))))]


def tally(rows):
    """The contract's counts, and for the report: 'n_wide', 'n_refused', 'n_out_of_range' (usable rows
    with a truth outside the shown range, P28) and, as EVAL.md defines them and
    final/run_eval_final.py:35-54 computes them, the median and 95th percentile of |edt / truth - 1|
    over usable rows with a truth and the median relative half-width (edt_hi - edt_lo) / (2 edt) over
    usable rows."""
    n = n_ok = n_usable = n_ok_truth = n_wrong = n_usable_truth = n_covered = 0
    excluded, refused = Counter(), Counter()
    errs, hws = [], []
    for r in rows:
        c = classify(r)
        n += 1
        n_ok += c['ok']
        n_usable += c['usable']
        if r['status'] == 'refused':
            refused[_reason(r)] += 1
        if c['usable']:
            edt, lo, hi = _shown(r)
            if edt > 0:
                hws.append((hi - lo) / (2.0 * edt))
        if not c['has_truth']:
            excluded[_truth(r)[1]] += 1
            continue
        if c['ok']:
            n_ok_truth += 1
            n_wrong += c['wrong_silent']
        if c['usable']:
            n_usable_truth += 1
            n_covered += c['covered']
            errs.append(abs(edt / _truth(r)[0] - 1.0))
    return {'n': n, 'n_ok': n_ok, 'n_usable': n_usable, 'ok_share': _share(n_ok, n),
            'usable_share': _share(n_usable, n), 'excluded': dict(excluded), 'refused': dict(refused),
            'n_ok_truth': n_ok_truth, 'n_wrong_silent': n_wrong, 'wrong_silent_share': _share(n_wrong, n_ok_truth),
            'n_usable_truth': n_usable_truth, 'n_covered': n_covered, 'coverage': _share(n_covered, n_usable_truth),
            'n_wide': n_usable - n_ok, 'n_refused': n - n_usable, 'n_out_of_range': n_usable_truth - n_covered,
            'med_abs_err': statistics.median(errs) if errs else None,
            'p95_abs_err': _p95(errs) if errs else None,
            'med_half_width': statistics.median(hws) if hws else None}


# ---- H1-H6 (PREREG.md:51-60, P27, P29-P31) ---------------------------------------------------------------
def _rows_of(rows_by_set, s):
    rows = list((rows_by_set or {}).get(s) or ())
    other = [r.get('id') for r in rows if r.get('set') != s]
    if other:
        raise ValueError('%d rows filed under %r belong to another set, the first %r' % (len(other), s, other[0]))
    return rows


def _one_mode(rows):
    modes = {r.get('mode') for r in rows if r.get('set') == 'spps'}
    if len(modes) > 1:
        raise ValueError('SPPS-fresh rows of the modes %s in one criterion: P11 scores each mode as its own set'
                         % sorted(map(str, modes)))


def _at_most(a, b, bar):
    return b > 0 and Fraction(a, b) <= bar


def _at_least(a, b, bar):
    return b > 0 and Fraction(a, b) >= bar


def h1(rows_by_set):
    per = {}
    for s in H1_SETS:
        t = tally(_rows_of(rows_by_set, s))
        per[s] = {'n_ok_truth': t['n_ok_truth'], 'n_wrong_silent': t['n_wrong_silent'],
                  'share': t['wrong_silent_share'], 'excluded': t['excluded'],
                  'pass': _at_most(t['n_wrong_silent'], t['n_ok_truth'], H1_WRONG_MAX)}
    return {'pass': all(p['pass'] for p in per.values()), 'per_set': per}


def h2(rows):
    rows = list(rows)
    other = [r.get('id') for r in rows if r.get('set') != 'spps']
    if other:
        raise ValueError('H2 pools SPPS-fresh only; %d rows of another set, the first %r' % (len(other), other[0]))
    _one_mode(rows)
    t = tally(rows)
    cov = _at_least(t['n_covered'], t['n_usable_truth'], H2_COVERAGE_MIN)
    ws = _at_most(t['n_wrong_silent'], t['n_ok_truth'], H2_WRONG_MAX)
    return {'pass': cov and ws, 'coverage_pass': cov, 'wrong_silent_pass': ws,
            'n_usable_truth': t['n_usable_truth'], 'n_covered': t['n_covered'], 'coverage': t['coverage'],
            'n_ok_truth': t['n_ok_truth'], 'n_wrong_silent': t['n_wrong_silent'],
            'wrong_silent_share': t['wrong_silent_share'], 'excluded': t['excluded']}


def distance_class(d_m):
    d = _number(d_m)
    if d is None or not math.isfinite(d) or d < 0:
        raise ValueError('a source-receiver distance of %r m has no class' % (d_m,))
    return 'near' if d < NEAR_M else 'far' if d > FAR_M else 'mid'


def _step(row):
    """step_ms rounded to 1e-6 ms, so that a step carried through float32 (1 ms reads
    1.0000000474974513 ms) groups with the plan's 1, 2 and 5 ms and meets H4's filter."""
    s = _number(row.get('step_ms'))
    if s is None or not (math.isfinite(s) and s > 0):
        raise ValueError('row %r: step_ms %r' % (row.get('id'), row.get('step_ms')))
    return round(s, 6)


def subgroup(row):
    """H3's key: (set, room, distance class, step_ms) for SPPS-fresh and ISM-fresh, (set, family, step_ms)
    for Synth-fresh."""
    s = row.get('set')
    if s in ('spps', 'ism'):
        if row.get('room') is None:
            raise ValueError('row %r: no room, which its H3 subgroup needs' % (row.get('id'),))
        return s, row['room'], distance_class(row.get('d_m')), _step(row)
    if s == 'synth':
        fam = _number(row.get('family'))
        if fam is None or not math.isfinite(fam):
            raise ValueError('row %r: family %r, which its H3 subgroup needs' % (row.get('id'), row.get('family')))
        return s, fam, _step(row)
    raise ValueError('row %r: H3 has no subgroup for set %r (PREREG.md:57)' % (row.get('id'), s))


def h3(rows):
    rows = list(rows)
    _one_mode(rows)
    groups = {}
    for r in rows:
        key = subgroup(r)
        c = classify(r)
        g = groups.setdefault(key, {'n_rows': 0, 'n_ok': 0, 'n_ok_truth': 0, 'n_wrong_silent': 0})
        g['n_rows'] += 1
        g['n_ok'] += c['ok']
        if c['ok'] and c['has_truth']:
            g['n_ok_truth'] += 1
            g['n_wrong_silent'] += c['wrong_silent']
    for g in groups.values():
        g['judged'] = g['n_ok_truth'] >= H3_MIN_OK
        g['share'] = _share(g['n_wrong_silent'], g['n_ok_truth'])
        g['pass'] = not g['judged'] or _at_most(g['n_wrong_silent'], g['n_ok_truth'], H3_WRONG_MAX)
    return {'pass': all(g['pass'] for g in groups.values()), 'subgroups': groups, 'n_subgroups': len(groups),
            'n_judged': sum(g['judged'] for g in groups.values()),
            'n_failing': sum(not g['pass'] for g in groups.values())}


def _r_m(row):
    r = _number(row.get('R_m'))
    if r is None or not (math.isfinite(r) and r > 0):
        raise ValueError('row %r: R_m %r, which H4 and the refusal table need' % (row.get('id'), row.get('R_m')))
    return r


def _in_h4(row):
    """P30': at 1 ms, with the band's design T60 at most 3.0 s and the receiver radius at most 0.5 m (1e-9 allowed)."""
    if _step(row) != H4_STEP_MS:
        return False
    t60 = _number(row.get('design_t60_s'))
    if t60 is None or not (math.isfinite(t60) and t60 > 0):
        raise ValueError('row %r at 1 ms: design_t60_s %r, which H4 filters on (P30)'
                         % (row.get('id'), row.get('design_t60_s')))
    return t60 <= H4_T60_MAX_S and _r_m(row) <= H4_R_MAX_M + R_TOL


def _is_rtl(row):
    return row.get('status') == 'refused' and row.get('reason') == H4_RTL


def h4(rows_by_set):
    per = {}
    for s in H4_SETS:
        rows = _rows_of(rows_by_set, s)
        _one_mode(rows)
        before = [r for r in rows if _in_h4(r)]
        sel = [r for r in before if not _is_rtl(r)]            # a refusal as receiver_too_large leaves the denominator
        cs = [classify(r) for r in sel]
        n, n_usable, n_ok = len(sel), sum(c['usable'] for c in cs), sum(c['ok'] for c in cs)
        per[s] = {'n_before': len(before), 'n_receiver_too_large': len(before) - n,
                  'share_receiver_too_large': _share(len(before) - n, len(before)),
                  'n': n, 'n_usable': n_usable, 'n_ok': n_ok, 'usable_share': _share(n_usable, n),
                  'ok_share': _share(n_ok, n),
                  'refused': dict(Counter(_reason(r) for r in sel if r['status'] == 'refused')),
                  'pass': _at_least(n_usable, n, H4_USABLE_MIN)}
    return {'pass': all(p['pass'] for p in per.values()), 'per_set': per}


def r_class(r_m):
    """The refusal table's radius class: R_m 0.5 is in the first class and 1.0 in the second (1e-9 allowed)."""
    return R_CLASSES[0] if r_m <= H4_R_MAX_M + R_TOL else R_CLASSES[1] if r_m <= 1.0 + R_TOL else R_CLASSES[2]


def rtl_by_r_class(rows):
    """receiver_too_large over all radii: per radius class, the rows, the ones refused as receiver_too_large and their share."""
    out = {c: {'n_rows': 0, 'n_receiver_too_large': 0} for c in R_CLASSES}
    for r in rows:
        t = out[r_class(_r_m(r))]
        t['n_rows'] += 1
        t['n_receiver_too_large'] += _is_rtl(r)
    for t in out.values():
        t['share'] = _share(t['n_receiver_too_large'], t['n_rows'])
    return out


def _by_id(rows, s, who):
    out = {}
    for r in rows:
        if r.get('id') in out:
            raise ValueError('%s %s: id %r appears twice' % (who, s, r.get('id')))
        out[r.get('id')] = r
    return out


def _input_key(row):
    """What a scored row takes from its input and h5's pair must share: the truth (NaN as one value),
    its status and the SPPS mode."""
    t = _number(row.get('truth'))
    return 'nan' if t is not None and math.isnan(t) else t, row.get('truth_status'), row.get('mode')


def h5(frozen_by_set, upstream_by_set):
    per = {}
    for s in H5_SETS:
        fr, up = _rows_of(frozen_by_set, s), _rows_of(upstream_by_set, s)
        _one_mode(fr)
        _one_mode(up)
        f, u = _by_id(fr, s, 'the method'), _by_id(up, s, 'upstream')
        paired = [i for i in f if i in u]
        n = wf = wu = 0
        for i in paired:
            if _input_key(f[i]) != _input_key(u[i]):
                raise ValueError('%s %r: the two rows carry a different truth or mode' % (s, i))
            cf, cu = classify(f[i]), classify(u[i])
            if not cf['has_truth']:
                continue
            n += 1
            wf += cf['wrong_silent']
            wu += cu['wrong_silent']
        per[s] = {'n_paired': len(paired), 'n_eligible': n, 'n_wrong_silent': wf, 'n_wrong_silent_upstream': wu,
                  'n_method_only': len(f) - len(paired), 'n_upstream_only': len(u) - len(paired),
                  'pass': n > 0 and wf < wu}
    return {'pass': all(p['pass'] for p in per.values()), 'per_set': per}


def _vote_reason(raw):
    """A vote's reasons for the report: its 'reasons' when it parses, else the raw text as given."""
    if attack.parse_vote(raw) is not None:
        return json.loads(raw)['reasons']
    return None if raw is None else str(raw)


def h6(classes):
    out, ids = [], set()
    for c in classes:
        cid = c.get('id')
        if cid in ids:
            raise ValueError('class %r appears twice' % (cid,))
        ids.add(cid)
        nd, nw = c.get('n_draws'), c.get('n_wrong_silent')
        whole = [isinstance(x, numbers.Integral) and not isinstance(x, bool) for x in (nd, nw)]
        if not whole[0] or nd != attack.DRAWS:
            raise ValueError('class %r: %r draws, not the %d P26 fixes' % (cid, nd, attack.DRAWS))
        if not whole[1] or not 0 <= nw <= nd:
            raise ValueError('class %r: %r wrong-silent draws of %r' % (cid, nw, nd))
        nd, nw = int(nd), int(nw)
        votes = c.get('votes')
        votes = list(votes) if isinstance(votes, (list, tuple)) else ([] if votes is None else [votes])
        verdict = attack.panel(votes)
        reproducible = nw >= attack.REPRODUCIBLE
        parsed = [attack.parse_vote(v) for v in votes]
        agreement = {'implausible': parsed.count('implausible'), 'plausible': parsed.count('plausible'),
                     'unreadable': parsed.count(None), 'missing': max(0, attack.PANEL_SIZE - len(votes))}
        out.append({'id': cid, 'n_draws': nd, 'n_wrong_silent': nw, 'reproducible': reproducible,
                    'votes': parsed, 'reasons': [_vote_reason(v) for v in votes], 'agreement': agreement,
                    'panel': verdict, 'pass': not (reproducible and verdict == 'plausible')})
    return {'pass': bool(out) and all(x['pass'] for x in out), 'classes': out, 'n_classes': len(out),
            'n_reproducible': sum(x['reproducible'] for x in out), 'n_failing': sum(not x['pass'] for x in out)}


# ---- the evaluation ----------------------------------------------------------------------------------
def _check_inputs(inputs, votes):
    """Every input as evaluate's docstring lists, before any method runs. Returns {class_id: draws}."""
    seen, draws = set(), Counter()
    for x in inputs:
        missing = [k for k in ('set', 'id', 'bins', 'dt', 't_arrival', 'truth_status') if k not in x]
        if missing:
            raise ValueError('input %r has no %s' % (x.get('id'), missing))
        s = x['set']
        if s not in SETS:
            raise ValueError('input %r: set %r is not one of %s' % (x['id'], s, SETS))
        if (s, x['id']) in seen:
            raise ValueError('two inputs of %s have the id %r' % (s, x['id']))
        seen.add((s, x['id']))
        _truth(x)
        dt = float(x['dt'])
        if not (math.isfinite(dt) and dt > 0):
            raise ValueError('input %r: dt %r' % (x['id'], x['dt']))
        if not math.isclose(dt * 1e3, _step(x), rel_tol=1e-6):
            raise ValueError('input %r: step_ms %r is not dt = %r s' % (x['id'], x['step_ms'], dt))
        if s == 'spps' and x.get('mode') not in MODES:
            raise ValueError('input %r: SPPS-fresh mode %r is not one of %s (P11)' % (x['id'], x.get('mode'), MODES))
        if s in H3_SETS:
            subgroup(x)
        if s in H4_SETS:
            _r_m(x)                       # round 2: every SPPS and ISM row carries its receiver radius
            _in_h4(x)
        if s == 'attack':
            if x.get('class_id') is None:
                raise ValueError('attack input %r has no class_id (P26)' % (x['id'],))
            draws[x['class_id']] += 1
    bad = {c: n for c, n in draws.items() if n != attack.DRAWS}
    if bad:
        raise ValueError('classes without %d draws each (P26): %s' % (attack.DRAWS, bad))
    unrun = [c for c in (votes or {}) if c not in draws]
    if unrun:
        raise ValueError('votes for classes with no draw: %s' % unrun)
    return draws


def _label(row):
    return 'spps-' + row['mode'] if row['set'] == 'spps' else row['set']


def _versus(rows, up_rows):
    """FINAL.md's benefit/regress against upstream, on the rows with a truth, paired by id: a row is
    good when it is ok and not wrong-silent (EVAL.md)."""
    up = {r['id']: r for r in up_rows}
    out = Counter()
    for r in rows:
        c = classify(r)
        if not c['has_truth']:
            continue
        cu = classify(up[r['id']])
        g, gu = c['ok'] and not c['wrong_silent'], cu['ok'] and not cu['wrong_silent']
        out['both_good' if g and gu else 'benefit' if g else 'regress' if gu else 'neither'] += 1
    return {k: out[k] for k in ('benefit', 'regress', 'both_good', 'neither')}


def criteria(by_label, up_by_label, classes, mode):
    """H1-H6 for one instance and one SPPS mode (P11): by_label and up_by_label map LABELS to rows."""
    spps = by_label['spps-' + mode]
    rows_by_set = dict(spps=spps, ism=by_label['ism'], synth=by_label['synth'], attack=by_label['attack'])
    up_by_set = dict(spps=up_by_label['spps-' + mode], ism=up_by_label['ism'], synth=up_by_label['synth'],
                     attack=up_by_label['attack'])
    out = {'H1': h1(rows_by_set), 'H2': h2(spps), 'H3': h3(spps + by_label['ism'] + by_label['synth']),
           'H4': h4(rows_by_set), 'H5': h5(rows_by_set, up_by_set), 'H6': h6(classes)}
    out['pass'] = all(out[h]['pass'] for h in H_IDS)
    out['failing'] = [h for h in H_IDS if not out[h]['pass']]
    return out


def _key(k):
    if isinstance(k, tuple):
        return '|'.join(_key(x) for x in k)
    if isinstance(k, float):
        return str(int(k)) if k.is_integer() else repr(k)
    return str(k)


def _jsonable(x):
    if isinstance(x, dict):
        return {_key(k): _jsonable(v) for k, v in x.items()}
    if isinstance(x, (list, tuple)):
        return [_jsonable(v) for v in x]
    return clean(x)


def evaluate(inputs, *, method_path=None, out_dir, votes=None):
    frozen = method.load(method_path)       # VoidRun before anything is computed or written (PREREG-2.md)
    port = upstream.port()                  # VoidRun unless the copy is the validated port (P32)
    inputs = list(inputs)
    draws = _check_inputs(inputs, votes)

    runs = (('frozen', frozen.analyse), ('upstream', upstream.analyse))
    scored = {name: [] for name, _ in runs}
    for x in inputs:
        base = {k: x.get(k) for k in ROW_FIELDS}
        meta = x.get('meta')
        for name, analyse in runs:
            res = analyse(x['bins'], float(x['dt']), x['t_arrival'], None if meta is None else dict(meta))
            scored[name].append(dict(base, status=res['status'], edt=res['edt'], edt_lo=res['edt_lo'],
                                     edt_hi=res['edt_hi'], reason=res['reason']))

    by_label = {name: {lab: [r for r in rows if _label(r) == lab] for lab in LABELS} for name, rows in scored.items()}
    tables = {}
    for lab in LABELS:
        t = {name: tally(by_label[name][lab]) for name in INSTANCES}
        t['versus_upstream'] = {'frozen': _versus(by_label['frozen'][lab], by_label['upstream'][lab])}
        t['rtl_by_r_class'] = rtl_by_r_class([r for r in by_label['frozen'][lab] if r.get('R_m') is not None])
        tables[lab] = t
    classes = []
    for cid in sorted(draws, key=str):
        rows = [r for r in by_label['frozen']['attack'] if r['class_id'] == cid]
        classes.append({'id': cid, 'n_draws': len(rows), 'votes': (votes or {}).get(cid),
                        'n_wrong_silent': sum(bool(classify(r)['wrong_silent']) for r in rows)})
    crit = {'frozen': {mode: criteria(by_label['frozen'], by_label['upstream'], classes, mode) for mode in MODES}}
    summary = {
        'schema': 'm8b.score/2',
        'method': {'path': str(frozen.checked_path), 'sha256': frozen.checked_sha256, 'verified': True,
                   'Z': float(frozen.Z)},
        'upstream': {'path': str(port.checked_path), 'sha256': port.checked_sha256, 'verified': True,
                     'source': upstream.UPSTREAM_SOURCE},
        'truth_functions': TRUTH_FUNCTIONS,
        'inputs': {'n': len(inputs), 'per_set': {lab: len(by_label['frozen'][lab]) for lab in LABELS},
                   'attack_classes': len(draws), 'voted_classes': len(votes or {})},
        'tables': tables,
        'criteria': crit,
        'verdict': {'decided_by': 'the frozen method (v2.1, Z = %g) in Random mode (PREREG-2.md, P11)' % float(frozen.Z),
                    'pass': crit['frozen']['random']['pass'],
                    'failing': crit['frozen']['random']['failing'],
                    'energetic_pass': crit['frozen']['energetic']['pass'],
                    'energetic_failing': crit['frozen']['energetic']['failing']},
        'generated': {'at': datetime.datetime.now().astimezone().isoformat(timespec='seconds'),
                      'python': platform.python_version(), 'numpy': np.__version__},
    }
    summary = _jsonable(summary)
    lines = ''.join(json.dumps(_jsonable(_row_record(x, i, scored)), allow_nan=False) + '\n'
                    for i, x in enumerate(inputs))
    text = json.dumps(summary, indent=1, allow_nan=False) + '\n'
    report = report_md(summary)

    out = Path(out_dir)
    out.mkdir(parents=True, exist_ok=True)
    (out / 'rows.jsonl').write_text(lines, encoding='utf-8', newline='\n')
    (out / 'summary.json').write_text(text, encoding='utf-8', newline='\n')
    (out / 'REPORT.md').write_text(report, encoding='utf-8', newline='\n')
    return summary


def _row_record(x, i, scored):
    rec = {k: x.get(k) for k in ROW_FIELDS}
    truth, why = _truth(x)
    rec['excluded_as'] = why
    for name in INSTANCES:
        r = scored[name][i]
        c = classify(r)
        rec[name] = dict(status=r['status'], edt=r['edt'], edt_lo=r['edt_lo'], edt_hi=r['edt_hi'], reason=r['reason'],
                         err=None if truth is None or r['edt'] is None else float(r['edt']) / truth - 1.0, **c)
    return rec


# ---- REPORT.md -----------------------------------------------------------------------------------------
def _n(x):
    return '{:,}'.format(x)


def _pct(a, b, digits=1):
    return '–' if not b else '%.*f %%' % (digits, 100.0 * a / b)


def _pass(ok):
    return 'passes' if ok else 'fails'


def _counts(d):
    return ', '.join('%s %s' % (k, _n(v)) for k, v in sorted(d.items())) or 'none'


def _why(h, r):
    """The counts behind one criterion's result, in a line."""
    if h == 'H1':
        return '; '.join('%s: %s of %s ok rows with a truth wrong-silent (%s)' % (
            NAMES[s], _n(p['n_wrong_silent']), _n(p['n_ok_truth']), _pct(p['n_wrong_silent'], p['n_ok_truth'], 2))
            for s, p in r['per_set'].items())
    if h == 'H2':
        return ('coverage %s of %s usable rows with a truth (%s); wrong-silent %s of %s ok rows with a truth (%s)' % (
            _n(r['n_covered']), _n(r['n_usable_truth']), _pct(r['n_covered'], r['n_usable_truth']),
            _n(r['n_wrong_silent']), _n(r['n_ok_truth']), _pct(r['n_wrong_silent'], r['n_ok_truth'], 2)))
    if h == 'H3':
        bad = [(k, g) for k, g in r['subgroups'].items() if not g['pass']]
        text = '%s subgroups, %s judged (20 or more ok rows with a truth), %s over 10 %%' % (
            _n(r['n_subgroups']), _n(r['n_judged']), _n(r['n_failing']))
        if bad:
            text += ': ' + '; '.join('%s %s of %s' % (k, _n(g['n_wrong_silent']), _n(g['n_ok_truth']))
                                     for k, g in bad[:5]) + ('; ...' if len(bad) > 5 else '')
        return text
    if h == 'H4':
        return '; '.join('%s: n = %s (%s in the filter, %s refused as receiver_too_large), %s of %s rows usable (%s), ok %s' % (
            NAMES[s], _n(p['n']), _n(p['n_before']), _n(p['n_receiver_too_large']), _n(p['n_usable']), _n(p['n']),
            _pct(p['n_usable'], p['n']), _pct(p['n_ok'], p['n']))
            for s, p in r['per_set'].items())
    if h == 'H5':
        return '; '.join("%s: %s against upstream's %s on %s rows with a truth" % (
            NAMES[s], _n(p['n_wrong_silent']), _n(p['n_wrong_silent_upstream']), _n(p['n_eligible']))
            for s, p in r['per_set'].items())
    return ('%s classes, %s reproducible (5 or more of 20 draws wrong-silent), %s of those not called '
            'implausible by 3 of 3 judges' % (_n(r['n_classes']), _n(r['n_reproducible']), _n(r['n_failing'])))


def _plain(s):
    cf = s['criteria']['frozen']
    per = s['inputs']['per_set']
    out = []
    r = cf['random']
    if r['pass']:
        out.append('In Random mode, the mode the PREREG-2\'s verdict is about, the round-2 EDT method (v2.1, Z = %g) **passes**: '
                   'it meets all six criteria, H1-H6.' % s['method']['Z'])
    else:
        out.append('In Random mode, the mode the PREREG-2\'s verdict is about, the round-2 EDT method (v2.1, Z = %g) **fails**: it '
                   'misses %s of the six criteria (%s).' % (s['method']['Z'], len(r['failing']), ', '.join(r['failing'])))
    e = cf['energetic']
    out.append('In Energetic mode, scored as its own set, which decides only whether EDT is shown for energetic '
               'runs (P11), it %s%s.' % (_pass(e['pass']), '' if e['pass'] else ' (%s)' % ', '.join(e['failing'])))
    tot = {k: sum(s['tables'][lab]['frozen'][k] for lab in LABELS) for k in ('n', 'n_ok_truth', 'n_wrong_silent')}
    up = {k: sum(s['tables'][lab]['upstream'][k] for lab in LABELS) for k in ('n_ok_truth', 'n_wrong_silent')}
    out.append('Over all %s rows, it gave a confident value (ok) with a known truth on %s, and was more than 5 %% '
               'off without warning (wrong-silent) on %s of them (%s). Upstream\'s EDT on the same rows was '
               'wrong-silent on %s of its %s (%s).' % (
                   _n(tot['n']), _n(tot['n_ok_truth']), _n(tot['n_wrong_silent']),
                   _pct(tot['n_wrong_silent'], tot['n_ok_truth'], 2), _n(up['n_wrong_silent']), _n(up['n_ok_truth']),
                   _pct(up['n_wrong_silent'], up['n_ok_truth'], 2)))
    rtl = {lab: s['tables'][lab]['frozen']['refused'].get(H4_RTL, 0) for lab in LABELS}
    out.append('It refused %s rows as receiver_too_large (the ball is too large for the room\'s decay): %s.'
               % (_n(sum(rtl.values())), ', '.join('%s %s' % (NAMES[lab], _n(v)) for lab, v in rtl.items())))
    excl = Counter()
    for lab in LABELS:
        excl.update(s['tables'][lab]['frozen']['excluded'])
    if excl:
        out.append('%s rows have no usable truth and are left out of every count that needs one, though not of the '
                   'ok and usable shares (P27): %s.' % (_n(sum(excl.values())), _counts(excl)))
    empty = [NAMES[lab] for lab in LABELS if not per[lab]]
    if empty:
        out.append('No rows were given for %s, so every criterion that needs them fails: a set that answers '
                   'nothing cannot pass (P27).' % ', '.join(empty))
    out.append('The method file was checked before anything ran: sha256 %s, as PREREG-2.md pins it.'
               % s['method']['sha256'])
    return ' '.join(out)


def report_md(s):
    """REPORT.md from a summary (summary.json's content): plain words first, then the tables."""
    cf = s['criteria']['frozen']
    L = ['# M8b EDT held-out test, round 2: the scorer\'s report', '', '**In plain words.** ' + _plain(s), '',
         '**What each criterion found** (the frozen method, Random mode; Energetic where it differs):', '']
    for h in H_IDS:
        line = '- **%s %s.** %s. %s.' % (h, _pass(cf['random'][h]['pass']), H_TEXT[h], _why(h, cf['random'][h]))
        if h in ('H2', 'H3', 'H4', 'H5'):
            line += ' Energetic: %s; %s.' % (_pass(cf['energetic'][h]['pass']), _why(h, cf['energetic'][h]))
        L.append(line)
    L += ['', '## Criteria (PREREG-2.md)', '',
          '| | Criterion | Random | Energetic |', '|---|---|---|---|']
    for h in H_IDS:
        L.append('| %s | %s | %s | %s |' % (h, H_TEXT[h], _pass(cf['random'][h]['pass']), _pass(cf['energetic'][h]['pass'])))
    L.append('| | **All six** | **%s** | **%s** |' % (_pass(cf['random']['pass']), _pass(cf['energetic']['pass'])))
    L += ['', 'Energetic decides only whether EDT is shown for energetic runs (P11); H1 and H6 hold no SPPS row and are the '
              'same for both modes.', '',
          '## Each set (FINAL.md section 3)', '',
          'Each cell reads frozen (Z = %g) · upstream, except benefit/regress, which is the method\'s. '
          'Wrong-silent: ok and more than 5 %% off, of the ok rows with a truth. Truth outside the range: of the '
          'usable rows with a truth (P28\'s tolerance). Error: |edt / truth - 1| over usable rows with a truth. '
          'Half-width: (edt_hi - edt_lo) / (2 edt) over usable rows. Benefit/regress against upstream: rows with a '
          'truth on which only the method, or only upstream, is ok and within 5 %%. Without a truth: rows left out '
          'of every count that needs one (P27).' % s['method']['Z'], '',
          '| Set | rows | usable | ok | wrong-silent | truth outside range | median error | p95 error | '
          'median half-width | benefit/regress | without a truth |', '|---|---|---|---|---|---|---|---|---|---|---|']
    for lab in LABELS:
        t = s['tables'][lab]
        f = [t[name] for name in INSTANCES]
        v = t['versus_upstream']
        L.append('| %s | %s | %s | %s | %s | %s | %s | %s | %s | %s | %s |' % (
            NAMES[lab], _n(f[0]['n']),
            ' · '.join(_pct(x['n_usable'], x['n']) for x in f),
            ' · '.join(_pct(x['n_ok'], x['n']) for x in f),
            ' · '.join('%s of %s' % (_n(x['n_wrong_silent']), _n(x['n_ok_truth'])) for x in f),
            ' · '.join('%s of %s' % (_n(x['n_out_of_range']), _n(x['n_usable_truth'])) for x in f),
            ' · '.join(_fmt_rel(x['med_abs_err']) for x in f),
            ' · '.join(_fmt_rel(x['p95_abs_err']) for x in f),
            ' · '.join(_fmt_rel(x['med_half_width']) for x in f),
            '%s/%s' % (_n(v['frozen']['benefit']), _n(v['frozen']['regress'])),
            _counts(f[0]['excluded'])))
    L += ['', '## H3: subgroups over 10 % wrong-silent (PREREG-2.md, P29)', '']
    any_bad = False
    for mode in MODES:
        g = cf[mode]['H3']
        bad = [(k, x) for k, x in g['subgroups'].items() if not x['pass']]
        L.append('- frozen, %s: %s subgroups, %s judged, %s over 10 %%.' % (
            mode, _n(g['n_subgroups']), _n(g['n_judged']), _n(g['n_failing'])))
        for k, x in bad:
            any_bad = True
            L.append('  - `%s`: %s of %s ok rows with a truth wrong-silent (%s)' % (
                k, _n(x['n_wrong_silent']), _n(x['n_ok_truth']), _pct(x['n_wrong_silent'], x['n_ok_truth'])))
    if not any_bad:
        L.append('- No judged subgroup is over 10 %.')
    L += ['', '## H4: at 1 ms, design T60 ≤ 3 s and receiver radius ≤ 0.5 m (PREREG-2.md, P30\')', '',
          'n is the rows left in the denominator: the rows in the filter (n before) less those refused as receiver_too_large, '
          'which are counted beside it.', '',
          '| Mode | Set | n before | receiver_too_large (count, share) | n | usable | ok | other refusals |',
          '|---|---|---|---|---|---|---|---|']
    for mode in MODES:
        for st, p in cf[mode]['H4']['per_set'].items():
            L.append('| %s | %s | %s | %s (%s) | %s | %s | %s | %s |' % (
                mode, NAMES[st], _n(p['n_before']), _n(p['n_receiver_too_large']),
                _pct(p['n_receiver_too_large'], p['n_before']), _n(p['n']), _pct(p['n_usable'], p['n']),
                _pct(p['n_ok'], p['n']), _counts(p['refused'])))
    L += ['', "## H5: wrong-silent against upstream's on the same rows (P31)", '',
          '| Mode | Set | rows with a truth | method | upstream | |', '|---|---|---|---|---|---|']
    for mode in MODES:
        for st, p in cf[mode]['H5']['per_set'].items():
            L.append('| %s | %s | %s | %s | %s | %s |' % (
                mode, NAMES[st], _n(p['n_eligible']), _n(p['n_wrong_silent']),
                _n(p['n_wrong_silent_upstream']), _pass(p['pass'])))
    L += ['', '## H6: the attack classes (P26)', '']
    h6r = cf['random']['H6']
    if not h6r['classes']:
        L.append('No attack class was scored, so H6 fails (P27).')
    for c in h6r['classes']:
        a = c['agreement']
        L.append('- `%s`: %s of %s draws wrong-silent, %s; panel: %s (%s implausible, %s plausible, '
                 '%s unreadable, %s missing); %s.' % (
                     c['id'], _n(c['n_wrong_silent']), _n(c['n_draws']),
                     'reproducible' if c['reproducible'] else 'not reproducible', c['panel'], a['implausible'],
                     a['plausible'], a['unreadable'], a['missing'], _pass(c['pass'])))
        for i, (v, why) in enumerate(zip(c['votes'], c['reasons']), 1):
            L.append('  - judge %d: %s. %s' % (i, v or 'no readable vote (counts as plausible)', why or ''))
    L += ['', '## Refusals, by reason and set (PREREG-2.md)', '',
          'Refusals are never counted as wrong. Each cell reads frozen (Z = %g) · upstream.' % s['method']['Z'], '']
    for lab in LABELS:
        t = s['tables'][lab]
        L.append('- %s: %s' % (NAMES[lab], ' · '.join(_counts(t[name]['refused']) for name in INSTANCES)))
    L += ['', '### receiver_too_large over all radii, by set and radius class (frozen)', '',
          'Count of rows refused as receiver_too_large, of the set\'s rows in the class (share). Energetic is its own set (P11).', '',
          '| Set | R <= 0.5 m | 0.5-1.0 m | R > 1.0 m |', '|---|---|---|---|']
    for lab in LABELS:
        t = s['tables'][lab]['rtl_by_r_class']
        L.append('| %s | %s |' % (NAMES[lab], ' | '.join('%s of %s (%s)' % (_n(t[c]['n_receiver_too_large']), _n(t[c]['n_rows']),
                                                                             _pct(t[c]['n_receiver_too_large'], t[c]['n_rows']))
                                                          for c in R_CLASSES)))
    L += ['', '## The run', '',
          '- Method: `%s`, sha256 `%s`, checked before anything ran (PREREG-2.md), Z = %g.' % (
              s['method']['path'], s['method']['sha256'], s['method']['Z']),
          '- Upstream: the port `%s`, sha256 `%s` (`%s`, P32).' % (
              s['upstream']['path'], s['upstream']['sha256'], s['upstream']['source']),
          '- Inputs: %s rows; %s. Attack classes: %s, with votes for %s.' % (
              _n(s['inputs']['n']), ', '.join('%s %s' % (NAMES[lab], _n(s['inputs']['per_set'][lab])) for lab in LABELS),
              _n(s['inputs']['attack_classes']), _n(s['inputs']['voted_classes'])),
          '- The truth of each set (PREREG.md:34):']
    for k, v in s['truth_functions'].items():
        L.append('  - %s: %s.' % (NAMES[k], v))
    L.append('- Generated %s, Python %s, numpy %s.' % (s['generated']['at'], s['generated']['python'],
                                                      s['generated']['numpy']))
    return '\n'.join(L) + '\n'


def _fmt_rel(x):
    return '–' if x is None else '%.2f %%' % (100.0 * x)
