"""The truth of SPPS-fresh rows (HARNESS-PLAN.md P15-P19).

STUB (HARNESS-PLAN.md section 6, step 3): every function returns None. Tests T6-T9 hold the
contract below (T6 through ism_fresh, which reads its truth with truth_ideal).

Contract (P15, Definition A):
- truth_ideal(direct, refl, t, dt) -> (edt, ts): a text copy of
  target/agents/followup-design/skeptic-gf3/attack_ism.py:55-77 (sha256 3fb8a84b...), with `line`,
  `integral` and MIN_REGRESSION_BINS copied from rerun/mirror.py (sha256 c5603d5b...:20, 109-146).
  provenance.json records both. NaN when the line has no fit.
- split(bins, dt, t_arr, h, blocked=False) -> (direct, refl, t): the bins overlapping
  [t_arr - h, t_arr + h) are direct and the rest reflected, t = t_arr. Blocked: direct is all 0,
  every bin is reflected, and t is the start of the first bin with energy (the critique's
  occluded truth, critique/scan_i1_occluded.py:1-4, 20).
- read(bins, dt, t_arr, h, blocked=False) -> float: truth_ideal's EDT of split(...).
- last10_share(bins) -> float: the energy in the last 10 % of the bins over the total (P18);
  NaN for a series with no energy.
- uncertainty(edts, edt_sum) -> float: P19's u, statistics.stdev(edts) (ddof 1) / sqrt(K) / edt_sum.
- verdict(truth, u, share) -> str: the first that applies of 'truth_nan' (truth not finite),
  'truth_truncated' (share > 1e-6, P18), 'truth_uncertain' (u > 0.01, P19), else 'ok'.
  A share of exactly 1e-6 and a u of exactly 0.01 are kept.
- assess(refs, dt, t_arr, h, blocked=False) -> dict(truth, edts, u, share, status): the K
  references summed bin by bin (to the shortest), truth = read(sum), edts = read of each
  reference, u = uncertainty(edts, truth), share = last10_share(sum), status = verdict(...).
"""
U_MAX = 0.01            # P19
TRUNC_SHARE = 1e-6      # P18


def truth_ideal(direct, refl, t, dt):
    return None


def split(bins, dt, t_arr, h, blocked=False):
    return None


def read(bins, dt, t_arr, h, blocked=False):
    return None


def last10_share(bins):
    return None


def uncertainty(edts, edt_sum):
    return None


def verdict(truth, u, share):
    return None


def assess(refs, dt, t_arr, h, blocked=False):
    return None
