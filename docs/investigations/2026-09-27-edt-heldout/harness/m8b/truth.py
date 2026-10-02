"""The truth of SPPS-fresh rows (HARNESS-PLAN.md P15-P19).

P15's Definition A, as the corpus read it: the direct sound a step at the arrival t, every reflection
where the ball records it. truth_ideal is the corpus's own truth function, run from two text copies:
m8b/_truth_ideal.py (attack_ism.py:55-77, sha256 3fb8a84b...) and the line fit it calls,
m8b/_mirror.py (rerun/mirror.py:20 and 109-146, sha256 c5603d5b...), each a header and then the source
lines unchanged below a marker. provenance.json records both. copies() executes a copy only after its
bytes, any CRLF made LF, hash to the value pinned here (VoidRun otherwise, with checked_sha256 and
checked_path set and nothing of it executed). It checks on every call and executes each copy once per
process, so a copy changed on disk voids the next truth. check_copies() compares each copy's body with
its source lines in target/, for the receipts; nothing else here reads target/.

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

Where the contract is silent:
- Bin k is [k dt, (k + 1) dt), with its edges computed as k * dt, and it is direct when
  k dt < t_arr + h and (k + 1) dt > t_arr - h. split returns float64 arrays the length of bins.
  It refuses dt <= 0, and h <= 0 for a receiver that is not blocked: a ball has R > 0 (P8: 0.31 m),
  and the half-open window of a point would hold no bin. A blocked series with no energy has no
  first bin with energy, so its t is NaN.
- read is NaN for a series with no energy, or with a NaN in it, without calling truth_ideal. It
  raises ValueError for an arrival at or past the series' end, where truth_ideal would index past
  the last bin.
- last10_share takes the last n // 10 of n bins, and at least the last bin.
- uncertainty needs K >= 2 (statistics.stdev raises otherwise). It is NaN when an EDT is not finite
  (statistics.stdev would raise) or edt_sum is not a finite positive number.
- verdict: a share or u that is NaN or None cannot be shown to meet its bound, so it fails it:
  'truth_truncated' or 'truth_uncertain', never 'ok'.

What the split costs, measured on the corpus only (P4): on the 944 corpus ISM receiver-bands with a
truth, read from a 0.1 ms reference with air per step as T7 builds it, P15's split is within 0.02 % of
the image-source split wherever the first reflection arrives 2R/c (1.81 ms) or more after the direct
sound. Where it arrives sooner, part of that reflection lies in the direct window and counts as
direct: 181 rows are off by more than 0.3 % and 36 by more than 1 %, at most 2.08 % (20 kHz); at
125 Hz-4 kHz, 49 of 477 by more than 0.3 %, at most 1.20 %. T7's six rows are within 0.3 %.
(C:/tmp/m8b-edt/step6-truth/check_truth_b2.json, 2026-10-01.)

HARNESS-PLAN.md 8.2's third call (P15's split close to the direct sound, 8.1's second open point):
a row whose first reflection falls within 2R/c of the direct sound is scored on a truth the split may
move by up to the figures just above, 1.20 % in 125 Hz-4 kHz or 2.08 % (rounded up to 2.1 %) above.
Such a row's wrong-silent verdict is not trusted when its own error already sits close enough to the
5 % line that the split's own slop could move it across: split_borderline(...) below is that test, to
be read by whatever builds a set's rows (the SPPS-fresh row-builder, once written) once the method's
edt is known, so that a borderline row's truth_status is set to 'truth_split_borderline' in place of
'ok' before scoring. It does not touch verdict() or assess(), whose status vocabulary and behaviour on
every existing caller (T9) are unchanged: a row that split_borderline flags is still 'ok' as far as
verdict() is concerned, and becomes 'truth_split_borderline' only in the row the scorer is given,
exactly as P27 already excludes 'truth_nan', 'truth_truncated' and 'truth_uncertain' rows from every
truth-based count while keeping them in the ok and usable shares and in H4.
"""
import json
import math
import statistics
import types
from pathlib import Path

import numpy as np

from .corpus import VoidRun, sha256_bytes  # one VoidRun for the harness

U_MAX = 0.01            # P19
TRUNC_SHARE = 1e-6      # P18
JND = 0.05                              # PREREG.md:48; HARNESS-PLAN.md 8.2, third call
SPLIT_BORDERLINE_BAND_MAX_HZ = 4000.0   # 125 Hz-4 kHz against above (8 kHz, 16 kHz, 20 kHz)
SPLIT_BORDERLINE_BOUND_LOW = 0.012      # 1.2 % in 125 Hz-4 kHz (step 6, 8.1's measured figure)
SPLIT_BORDERLINE_BOUND_HIGH = 0.021     # 2.1 % above (step 6 measured 2.08 %, rounded up)

HERE = Path(__file__).resolve().parent
MIRROR_COPY = HERE / '_mirror.py'
IDEAL_COPY = HERE / '_truth_ideal.py'
# Each copy's sha256, the whole file with its line ends LF (provenance.json's copy_sha256).
MIRROR_COPY_SHA256 = '9824f4bb1d0e4f24554f31efc7749a20ba243bf8ef971e4f88d82ad5b4ebad79'
IDEAL_COPY_SHA256 = '2a37e532855df11e61bfc402ef2637c3e3acc2c372f29aa22e5e8991c72c6f3e'
MIRROR_MARKER = '# ---- copied from mirror.py (do not edit below this line) ----'
IDEAL_MARKER = '# ---- copied from attack_ism.py (do not edit below this line) ----'

_LOADED = {}            # (resolved path, module name) -> the module executed from that path's checked bytes


def _checked(path, want, name, preset=None):
    p = Path(path).resolve()
    data = p.read_bytes().replace(b'\r\n', b'\n')
    h = sha256_bytes(data)
    if h != want:
        e = VoidRun('%s has sha256 %s with its line ends made LF, not %s: it is not the text copy of the corpus '
                    'truth that provenance.json records (P15), and nothing of it was executed' % (p, h, want))
        e.checked_sha256 = h
        e.checked_path = p
        raise e
    mod = _LOADED.get((p, name))
    if mod is None:
        mod = types.ModuleType(name)
        mod.__file__ = str(p)
        mod.__dict__.update(preset or {})
        exec(compile(data, str(p), 'exec'), mod.__dict__)
        mod.checked_sha256 = h
        mod.checked_path = p
        _LOADED[(p, name)] = mod
    return mod


def copies():
    """(mirror, ideal): the two text copies as modules, each executed only from bytes whose hash was
    checked on this call. The truth_ideal copy's M is the checked mirror copy, as attack_ism.py's M is
    mirror.py."""
    mirror = _checked(MIRROR_COPY, MIRROR_COPY_SHA256, 'm8b_truth_mirror')
    ideal = _checked(IDEAL_COPY, IDEAL_COPY_SHA256, 'm8b_truth_ideal', preset=dict(M=mirror))
    return mirror, ideal


def check_copies(target_root=None):
    """Each copy against its source in target/ (corpus.find_target_root): the source hashes as
    provenance.json records, the body below the marker is the text of the recorded lines (each range's
    lines joined by \\n, the ranges by two blank lines, as corpus.z3echo_body joins them), and the
    copy's hashes are those provenance.json records and this module pins. VoidRun on any difference.
    Returns a receipt per copy."""
    from .corpus import PROVENANCE, find_target_root
    prov = json.loads(PROVENANCE.read_text(encoding='utf-8'))
    target = Path(find_target_root(target_root))
    out = {}
    for name, path, marker, pin in (('m8b/_mirror.py', MIRROR_COPY, MIRROR_MARKER, MIRROR_COPY_SHA256),
                                    ('m8b/_truth_ideal.py', IDEAL_COPY, IDEAL_MARKER, IDEAL_COPY_SHA256)):
        e = prov[name]
        source = target.parent / e['source']
        data = source.read_bytes()
        got = dict(sha256=sha256_bytes(data), sha256_lf=sha256_bytes(data.replace(b'\r\n', b'\n')))
        bad = {k: (e['source_' + k], got[k]) for k in got if e['source_' + k] != got[k]}
        if bad:
            raise VoidRun('%s: its source %s has changed: %s' % (name, source, bad))
        lines = data.decode('utf-8').replace('\r\n', '\n').split('\n')
        want = '\n\n\n'.join('\n'.join(lines[a - 1:b]) for a, b in e['line_ranges']) + '\n'
        copy = path.read_bytes().replace(b'\r\n', b'\n')
        _, sep, body = copy.decode('utf-8').partition(marker + '\n')
        if not sep or body != want:
            raise VoidRun('%s is not the text of its source lines %s below its marker' % (path, e['line_ranges']))
        hashes = dict(copy_sha256=sha256_bytes(copy), copy_body_sha256_lf=sha256_bytes(body.encode('utf-8')))
        if hashes['copy_sha256'] != pin or any(e[k] != v for k, v in hashes.items()):
            raise VoidRun('%s: hashes %s differ from provenance.json or the pin %s' % (path, hashes, pin))
        out[name] = dict(source=e['source'], source_sha256=got['sha256'], line_ranges=e['line_ranges'],
                         body_lines=body.count('\n'), verified=True, **hashes)
    return out


def truth_ideal(direct, refl, t, dt):
    """attack_ism.py:55-77's truth_ideal, run from its checked text copy (copies())."""
    return copies()[1].truth_ideal(direct, refl, t, dt)


def split(bins, dt, t_arr, h, blocked=False):
    v = np.asarray(bins, dtype=np.float64)
    if v.ndim != 1:
        raise ValueError('a series is one-dimensional, not of shape %s' % (v.shape,))
    dt = float(dt)
    if not dt > 0:
        raise ValueError('dt must be positive, not %r' % dt)
    if blocked:
        nz = np.flatnonzero(v > 0)
        t = int(nz[0]) * dt if len(nz) else float('nan')
        return np.zeros_like(v), v.copy(), t
    t_arr, h = float(t_arr), float(h)
    if not h > 0:
        raise ValueError('h = R/c must be positive for a receiver that is not blocked, not %r' % h)
    edges = np.arange(len(v) + 1) * dt
    hit = (edges[:-1] < t_arr + h) & (edges[1:] > t_arr - h)
    return np.where(hit, v, 0.0), np.where(hit, 0.0, v), t_arr


def read(bins, dt, t_arr, h, blocked=False):
    direct, refl, t = split(bins, dt, t_arr, h, blocked)
    if not math.isfinite(t) or not float(direct.sum() + refl.sum()) > 0:
        return float('nan')
    if t / float(dt) >= len(refl):
        raise ValueError('t = %r s lies at or past the end of the %d-bin series' % (t, len(refl)))
    return float(truth_ideal(direct, refl, t, float(dt))[0])


def last10_share(bins):
    v = np.asarray(bins, dtype=np.float64)
    total = float(v.sum())
    if not total > 0:
        return float('nan')
    n = len(v)
    return float(v[n - max(1, n // 10):].sum()) / total


def uncertainty(edts, edt_sum):
    e = [float(x) for x in edts]
    if len(e) < 2:
        raise ValueError('P19 needs at least two references, not %d' % len(e))
    s = float(edt_sum)
    if not all(math.isfinite(x) for x in e) or not (math.isfinite(s) and s > 0):
        return float('nan')
    return statistics.stdev(e) / math.sqrt(len(e)) / s


def _number(x):
    return float('nan') if x is None else float(x)


def verdict(truth, u, share):
    if truth is None or not math.isfinite(truth):
        return 'truth_nan'
    if not _number(share) <= TRUNC_SHARE:
        return 'truth_truncated'
    if not _number(u) <= U_MAX:
        return 'truth_uncertain'
    return 'ok'


def split_borderline_bound(band_hz):
    """HARNESS-PLAN.md 8.2, third call: the split's measured bound of the 5 % line, by band (step 6,
    8.1's second open point): 1.2 % in 125 Hz-4 kHz, 2.1 % above."""
    b = _number(band_hz)
    if not math.isfinite(b):
        raise ValueError('band_hz must be a finite number, not %r' % (band_hz,))
    return SPLIT_BORDERLINE_BOUND_LOW if b <= SPLIT_BORDERLINE_BAND_MAX_HZ else SPLIT_BORDERLINE_BOUND_HIGH


def split_borderline(edt, truth, gap_s, h, band_hz):
    """HARNESS-PLAN.md 8.2, third call (P15): True when a row's first reflection falls within 2R/c
    (2 * h) of the direct sound, and its error against truth is within the split's measured bound of
    the 5 % line (JND) for its band (split_borderline_bound). Such a row's wrong-silent verdict is not
    trusted: the scorer reports it apart as 'truth_split_borderline' (P27), decided by none of
    H1-H3 or H5. gap_s is the time from the direct arrival to the first reflection (as corpus.py's
    'gap_ms' quantity, in seconds); edt and truth are the method's reading and the row's truth. False
    whenever a value needed is None or not finite, or h is not positive, or truth is not positive:
    the row is then judged exactly as P28 and P33 already do, with no borderline exception."""
    g, hh = _number(gap_s), _number(h)
    if not (math.isfinite(g) and math.isfinite(hh) and hh > 0):
        return False
    if not g < 2.0 * hh:
        return False
    e, t = _number(edt), _number(truth)
    if not (math.isfinite(e) and math.isfinite(t) and t > 0):
        return False
    err = abs(e / t - 1.0)
    return abs(err - JND) <= split_borderline_bound(band_hz)


def assess(refs, dt, t_arr, h, blocked=False):
    arrs = [np.asarray(r, dtype=np.float64) for r in refs]
    if len(arrs) < 2:
        raise ValueError('P19 needs at least two references, not %d' % len(arrs))
    n = min(len(a) for a in arrs)
    total = sum(a[:n] for a in arrs)
    truth = read(total, dt, t_arr, h, blocked)
    edts = [read(a, dt, t_arr, h, blocked) for a in arrs]
    u = uncertainty(edts, truth)
    share = last10_share(total)
    return dict(truth=truth, edts=edts, u=u, share=share, status=verdict(truth, u, share))
