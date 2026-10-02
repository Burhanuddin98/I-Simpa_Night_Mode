"""The bed's truths for C50, C80, D50, Ts and SPL (../PREREG.md; ../ADDENDUM-1.md items 4-6).

Written from ISO 3382-1 A.2.3 and docs/params.md's definitions ("Clarity C50 and C80, definition D50, centre time Ts";
"Sound pressure level"), not ported from the product. u = t - t0, t0 the direct sound's centre:
    C_te = 10 lg(E[0, te] / E(te, inf)),  D50 = E[0, 50 ms] / E_total,  Ts = int u E du / E_total,
    SPL  = 10 lg(E_total / p0^2),  p0^2 = (20 uPa)^2 = 4e-10.
The direct sound is at u = 0 (ADDENDUM-1 item 4). Reflected energy is where the ball records it, spread evenly inside
each bin; any part of it before `t_from` is placed at u = 0. A window edge inside a bin splits the bin in proportion.

- binned(e0, bins, dt, t0, t_from=None): e0 the energy at u = 0 (the direct sound), bins[k] the energy in
  [k dt, (k+1) dt) (absolute time), t_from >= t0 the time before which bin energy counts as at u = 0 (t0 for S1, S2
  and C, whose direct sound is separate; t0 + R/c for set B, item 4). Returns a dict of the five metrics.
- closed_form(Ed, gap, A, k): the same five on the generator's model (critique/synth.py): Ed at u = 0, then a
  density sum_i A_i exp(-k_i (u - gap)) for u >= gap; infinite length.
"""
import math

import numpy as np

P0_SQUARED = 4e-10
WINDOWS = (0.05, 0.08)
METRICS = ('c50', 'c80', 'd50', 'ts', 'spl')


def _db(x):
    return 10 * math.log10(x) if x > 0 else (float('-inf') if x == 0 else float('nan'))


def _finish(total, late50, late80, moment):
    """The late energies are passed, not the early ones: a high C has a tiny late part, which total - early would lose."""
    return dict(
        c50=_db((total - late50) / late50) if late50 > 0 else float('nan'),
        c80=_db((total - late80) / late80) if late80 > 0 else float('nan'),
        d50=(total - late50) / total if total > 0 else float('nan'),
        ts=moment / total if total > 0 else float('nan'),
        spl=_db(total / P0_SQUARED),
    )


def binned(e0, bins, dt, t0, t_from=None):
    b = np.asarray(bins, dtype=float)
    t_from = t0 if t_from is None else float(t_from)
    if t_from < t0:
        raise ValueError('t_from %r before t0 %r' % (t_from, t0))
    n = len(b)
    edges = np.arange(n + 1) * dt
    lo, hi = edges[:-1], edges[1:]
    # the part of each bin after t_from, at its recorded time; the part before it, at u = 0
    after_lo = np.maximum(lo, t_from)
    frac_after = np.clip((hi - after_lo) / dt, 0.0, 1.0)
    e_after = b * frac_after
    head = float(e0) + float(b.sum() - e_after.sum())
    total = head + float(e_after.sum())
    # first moment: an evenly spread part [after_lo, hi] has its centre in the middle
    mid = 0.5 * (after_lo + hi) - t0
    moment = float(np.sum(e_after * np.where(frac_after > 0, mid, 0.0)))

    def late(te):
        edge = max(t0 + te, t_from)
        part = np.clip((hi - np.maximum(lo, edge)) / dt, 0.0, 1.0)
        return float(np.sum(b * np.minimum(part, frac_after)))

    return _finish(total, late(WINDOWS[0]), late(WINDOWS[1]), moment)


def closed_form(Ed, gap, A, k):
    A, k = np.asarray(A, float), np.asarray(k, float)
    rev = float(np.sum(A / k))
    total = float(Ed) + rev

    def late(te):            # reverberant energy after u = te
        return float(np.sum(A / k * np.exp(-k * max(0.0, te - gap))))

    # int_gap^inf u A e^{-k (u - gap)} du = A (gap / k + 1 / k^2)
    moment = float(np.sum(A * (gap / k + 1 / k ** 2)))
    return _finish(total, late(WINDOWS[0]), late(WINDOWS[1]), moment)


# limens (ISO 3382-1 Table A.1, STANDARDS-CHECK.md); T20 and T30 relative to the truth
LIMEN = dict(t20=0.05, t30=0.05, c50=1.0, c80=1.0, d50=0.05, ts=0.010, spl=1.0)
RELATIVE = frozenset(('t20', 't30'))


def limen(metric, truth):
    return LIMEN[metric] * abs(truth) if metric in RELATIVE else LIMEN[metric]
