"""Synth-fresh (HARNESS-PLAN.md P23, P24, P33): double slopes through the critique's committed
generator, docs/investigations/2026-09-27-edt-simplify/critique/synth.py (hash pinned as
corpus.SYNTH_SHA256_LF), noise-free (PREREG.md:55).

Built in section 6, step 6 (T15, and Synth-fresh's half of T22b). Nothing here is copied from
target/: the generator is a committed file run through its hash gate, and the draw is P23's and P24's
text, so provenance.json has no entry for this file. Tests T13, T15 and T22 hold the contract below.

Contract:
- draw(seed, *, a1=None) -> [spec]: calls driver.require_not_heldout(seed, a1) first. 500 rows for
  each rate ratio in RATIOS and step in STEPS_MS, 4,000 in all. A spec is a dict with 'id',
  'ratio', 'step_ms', 'dt' (s), 't60_s' (of k1), 'k' [k1, k2 = k1 / ratio], 'A' [1, E2 * k2] with
  E2 = E1 * 10 ** (late_share_db / 10) and E1 = 1 / k1 (scan_i4.py:20-23), 'late_share_db',
  'drr_db', 'Ed' (= S_rev * 10 ** (drr_db / 10), S_rev = sum A_i / k_i), 'R_m', 'half_width'
  (R / C), 'd_m', 'gap_ms', 'delay_ms', 'emission_steps' (ceil(float32(delay) / float32(dt)), as
  results/spps.rs:49-52), 'emission_s' (emission_steps * dt), 't_arrival' (emission_s + d / C),
  'run_over_t60' (f), 'run_s' (t_arrival + f * t60_s). Exactly half of each ratio-and-step cell
  has no delay (P24).
- histogram(spec) -> numpy array: synth.histogram of the row, round(run_s / dt) bins, nothing
  before the emission.
- truth(spec) -> float: synth.truth_edt(t_arrival, Ed, gap, A, k) (P15, Definition A).
- twin(spec) -> the same row with no delay.
- BOUNDS: {quantity: (lo, hi)} in weak_spots.json's vocabulary, for every quantity this set draws
  (P24): its ranges, widened where P33 needs it.

The bounds (BOUNDS, P24 as the plan now states it, revised by HARNESS-PLAN.md 8.2's second call).
P23 and P24's ranges stand except two, which P33 widens because weak_spots.json holds a binding
value outside them, each exactly to that value:
- d's top, 30 m to 30.8022 m: scan 1's row as the critique first ran it (target's
  critique/scan_i12.json, no d cutoff), which put the arrival 0.95 of a step past the start of a
  step (scan1|V20000|T0.3|d30|gap{5,10}|5ms|ph0.95: d 30.8022 m);
- the delay from 60 ms to 60.00000284984708 ms: z3's dis-012, dis-016 and dis-029, delayed 60 steps
  of their float32 1 ms step (corpus.py _z3_quantities: delay_ms = steps * dt * 1e3).
d - R's floor (P24's d >= R + 0.2 m) and d's floor (P24's 0.4 m) do NOT move, as 8.2's second call
settles: the one weak-spot row that used to widen them, scan1|V200|T3|d0.7|gap{5,10}|10ms|ph0.05
(d 0.1716 m, R 0.31 m, so d - R = -0.1384 m), is the generator's own defect, not the method's (see
"What d < R costs" below), and P33 gains the exception that such a row does not bind a bound: a weak
spot whose own histogram does not hold its truth is not evidence the fresh set must reach there.
Draws therefore redraw while d < R + 0.2 m, as P24 first said. weak_spots.json still lists both rows
(575 rows, T22a; corpus.histogram_holds_truth), with every other field as before, but their d_m and
d_minus_R_m are now null: they no longer constrain D_M or D_MINUS_R_MIN_M (corpus.py scan1, P33),
not merely left wide of them. The PREREG fixes none of these ranges (PREREG.md:37-41
fixes the ratios, the DRR, the steps and the run lengths), and the product accepts every value:
PHYSICS.md's hard limits (P25) are what the product accepts, T60 0.1-10 s and DRR -40 to +30 dB, and
crates/simpa-core/src/validate/project.rs has no rule on a receiver's distance from a source and
takes any finite delay >= 0 that starts before the run ends. Every other binding weak-spot value
lies inside P24's ranges (T60 0.125-3.0 s, R 0.31-1.49 m, gap 0-10 ms; no weak row carries a late
share), so no other bound moves. d - R's top is what d's top and R's bottom give, 30.8022 - 0.1 m.

What d < R costs (C:/tmp/m8b-edt/step6-synth/check_synth.py, dev seeds only, the frozen method not
run): when the ball holds the source, synth.ball_atoms puts nearly all of the direct sound before
t = 0, where synth.histogram drops it. It divides each atom's weight by max(rho, 1e-9) with
rho = C tau, which is negative before t = 0, so an atom there weighs about 1e8 times one after it.
Scan 1's weak row that used to set d - R's bottom keeps 1.5e-7 of its direct sound, and its logged
EDT, 3.0003 s, is within 0.1 % of the truth of what its histogram holds (3.0034 s; the row's own
truth is 3.3244 s). In 20 dev draws 11.45 rows per draw have d < R; 227 of those 229 keep under
0.03 % of their direct sound (the other 2 lie within half an atom's spacing, R / 400, of d = R, and
keep all of it), and on 2.85 rows per draw the truth of what the histogram holds is more than 5 % off
the row's truth. Every row with d >= R keeps its whole direct sound. HARNESS-PLAN.md 8.2's second
call reads this as the generator's defect, not the method's, and excludes d < R by redrawing, with
each redraw's reason recorded (draw(..., rejections=...), reason 'd_near_source').

Where the contract is silent:
- generator() executes critique/synth.py only from bytes whose sha256, with any CRLF made LF, is
  corpus.SYNTH_SHA256_LF, as ism_fresh.generator() does for ism.py: it checks on every call,
  executes the copy once per path and process, and on a mismatch raises VoidRun with
  checked_sha256 and checked_path set and nothing of the file executed.
- The draw: np.random.default_rng(SeedSequence(seed)), the cells in the order of RATIOS then
  STEPS_MS, and in each cell first a permutation that marks exactly 250 rows delayed, then for each
  row, in this order: T60 (log-uniform), late share, DRR, R (log-uniform), d (redrawn alone while
  d - R < D_MINUS_R_MIN_M, i.e. d < R + 0.2 m, P24 as 8.2's second call restates it), gap, the delay
  (delayed rows only) and f. Each uniform value is clipped to its range, so no rounding takes it
  outside. A delayed row's delay is drawn on (0, 60.00000284984708] ms, never 0, so 'delay_ms' == 0
  marks exactly the undelayed half. Ids are 'synth|<ratio>|<step>ms|<index in its cell>'. Every
  number is a Python float or int.
- draw(seed, *, a1=None, rejections=None): when the caller passes a dict for rejections, draw()
  counts each d-redraw into it under the reason 'd_near_source' (rejections[reason] =
  rejections.get(reason, 0) + 1); rejections is left untouched when None (the default), so the
  bare-list contract above holds unchanged for every existing caller.
- make_spec(**params) builds a spec from the drawn values, with dt = step_ms * 1e-3 and every
  derived field computed as the contract writes it; draw() and twin() use it, and so can the
  attack kit.
- histogram(spec): the undelayed response, synth.histogram with the arrival d / C (and the row's
  half_width, Ed, gap, A and k) over n - s bins, n = round(run_s / dt) and s = emission_steps,
  after s empty bins. An emission at a whole step shifts the whole response by whole steps, so this
  is the delayed row's response; synth.histogram given t_arrival itself would instead weight the
  ball as if it sat C * t_arrival from the source (synth.ball_atoms takes the ball's distance as
  C * t_arr), not d.
- twin(spec): make_spec with the row's values and delay 0, its id the row's with '|twin' added.
- truth(spec) passes t_arrival as T15 does. It is NaN for every DRR above 10 log10(9) = 9.54 dB
  (HARNESS-PLAN.md 8.1); those rows stay in the draw, and the scorer excludes them as truth_nan
  (P27).
"""
import math
import operator
from pathlib import Path

import numpy as np

from . import driver
from .corpus import SYNTH, SYNTH_SHA256_LF, VoidRun, exec_module, sha256_bytes  # one VoidRun for the harness

C = 343.2                           # critique/synth.py:15
RATIOS = (1.5, 5.0)
STEPS_MS = (1.0, 2.0, 5.0, 10.0)
ROWS_PER_CELL = 500
HELDOUT_SEED = 2026100101           # P24

T60_S = (0.1, 10.0)                 # P24, log-uniform: PHYSICS.md's T60 limit (P25)
LATE_SHARE_DB = (-20.0, -3.0)       # P23
DRR_DB = (-20.0, 10.0)              # PREREG.md:39
R_M = (0.1, 1.5)                    # P24, log-uniform
D_M = (0.4, 30.8022)                # P24's 0.4-30 m; top widened by P33: scan 1, first run
D_MINUS_R_MIN_M = 0.2               # P24's d >= R + 0.2 m, as 8.2's second call restates it (not
                                     # widened: the one weak-spot row that justified -0.1384 m is the
                                     # generator's own defect, not the method's, and no longer binds)
GAP_MS = (0.0, 40.0)                # P24
DELAY_MS = (0.0, 60.00000284984708)  # P24's 60 ms, widened by P33: z3, 60 steps of a float32 1 ms step
RUN_OVER_T60 = (0.3, 3.0)           # PREREG.md:41
BOUNDS = {
    't60_s': T60_S,
    'late_share_db': LATE_SHARE_DB,
    'drr_db': DRR_DB,
    'R_m': R_M,
    'd_m': D_M,
    'd_minus_R_m': (D_MINUS_R_MIN_M, D_M[1] - R_M[0]),
    'gap_ms': GAP_MS,
    'delay_ms': DELAY_MS,
    'run_over_t60': RUN_OVER_T60,
}
PARAMS = ('ratio', 'step_ms', 't60_s', 'late_share_db', 'drr_db', 'R_m', 'd_m', 'gap_ms', 'delay_ms',
          'run_over_t60')

_LOADED = {}        # resolved path -> the module executed from that path's checked bytes


def generator(path=None):
    p = Path(SYNTH if path is None else path).resolve()
    data = p.read_bytes().replace(b'\r\n', b'\n')
    h = sha256_bytes(data)
    if h != SYNTH_SHA256_LF:
        e = VoidRun('%s has sha256 %s with its line ends made LF, not %s: it is not the critique\'s generator '
                    '(P15, P24), and nothing of it was executed' % (p, h, SYNTH_SHA256_LF))
        e.checked_sha256 = h
        e.checked_path = p
        raise e
    mod = _LOADED.get(p)
    if mod is None:
        mod = exec_module('m8b_synth', p, data)
        mod.checked_sha256 = h
        mod.checked_path = p
        _LOADED[p] = mod
    return mod


def make_spec(*, id, ratio, step_ms, t60_s, late_share_db, drr_db, R_m, d_m, gap_ms, delay_ms, run_over_t60):
    dt = step_ms * 1e-3
    k1 = 6 * math.log(10) / t60_s
    k2 = k1 / ratio
    E1 = 1.0 / k1
    E2 = E1 * 10 ** (late_share_db / 10)
    A = [1.0, E2 * k2]
    k = [k1, k2]
    S_rev = sum(a / kk for a, kk in zip(A, k))
    steps = math.ceil(float(np.float32(delay_ms * 1e-3) / np.float32(dt)))       # results/spps.rs:49-52
    emission_s = steps * dt
    t_arrival = emission_s + d_m / C
    return dict(id=str(id), ratio=float(ratio), step_ms=float(step_ms), dt=dt, t60_s=float(t60_s), k=k, A=A,
                late_share_db=float(late_share_db), drr_db=float(drr_db), Ed=S_rev * 10 ** (drr_db / 10),
                R_m=float(R_m), half_width=R_m / C, d_m=float(d_m), gap_ms=float(gap_ms), delay_ms=float(delay_ms),
                emission_steps=int(steps), emission_s=emission_s, t_arrival=t_arrival,
                run_over_t60=float(run_over_t60), run_s=t_arrival + run_over_t60 * t60_s)


def _uniform(rng, box):
    lo, hi = box
    return min(max(lo + (hi - lo) * rng.random(), lo), hi)


def _log_uniform(rng, box):
    lo, hi = box
    return min(max(math.exp(math.log(lo) + (math.log(hi) - math.log(lo)) * rng.random()), lo), hi)


def draw(seed, *, a1=None, rejections=None):
    driver.require_not_heldout(seed, a1)
    rng = np.random.default_rng(np.random.SeedSequence(operator.index(seed)))
    assert DELAY_MS[0] == 0.0
    rows = []
    for ratio in RATIOS:
        for step_ms in STEPS_MS:
            delayed = rng.permutation(np.arange(ROWS_PER_CELL) < ROWS_PER_CELL // 2)
            for i in range(ROWS_PER_CELL):
                t60 = _log_uniform(rng, T60_S)
                late = _uniform(rng, LATE_SHARE_DB)
                drr = _uniform(rng, DRR_DB)
                R = _log_uniform(rng, R_M)
                d = _uniform(rng, D_M)
                while d - R < D_MINUS_R_MIN_M:                    # HARNESS-PLAN.md 8.2, second call:
                    if rejections is not None:                    # the source inside the receiver ball
                        rejections['d_near_source'] = rejections.get('d_near_source', 0) + 1
                    d = _uniform(rng, D_M)
                gap = _uniform(rng, GAP_MS)
                delay = DELAY_MS[1] * (1.0 - rng.random()) if delayed[i] else 0.0      # (0, max]
                f = _uniform(rng, RUN_OVER_T60)
                rows.append(make_spec(id='synth|%g|%gms|%03d' % (ratio, step_ms, i), ratio=ratio, step_ms=step_ms,
                                      t60_s=t60, late_share_db=late, drr_db=drr, R_m=R, d_m=d, gap_ms=gap,
                                      delay_ms=delay, run_over_t60=f))
    return rows


def histogram(spec):
    S = generator()
    dt = spec['dt']
    s = spec['emission_steps']
    n = int(round(spec['run_s'] / dt))
    if not 0 <= s < n:
        raise ValueError('%s: emission at step %d of a %d-bin run' % (spec['id'], s, n))
    b = S.histogram(dt, (n - s) * dt, spec['d_m'] / C, spec['half_width'], spec['Ed'], spec['gap_ms'] * 1e-3,
                    spec['A'], spec['k'])
    if len(b) != n - s:
        raise RuntimeError('%s: synth.histogram gave %d bins, not %d' % (spec['id'], len(b), n - s))
    out = np.zeros(n)
    out[s:] = b
    return out


def truth(spec):
    S = generator()
    return float(S.truth_edt(spec['t_arrival'], spec['Ed'], spec['gap_ms'] * 1e-3, spec['A'], spec['k']))


def twin(spec):
    p = {q: spec[q] for q in PARAMS}
    p['delay_ms'] = 0.0
    return make_spec(id=spec['id'] + '|twin', **p)
