"""EDT for SPPS receiver histograms, v2.1: v1 (the graft in edt-simplify/FINAL.md) plus the fixes of
m8b/2026-09-27-edt-heldout/DIAGNOSIS.md, frozen for PREREG-2 (Z = 2.5, Burhan 2026-10-02 00:24).

F1  receiver_too_large: refuse when the ball's half-width h = R/c exceeds K_BALL of the fitted EDT. The fit starts
    at t_arrival + h, so it never sees the first h of the decay; in a dead room that is a shelf worth several
    percent of EDT. The method does not model the unseen shelf; it refuses and says why.
F2  counting noise with correlated bins: one particle crossing the ball is recorded across ~2h/dt+1 adjacent bins,
    so the second difference of single bins cancels part of the noise and w came out ~2x too small at 1 ms. w is now
    taken from bins summed in blocks two crossings long (B = ceil(2 * 2h/dt)), where neighbouring blocks share only the
    particles that straddle a boundary: hit-level w. The v1 end-point term 0.4343 sqrt(w/S10) is replaced by the first-order
    propagation of hit noise to the fitted slope (all bins, exact weights), which has no empirical constant.

v1 docstring follows.

LeanBand's shape (ISO 3382-1 0/-10 dB OLS on the Schroeder curve, 'ok' only when the shown range
is inside the 5 % JND) with the critique's origin fixes (../critique/fix_probe.py), a resolution
gate, a late-envelope tail gate and a counting-noise term in the range. Nothing else.
numpy only, pure function, EDT only.

bins[k] = energy recorded in [k dt, (k+1) dt).  t_arrival = d/c to the ball CENTRE (Night Mode's
arrival_s), or None (celerity gradient).  meta: optional 'half_width' = R/c (s); default R = 0.31 m.
"""
import math
import numpy as np

JND = 0.05          # ISO 3382-1 Annex A. status 'ok' iff the shown half-width is inside it
Z = 2.5             # Burhan, 2026-10-02 00:24 (PREREG-2 amendment 1)
MIN_POINTS = 8      # Schroeder samples between the ball's back and -10 dB (Lundeby: 3-10 per 10 dB)
TAIL_SHARE = 0.02   # refuse if the extrapolated unrecorded tail exceeds 2 % of S(t_-10dB)
K_BALL = 0.01       # v2/F1: refuse 'receiver_too_large' when h / EDT exceeds this
BLOCK_CROSSINGS = 2 # v2/F2: noise blocks span this many ball crossings (2h/dt bins each)
MIN_BLOCKS = 8      # v2/F2: at least this many blocks in the noise window
W_MARGIN = 0.75     # v2.1/F3: the w window runs from the first fitted sample to this fraction of the fit length past -10 dB
HW_FLOOR = 0.005    # never claim tighter than 0.5 % (t1: ball+gap offsets of 0.21-0.26 % otherwise fall outside)
C = 343.2


def _refuse(why):
    return dict(edt=None, edt_lo=None, edt_hi=None, status='refused', reason=why)


def count_weight(d, B):
    """Energy per hit w (var of a block sum = w * its mean) from second differences of B-bin block sums of d.
    The decay per block, x, is read from the window itself, so r = c_j - (c_{j-1} e^-x + c_{j+1} e^x) / 2 is zero
    for a pure exponential and var(r) = w E[c_j] (1 + cosh(x) / 2). Early reflections are outliers, not counting
    noise: w is the median of per-block r^2 / E[c] (chi2(1) median 0.4549). None when fewer than 3 blocks fit or no
    block has energy: the noise is then unknown, never zero."""
    nb = len(d) // B
    if nb < 3:
        return None
    c = d[:nb * B].reshape(nb, B).sum(axis=1)
    m = len(c) // 2
    e1, e2 = float(c[:m].sum()), float(c[m:2 * m].sum())
    x = math.log(e1 / e2) / m if e1 > e2 > 0 else 0.0
    r = c[1:-1] - 0.5 * (c[:-2] * math.exp(-x) + c[2:] * math.exp(x))
    cm = c[1:-1]
    ok = cm > 0
    if not ok.any():
        return None
    return float(np.median(r[ok] ** 2 / cm[ok]) / 0.4549 / (1.0 + 0.5 * math.cosh(x)))


def analyse(bins, dt, t_arrival, meta=None):
    meta = meta or {}
    b = np.clip(np.asarray(bins, float).ravel(), 0.0, None)
    n = len(b)
    if n < 4 or dt <= 0 or b.sum() <= 0:
        return _refuse('no_energy')
    h = meta.get('half_width')
    h = 0.31 / C if h is None else float(h)
    S = np.cumsum(b[::-1])[::-1]                    # S[i] = energy after i*dt, exact at bin starts

    # I1/I2: 0 dB at the FRONT of the ball's direct sound; first fitted sample after its BACK
    if t_arrival is None:                           # no geometric arrival: first recorded energy
        k0 = int(np.nonzero(b > 0)[0][0])
        t_start = (k0 + 1) * dt
    else:
        k0 = max(int(math.floor((t_arrival - h) / dt)), 0)
        t_start = t_arrival + h
        if b[k0:].sum() <= 0:
            return _refuse('no_energy_after_arrival')
        k_on = k0 + int(np.nonzero(b[k0:] > 0)[0][0])
        if k_on * dt > t_start + dt:                # blocked path: 0 dB at the first arrival
            k0, t_start = k_on, (k_on + 1) * dt
    L = 10.0 * np.log10(np.maximum(S[k0:], 1e-300) / S[k0])
    T = np.arange(k0, n) * dt
    below = np.nonzero(L <= -10.0)[0]
    if len(below) == 0:
        return _refuse('run_too_short')
    i10 = int(below[0])
    sel = np.nonzero((T >= t_start - 1e-12) & (np.arange(len(L)) <= i10))[0]
    if len(sel) < MIN_POINTS:                       # I3/I6: direct-only, or dt too coarse for it
        return _refuse('direct_only' if T[i10] <= t_start + dt else 'step_too_coarse')

    t, y = T[sel], L[sel]
    tb = t.mean()
    sxx = float(((t - tb) ** 2).sum())
    a = float(((t - tb) * (y - y.mean())).sum() / sxx)
    if a >= 0:
        return _refuse('not_decaying')
    resid = y - (y.mean() + a * (t - tb))
    se = math.sqrt(float((resid ** 2).sum()) / (len(t) - 2) / sxx)

    # I4: unrecorded tail from the LAST recorded envelope (Hirata), never from the early slope
    m = max(2, n // 5)                              # last 20 % vs the 20 % before it
    E1, E2 = float(b[n - 2 * m:n - m].sum()), float(b[n - m:].sum())
    U = 0.0 if E2 == 0 else (E2 * E2 / (E1 - E2) if E1 > E2 else math.inf)
    S10 = float(S[k0 + i10])
    if S10 <= 0:                                    # nothing recorded after -10 dB: too few hits
        return _refuse('too_few_particles')
    if U > TAIL_SHARE * S10:
        return _refuse('run_too_short' if math.isfinite(U) else 'not_decaying_at_run_end')

    # I5: counting noise from the bin scatter after -10 dB (diffuse part; early reflections read as noise, a safe side).
    # Compound Poisson, hit-level w; the slope variance is propagated exactly to first order below.
    B = max(1, int(math.ceil(BLOCK_CROSSINGS * 2.0 * h / dt - 1e-9)))     # block length in bins
    # v2.1/F3: w is estimated where the fit lives (first fitted sample to a margin past -10 dB), not from the far tail:
    # in Energetic mode the energy per hit falls with time, so a tail w is too small for the fit window (Random: w constant, unchanged).
    i0 = k0 + int(sel[0])
    j_end = min(n, max(k0 + i10 + int(math.ceil(W_MARGIN * len(sel))), i0 + MIN_BLOCKS * B))
    w = count_weight(b[i0:j_end], B)
    # first-order slope variance from hit noise: d slope / d b_j = g_j (the 0 dB normaliser cancels, since the centred times sum to 0)
    g = np.cumsum((t - tb) / sxx / S[k0 + sel])
    g = np.concatenate([g, np.full(n - i0 - len(g), g[-1])])
    sd = 0.0 if w is None else 4.343 * math.sqrt(w * float((b[i0:] * g * g).sum())) / -a

    if h > K_BALL * (-60.0 / a):                    # v2/F1: the unseen first h of the decay is a shelf the fit cannot see
        return _refuse('receiver_too_large')
    q = max(Z * math.sqrt((se / a) ** 2 + sd ** 2), HW_FLOOR)   # relative slope half-width: fit curvature + noise
    edt = -60.0 / a
    lo = edt / (1.0 + q)
    hi = edt / (1.0 - q) if q < 0.9 else 10.0 * edt
    hw = (hi - lo) / (2.0 * edt)
    ok = hw <= JND and w is not None                # unknown noise is never 'ok'
    return dict(edt=edt, edt_lo=lo, edt_hi=hi, status='ok' if ok else 'wide',
                reason='hw=%.3f;fit=%.3f;noise=%s;tail=%.1e;n=%d' % (hw, se / -a, 'unknown' if w is None else '%.3f' % sd, U / S10, len(t)))
