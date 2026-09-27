"""EDT for SPPS receiver histograms: the graft recommended in ../FINAL.md.

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
Z = 2.0             # ~95 %
MIN_POINTS = 8      # Schroeder samples between the ball's back and -10 dB (Lundeby: 3-10 per 10 dB)
TAIL_SHARE = 0.02   # refuse if the extrapolated unrecorded tail exceeds 2 % of S(t_-10dB)
HW_FLOOR = 0.005    # never claim tighter than 0.5 % (t1: ball+gap offsets of 0.21-0.26 % otherwise fall outside)
NOISE_TERM = True   # ablation switch only
C = 343.2


def _refuse(why):
    return dict(edt=None, edt_lo=None, edt_hi=None, status='refused', reason=why)


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

    # I5: counting noise of S(t10) from bin scatter after -10 dB (diffuse part; early reflections
    # are structure, not noise). Compound Poisson: var(b) = w E[b]; N_eff = S10 / w hits.
    d = b[k0 + i10: k0 + i10 + max(2 * (i10 + 1), 10)]
    r = d[1:-1] - 0.5 * (d[:-2] + d[2:])
    w = float((r ** 2).sum() / (1.5 * d[1:-1].sum())) if len(d) > 2 and d[1:-1].sum() > 0 else 0.0
    sd = 0.4343 * math.sqrt(w / S10) if NOISE_TERM else 0.0

    q = max(Z * math.sqrt((se / a) ** 2 + sd ** 2), HW_FLOOR)   # relative slope half-width: fit curvature + noise
    edt = -60.0 / a
    lo = edt / (1.0 + q)
    hi = edt / (1.0 - q) if q < 0.9 else 10.0 * edt
    hw = (hi - lo) / (2.0 * edt)
    return dict(edt=edt, edt_lo=lo, edt_hi=hi, status='ok' if hw <= JND else 'wide',
                reason='hw=%.3f;fit=%.3f;noise=%.3f;tail=%.1e;n=%d' % (hw, se / -a, sd, U / S10, len(t)))
