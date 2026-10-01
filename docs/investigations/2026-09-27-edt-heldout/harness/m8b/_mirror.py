"""Text copy of the line fit that P15's truth reads, from the Python mirror of the shipped decay code.

Source: target/agents/followup-design/skeptic-gf3/rerun/mirror.py (gitignored), sha256 c5603d5ba863e9f6,
lines 20 (MIN_REGRESSION_BINS) and 109-146 (line, integral), copied unchanged below the marker, the
ranges joined by two blank lines. The source is CRLF; this file is LF (../.gitattributes). Nothing
else of mirror.py is copied: these lines use only math. m8b/provenance.json records the source hash,
the ranges and this file's hashes. truth.py executes this file only after its bytes hash to the
value truth.py pins, and truth.check_copies() compares the body with its source lines. Do not edit
below the marker.
"""
import math

# ---- copied from mirror.py (do not edit below this line) ----
MIN_REGRESSION_BINS = 2.0


def line(top, pieces, dt, top_db=0.0, bottom_db=-10.0):
    """(slope dB/s, span s) or (None, reason)."""
    db = lambda x: 10 * math.log10(x / top)
    parts = []
    for (u0, u1, s0, s1, shape) in pieces:
        if shape != 'log':
            continue
        l0, l1 = db(s0), db(s1)
        if l1 > top_db or l0 < bottom_db:
            continue
        q = (l1 - l0) / (u1 - u0)
        ua = u0 + (top_db - l0) / q if l0 > top_db else u0
        ub = u0 + (bottom_db - l0) / q if l1 < bottom_db else u1
        if ub > ua:
            mid = 0.5 * (ua + ub)
            parts.append((ub - ua, mid, l0 + q * (mid - u0), q))
    span = sum(p[0] for p in parts)
    if span < MIN_REGRESSION_BINS * dt * (1 - 1e-9):
        return None, 'range_too_short'
    mean = sum(p[0] * p[1] for p in parts) / span
    suu = sul = 0.0
    for (w, mid, lev, q) in parts:
        inner = w ** 3 / 12
        suu += w * (mid - mean) ** 2 + inner
        sul += w * (mid - mean) * lev + q * inner
    slope = sul / suu
    if slope >= 0:
        return None, 'not_decaying'
    return (slope, span), None


def integral(p):
    u0, u1, s0, s1, shape = p
    h = u1 - u0
    if shape == 'log' and s0 - s1 > 1e-12 * s0:
        return h * (s0 - s1) / math.log(s0 / s1)
    return h * (s0 + s1) / 2

