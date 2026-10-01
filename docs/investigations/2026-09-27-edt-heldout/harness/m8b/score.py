"""The scorer (HARNESS-PLAN.md P27-P32; PREREG.md:44-63): definitions, exclusions, H1-H6, and the
evaluation run that writes REPORT.md (plain words first), summary.json and rows.jsonl.

STUB (HARNESS-PLAN.md section 6, step 3): every function returns None. Tests T18-T21 hold the
contract below.

Contract:
- A scored row is a dict: 'set' ('spps', 'ism', 'synth' or 'attack'), 'id', the method's
  'status', 'edt', 'edt_lo', 'edt_hi', 'reason', the row's 'truth' (float or None) and
  'truth_status' ('ok', 'truth_truncated', 'truth_uncertain' or 'truth_nan'; P18, P19), and the
  fields the H's group by: 'room', 'd_m', 'step_ms', 'band_hz', 'particles', 'seed', 'family',
  'design_t60_s'.
- classify(row) -> dict(ok, usable, has_truth, wrong_silent, covered): ok is status 'ok'; usable is
  'ok' or 'wide'; has_truth is truth_status 'ok' with a finite truth. wrong_silent is None without
  a truth, False for a row that is not ok, else |edt/truth - 1| > JND judged exactly (21/20 is 5 %
  and not wrong, whatever float division rounds it to). covered is None without a truth or for a
  row that is not usable, else edt_lo * (1 - 1e-9) <= truth <= edt_hi * (1 + 1e-9) (P28).
- tally(rows) -> dict: 'n', 'n_ok', 'n_usable', 'ok_share', 'usable_share' (every row, P27),
  'excluded' {truth_status: count} for the rows without a truth, 'refused' {reason: count},
  'n_ok_truth', 'n_wrong_silent', 'wrong_silent_share', 'n_usable_truth', 'n_covered', 'coverage'.
  A share whose denominator is 0 is None.
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
  - h4: each of 'spps' and 'ism' separately (P30): rows at step_ms 1 whose design_t60_s <= 3.0,
    usable >= 90 %; the ok share is reported. Rows without a truth stay in (P27). Returns
    'per_set' {set: dict(n, usable_share, ok_share, pass)}.
  - h5: each of 'spps', 'ism', 'synth' and 'attack' (P31): on the rows with a truth that both
    methods have (paired by id), the method's wrong-silent count strictly below upstream's; 0
    against 0 fails, and so does a set with no rows.
  - h6: classes [{'id', 'n_draws', 'n_wrong_silent', 'votes'}]: reproducible when 5 or more draws
    are wrong-silent; implausible only when attack.panel(votes) says so; fails when a
    reproducible class is plausible.
- evaluate(inputs, *, method_path=None, out_dir) -> summary dict: checks the method file's sha256
  first (method.load; VoidRun before anything is computed or written, PREREG.md:10), runs it, the
  Z = 3 instance and upstream on every input ({'set', 'id', 'bins', 'dt', 't_arrival', 'meta',
  'truth', 'truth_status', and the grouping fields}), and writes REPORT.md, summary.json and
  rows.jsonl into out_dir. summary['method'] = {'path', 'sha256', 'verified': True}.
"""
JND = 0.05
COVER_TOL = 1e-9


def classify(row):
    return None


def tally(rows):
    return None


def h1(rows_by_set):
    return None


def h2(rows):
    return None


def h3(rows):
    return None


def h4(rows_by_set):
    return None


def h5(frozen_by_set, upstream_by_set):
    return None


def h6(classes):
    return None


def evaluate(inputs, *, method_path=None, out_dir):
    return None
