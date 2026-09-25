"""The composite early check (REGISTERED.txt): the w1d bracket with exact split readings, a
band-aware direct-symmetry air factor, and guards G1-G6. Python mirror for evaluation only.

A curve is (top, pieces), piece = (u0, u1, s0, s1, shape), as edt3 / bracket.common.
Nothing here runs a solver. Read-only on every input.
"""
import math
import sys

import numpy as np

BR = 'B:/repos/I-Simpa_Night_Mode/target/agents/followup-design/bracket'
sys.path.insert(0, BR)
import common as C  # noqa: E402
import edt3  # noqa: E402

DT_MAX32 = np.float32(0.002)
R_MIN32 = np.float32(0.31)
EDT_LIMIT = 0.005
DIRECT_DB = -3.0
RANGE_BOTTOM_DB = -10.0


def f32(x):
    return np.float32(x)


def direct_after_lb(sums, dt, t, h, tau, air):
    """Lower bound on direct energy after tau (tau >= t): the histogram energy before
    2*sqrt(t^2-h^2) - tau, times the air factor across a chord (bracket/rule.py, factor fixed)."""
    t_lo = math.sqrt(max(t * t - h * h, 0.0))
    x = 2 * t_lo - tau
    kx = int(math.floor(x / dt + 1e-12))
    if kx <= 0:
        return 0.0
    kx = min(kx, len(sums) - 1)
    return air * (sums[0] - sums[kx])


def geometry(v, dt, t, h, d1, c, clear, air, dsym=True):
    k0 = edt3.onset(v)
    st = edt3.decay_start(t, h, k0, dt)
    if st is None:
        return None
    sums = edt3.backward_sums(v)
    end = edt3.energy_end(v)
    t1 = max(t, d1 / c) if math.isfinite(d1) else t
    ur = t1 - h - t
    first = st['exact_bin']
    top_bin = st['top_bin']
    m = int(math.floor((t1 - h) / dt + 1e-12))
    lo_bin = max(m, top_bin)
    xmax = sums[lo_bin] - sums[first] if lo_bin < first else 0.0
    if dsym and clear and xmax > 0 and t1 - h >= t and air is not None:
        tau = max(lo_bin * dt, t)
        xmax = max(0.0, xmax - direct_after_lb(sums, dt, t, h, tau, air))
    return dict(st=st, sums=sums, end=end, t1=t1, ur=ur, first=first, top_bin=top_bin, m=m,
                xmax=xmax, top=sums[top_bin], u1=first * dt - t)


def slots(g, dt, t, w_last):
    """The stretches whose in-bin arrangement is bounded exactly: the lump (0, u1] and the window
    bins first..w_last. Each: (a, p0, b, hi, lo, k): the curve is hi before its drop and lo after.
    A window bin's drop lies in [p0, b], p0 = max(bin start, t1 - h - t): no reflected energy
    arrives earlier. The lump's reflected share X in [0, X_max] arrives in [max(0, u_r), u1]; X = 0
    is a drop at 0, so its drop set is {0} U [p0, u1]."""
    S = g['sums']
    s = lambda k: S[k] if k < len(S) else 0.0
    out = []
    ur = g['ur']
    u1 = g['u1']
    if u1 > 0:
        lo = s(g['first'])
        hi = min(g['top'], lo + g['xmax'])
        out.append((0.0, min(max(0.0, ur), u1), u1, hi, lo, None))
    for k in range(g['first'], min(w_last + 1, g['end'])):
        a = k * dt - t
        b = (k + 1) * dt - t
        out.append((a, min(max(a, ur), b), b, s(k), s(k + 1), k))
    return out


def earliest(slot):
    a, p0, b, hi, lo, k = slot
    return 0.0 if k is None else p0


def extreme_taus(sl, ubar):
    """Drop positions of the two readings that are the least-squares slope's extremes over the
    envelope (support fixed): F = integral of (u - ubar) L(u) du is maximised with every drop at an
    end of its set (the later end when the set's midpoint lies after ubar), and minimised with the
    drop at ubar, or at the admissible point nearest it on the side F favours."""
    t_max, t_min = [], []
    for slot in sl:
        a, p0, b, hi, lo, k = slot
        e0 = earliest(slot)
        t_max.append(b if 0.5 * (e0 + b) > ubar else e0)
        if ubar >= b:
            t_min.append(b)
        elif ubar >= p0:
            t_min.append(ubar)
        elif k is None:
            # ubar inside (0, p0), which the lump cannot drop in: 0 or p0, whichever gives less F
            t_min.append(p0 if 0.5 * p0 < ubar else 0.0)
        else:
            t_min.append(p0)
    return t_max, t_min


def build(g, dt, t, w_last, taus):
    """Curve with slot j dropping at taus[j]; after the window, the shipped log-linear model."""
    S = g['sums']
    s = lambda k: S[k] if k < len(S) else 0.0
    top = g['top']
    pieces = []
    sl = slots(g, dt, t, w_last)
    for (a, a_adm, b, hi, lo, k), tau in zip(sl, taus):
        if k is not None and lo <= 0:
            # the last bin with energy: flat until the earliest arrival, then falls to nothing,
            # left out of every fit (as rule.build)
            if a_adm > a:
                pieces.append((a, a_adm, hi, hi, 'log'))
            pieces.append((a_adm, b, hi, 0.0, 'lin'))
            continue
        if tau > a and hi > 0:
            pieces.append((a, tau, hi, hi, 'log'))
        if b > tau and lo > 0:
            pieces.append((tau, b, lo, lo, 'log'))
    start = max(w_last + 1, g['first'])
    for k in range(start, g['end']):
        a = k * dt - t
        b = (k + 1) * dt - t
        s0, s1 = s(k), s(k + 1)
        r = min(max(a, g['ur']), b)
        if s1 <= 0:
            if r > a:
                pieces.append((a, r, s0, s0, 'log'))
            if b > r:
                pieces.append((r, b, s0, 0.0, 'lin'))
            continue
        if r > a:
            pieces.append((a, r, s0, s0, 'log'))
        if b > r:
            pieces.append((r, b, s0, s1, 'log'))
    return top, pieces


def flat_only(top, pieces):
    """True when EDT's fit would see only flat pieces (no slope information)."""
    db = lambda x: 10 * math.log10(x / top)
    for (u0, u1, s0, s1, shape) in pieces:
        if shape != 'log' or s1 <= 0:
            continue
        l0, l1 = db(s0), db(s1)
        if l1 > 0.0 or l0 < RANGE_BOTTOM_DB:
            continue
        if l1 < l0:
            return False
    return True


def evaluate(v, dt, t, h, d1, c, clear, R, air, n_extra=1, guards=('G1', 'G2', 'G3', 'G3b'),
             exact_split=True):
    """Returns dict(edt=..., ts=...), each dict(ok, value, why, lo, hi, half) plus features."""
    out = {}
    refuse_both = None
    if 'G1' in guards and f32(dt) > DT_MAX32:
        refuse_both = 'step_too_coarse'
    elif 'G2' in guards and f32(R) < R_MIN32:
        refuse_both = 'receiver_too_small'
    g = geometry(v, dt, t, h, d1, c, clear, air)
    if g is None:
        why = refuse_both or 'arrival_misfit'
        return dict(edt=dict(ok=False, why=why), ts=dict(ok=False, why=why), feat={})
    first = g['first']
    mr = max(first, int(math.floor((g['t1'] - h) / dt)))
    w_last = mr + n_extra
    S = g['sums']
    s = lambda k: S[k] if k < len(S) else 0.0
    top = g['top']
    lvl_direct = 10 * math.log10(s(first) / top) if s(first) > 0 else -math.inf
    lvl_wend = 10 * math.log10(s(w_last + 1) / top) if s(w_last + 1) > 0 else -math.inf
    feat = dict(lvl_direct=lvl_direct, lvl_wend=lvl_wend, xmax=g['xmax'] / top, u1=g['u1'],
                ur=g['ur'], w_last=w_last, first=first)
    sl = slots(g, dt, t, w_last)
    low = build(g, dt, t, w_last, [earliest(x) for x in sl])
    high = build(g, dt, t, w_last, [x[2] for x in sl])
    # ---- Ts: LOW and HIGH are its extremes (monotone in S).
    if refuse_both:
        out['ts'] = dict(ok=False, why=refuse_both)
    elif s(first) <= 0:
        out['ts'] = dict(ok=False, why='no_reverberation')
    else:
        ts_lo, ts_hi = C.ts_of(*low), C.ts_of(*high)
        if not (math.isfinite(ts_lo) and math.isfinite(ts_hi)):
            out['ts'] = dict(ok=False, why='non_finite')
        else:
            mid = 0.5 * (ts_lo + ts_hi)
            half = 0.5 * (ts_hi - ts_lo)
            lim = C.ts_limit(mid)
            out['ts'] = dict(ok=half <= lim, why=None if half <= lim else 'early_unresolved',
                             value=mid, lo=ts_lo, hi=ts_hi, half=half, lim=lim)
    # ---- EDT
    # the log-linear-in-window reading only to locate u_bar
    ubar = C.edt_fit_mean(*_loglinear(g, dt, t), dt)
    cands = [low, high]
    if math.isfinite(ubar):
        if exact_split:
            tau_max, tau_min = extreme_taus(sl, ubar)
        else:  # w1d's split: every slot at an end by the midpoint rule, both directions
            tau_max = [x[2] if 0.5 * (earliest(x) + x[2]) > ubar else earliest(x) for x in sl]
            tau_min = [earliest(x) if 0.5 * (earliest(x) + x[2]) > ubar else x[2] for x in sl]
        cands += [build(g, dt, t, w_last, tau_max), build(g, dt, t, w_last, tau_min)]
    vals = [C.edt_of(tp, pc, dt) for tp, pc in cands]
    why = refuse_both
    if why is None and 'G3' in guards and lvl_direct < DIRECT_DB:
        why = 'direct_dominates'
    if why is None and 'G3b' in guards and not lvl_wend > RANGE_BOTTOM_DB:
        why = 'window_past_range'
    if why is None and any(not math.isfinite(x) for x in vals):
        why = 'non_finite'
    if why is None and any(flat_only(tp, pc) for tp, pc in cands):
        why = 'flat_fit'
    if why is None:
        lo, hi = min(vals), max(vals)
        m = 0.5 * (lo + hi)
        half = max(abs(lo / m - 1), abs(hi / m - 1))
        out['edt'] = dict(ok=half <= EDT_LIMIT, why=None if half <= EDT_LIMIT else 'early_unresolved',
                          value=m, lo=lo, hi=hi, half=half)
    else:
        fin = [x for x in vals if math.isfinite(x)]
        out['edt'] = dict(ok=False, why=why,
                          value=0.5 * (min(fin) + max(fin)) if len(fin) == len(vals) and fin else None)
    out['feat'] = feat
    return out


def _loglinear(g, dt, t):
    """The shipped model re-anchored at t1 - h (rule.build 'log' everywhere): only for u_bar."""
    S = g['sums']
    s = lambda k: S[k] if k < len(S) else 0.0
    pieces = []
    u1 = g['u1']
    if u1 > 0 and s(g['first']) > 0:
        pieces.append((0.0, u1, s(g['first']), s(g['first']), 'log'))
    for k in range(g['first'], g['end']):
        a = k * dt - t
        b = (k + 1) * dt - t
        s0, s1 = s(k), s(k + 1)
        r = min(max(a, g['ur']), b)
        if s1 <= 0:
            if r > a:
                pieces.append((a, r, s0, s0, 'log'))
            if b > r:
                pieces.append((r, b, s0, 0.0, 'lin'))
            continue
        if r > a:
            pieces.append((a, r, s0, s0, 'log'))
        if b > r:
            pieces.append((r, b, s0, s1, 'log'))
    return g['top'], pieces


def air_factor(m, R, c, dt):
    """exp(-m (2R + c dt)): the least share of a chord's first-half energy its second half carries,
    with SPPS applying air absorption once per whole step (CalculationCore.cpp:50-58)."""
    return math.exp(-m * (2 * R + c * dt))


def verdict_err(q, value, truth):
    if q == 'edt':
        return abs(value / truth - 1) / EDT_LIMIT
    return abs(value - truth) / C.ts_limit(truth)
