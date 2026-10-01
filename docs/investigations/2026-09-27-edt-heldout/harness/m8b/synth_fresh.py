"""Synth-fresh (HARNESS-PLAN.md P23, P24, P33): double slopes through the critique's committed
generator, docs/investigations/2026-09-27-edt-simplify/critique/synth.py (hash pinned as
corpus.SYNTH_SHA256_LF), noise-free (PREREG.md:55).

STUB (HARNESS-PLAN.md section 6, step 3): every function returns None and BOUNDS is empty. Tests
T13, T15 and T22 hold the contract below.

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
"""
C = 343.2                           # critique/synth.py:15
RATIOS = (1.5, 5.0)
STEPS_MS = (1.0, 2.0, 5.0, 10.0)
ROWS_PER_CELL = 500
HELDOUT_SEED = 2026100101           # P24
BOUNDS = {}


def draw(seed, *, a1=None):
    return None


def histogram(spec):
    return None


def truth(spec):
    return None


def twin(spec):
    return None
