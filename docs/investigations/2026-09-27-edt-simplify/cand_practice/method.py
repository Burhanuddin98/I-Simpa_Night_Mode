"""
EDT (and, simply, Ts/C50/C80/D50) from an SPPS receiver energy histogram.

Built from what the field actually does (see DESIGN.md / practice/PRACTICE.md),
not a model-free worst-case bound:

  - ISO 3382-1: EDT is a straight-line OLS fit of the backward-integrated
    (Schroeder) energy curve, in dB, from 0 dB down to -10 dB, extrapolated to
    a 60 dB-equivalent time via -60/slope.
  - The 0 dB reference is the direct-sound ARRIVAL, not the emission instant
    (I1). For a simulated histogram this is not a detection problem: the
    arrival time is handed in (distance/c), so it is a bin index, not a fit.
  - Schroeder's backward integration is itself the field's answer to receiver
    shot noise (I5) -- it is a exact ensemble average, so no separate noise
    model is layered on top of it. What noise remains shows up as scatter
    around the regression line, which the fit's own standard error already
    measures -- that standard error becomes this method's uncertainty range.
  - A bin's energy could have landed anywhere inside [k*dt, (k+1)*dt) (I3).
    Using the bin CENTER as its time (upstream uses the bin's END) removes
    most of this; the residual is expressed as a small, explicit relative
    widening of the reported range, not a per-bin worst-case term.
  - Truncation/tail energy (I4) is handled the way Lundeby/Hirata handle it:
    a finite recording is always missing whatever energy would have arrived
    after it stopped, and that missing energy deflates the 0 dB reference
    every bin is measured against, biasing the fitted slope shallow-to-steep
    in a way the raw curve's own tail cannot be used to detect (the last
    recorded bin looks small because it IS the last one, not because the
    true decay is actually finished -- see DESIGN.md for the worked case).
    So, exactly as Lundeby 1995 does, the fitted line's own extrapolation to
    the recording's true end estimates how much is still missing, that
    estimate is added back before refitting, and this repeats a fixed few
    times until it stops moving. If it does not settle to a small correction,
    that is "run_too_short" -- refuse rather than trust an unbounded guess.
  - Fit-quality gate (I6): CATT-Acoustic's own r^2 >= 0.70 acceptance floor
    (lowered from 0.95 because "0 to -10 dB decay is seldom linear" -- their
    words). Below that, refuse rather than report a number with no warning.
  - I7: a range is always returned, per decision-log row 9, built from the
    regression's own statistics -- not a scaled safety factor.

No global state, no file I/O, deterministic, numpy only.
"""

import numpy as np

# ---- constants, each tied to the standard/tool it comes from ----
EDT_DB_DROP = 10.0      # ISO 3382-1: EDT window is 0 dB -> -10 dB
R2_GOOD = 0.95          # tight range at/above this
R2_ACCEPT = 0.70        # CATT-Acoustic v9.1 release notes: EDT accepted down to r^2 = 0.70
MIN_FIT_POINTS = 4      # need at least this many bins inside the fit window
TAIL_ITERATIONS = 6     # Lundeby 1995: iterate "a few times" until the estimate converges
TAIL_REFUSE = 0.05      # if, even after converging, >5% of the 0 dB reference is still
                        # estimated missing, the recording is too short to correct for
Z = 2.0                 # ~95% two-sided normal multiplier on the OLS slope's standard error
D50_MS = 0.050          # ISO 3382-1 D50 / C50 split
C80_MS = 0.080          # ISO 3382-1 C80 split
TAIL_FRAC_WARN = 0.005  # >0.5% of post-arrival energy still arriving in the last decile -> flag


def _ols(t, y):
    """y = a*t + b by ordinary least squares; also r^2 and the slope's std. error."""
    n = t.size
    if n < 2:
        return 0.0, 0.0, 0.0, float("inf")
    tbar, ybar = t.mean(), y.mean()
    dtc, dyc = t - tbar, y - ybar
    sxx = float(np.dot(dtc, dtc))
    if sxx <= 0.0:
        return 0.0, ybar, 0.0, float("inf")
    a = float(np.dot(dtc, dyc) / sxx)
    b = ybar - a * tbar
    resid = y - (a * t + b)
    sse = float(np.dot(resid, resid))
    sst = float(np.dot(dyc, dyc))
    r2 = 1.0 - sse / sst if sst > 0 else 0.0
    dof = n - 2
    se_a = float(np.sqrt(sse / dof / sxx)) if dof > 0 else float("inf")
    return a, b, r2, se_a


def _schroeder_db(bins, k0):
    """
    Backward cumulative energy from the direct-sound arrival bin k0 onward,
    in dB relative to 0 dB AT ARRIVAL (I1). Bins before k0 are dropped, not
    fit against -- they are pre-arrival silence. Monotonically non-increasing
    by construction (energies are non-negative), which is what lets the -10 dB
    crossing be found with a single scan below.
    """
    n = bins.size
    if k0 >= n:
        return np.array([]), 0.0
    tail = bins[k0:].astype(np.float64)
    cum = np.cumsum(tail[::-1])[::-1]
    e0 = float(cum[0])
    if e0 <= 0.0:
        return np.array([]), 0.0
    with np.errstate(divide="ignore"):
        db = 10.0 * np.log10(np.clip(cum / e0, 1e-300, None))
    return db, e0


def _refuse(reason):
    return {"edt": None, "edt_lo": None, "edt_hi": None, "status": "refused", "reason": reason}


def _fit_edt(t_rel, db, dt):
    below = np.nonzero(db < -EDT_DB_DROP)[0]
    if below.size == 0:
        return _refuse("run_too_short")  # curve never dropped 10 dB before the recording ended
    idx_cross = int(below[0])
    npts = idx_cross + 1
    if npts < MIN_FIT_POINTS:
        return _refuse("insufficient_samples")

    # Lundeby/Hirata tail correction, fixed-point iteration (see module
    # docstring): cum_norm is the backward-integrated curve normalised so
    # cum_norm[0] == 1 (0 dB at arrival). Each round, the current fit's own
    # line is extrapolated to the recording's true end to estimate what
    # fraction of that 0 dB reference is still missing (I4), that estimate
    # is added back to every point, and the line is refit. A recording that
    # was long enough needs almost no correction and converges immediately;
    # one that was not converges to a large, and then refused, correction.
    cum_norm = 10.0 ** (db / 10.0)
    a = b = r2 = se_a = missing_frac = None
    R = 0.0
    for _ in range(TAIL_ITERATIONS):
        cum_it = cum_norm + R
        db_it = 10.0 * np.log10(cum_it[:npts] / cum_it[0])
        a, b, r2, se_a = _ols(t_rel[:npts], db_it)
        if a >= 0.0:
            return _refuse("not_decaying")
        missing_frac = 10.0 ** ((a * t_rel[-1] + b) / 10.0)
        R = missing_frac * cum_it[0]

    if not np.isfinite(missing_frac) or missing_frac > TAIL_REFUSE:
        return _refuse("run_too_short")
    if r2 < R2_ACCEPT:
        return _refuse(f"poor_fit (r2={r2:.2f} < {R2_ACCEPT})")

    # The OLS standard error above treats every point as independent, but
    # points on a backward-integrated curve are not: neighbouring bins share
    # almost all of the same underlying particle hits, so se_a alone can be
    # over-confident, especially at a fine dt (many correlated points). As a
    # simple, independent check, the early and late halves of the same
    # window are each fit on their own -- how much THEY disagree is immune
    # to that correlation, since the two halves share no bins. The wider of
    # the two is what is reported.
    mid = npts // 2
    half_spread = 0.0
    if mid >= 2 and (npts - mid) >= 2:
        a1, _b1, _r1, _s1 = _ols(t_rel[:mid], db_it[:mid])
        a2, _b2, _r2, _s2 = _ols(t_rel[mid:npts], db_it[mid:npts])
        half_spread = abs(a1 - a2) / 2.0
    delta_a = max(Z * se_a, half_spread)

    a_shallow = a + delta_a   # closer to zero -> longer EDT
    a_steep = a - delta_a     # more negative -> shorter EDT
    if a_shallow >= 0.0:
        return _refuse("unreliable_slope")  # the CI itself reaches "not decaying"

    edt = -60.0 / a
    edt_hi_noise = -60.0 / a_shallow
    edt_lo_noise = -60.0 / a_steep

    window_dur = t_rel[npts - 1] - t_rel[0]
    if window_dur <= 0.0:
        window_dur = dt
    dt_rel = 0.5 * dt / window_dur  # I3: bin-center label is uncertain by at most dt/2

    edt_lo = edt_lo_noise * (1.0 - dt_rel)
    edt_hi = edt_hi_noise * (1.0 + dt_rel)
    if not (np.isfinite(edt_lo) and np.isfinite(edt_hi)) or edt_lo <= 0.0:
        return _refuse("unreliable_slope")

    status = "ok" if r2 >= R2_GOOD else "wide"
    reason = "" if status == "ok" else f"r2={r2:.2f} below {R2_GOOD}"
    return {"edt": edt, "edt_lo": edt_lo, "edt_hi": edt_hi, "status": status, "reason": reason}


def _energy_params(e, t_rel):
    """
    Ts/D50/C50/C80, all referenced to the direct-sound arrival like EDT.
    Simple by design (optional per the task): a single tail-fraction check
    stands in for I4/I5/I3 here, rather than the full OLS treatment EDT gets
    -- see DESIGN.md's "gives up" section.
    """
    out = {}
    etot = float(e.sum())
    if etot <= 0.0:
        return out

    n = e.size
    tail_frac = float(e[int(n * 0.9):].sum()) / etot
    wide = tail_frac > TAIL_FRAC_WARN
    status = "wide" if wide else "ok"
    reason = f"{tail_frac * 100:.2f}% of energy still arriving at run end" if wide else ""
    w = tail_frac  # relative widening, proportional to the unresolved tail's own share

    ts = float(np.dot(t_rel, e) / etot)
    out.update(ts=ts, ts_lo=ts * (1 - w), ts_hi=ts * (1 + w),
               ts_status=status, ts_reason=reason)

    early50 = float(e[t_rel <= D50_MS].sum())
    d50 = early50 / etot
    out.update(d50=d50, d50_lo=max(0.0, d50 * (1 - w)), d50_hi=min(1.0, d50 * (1 + w)),
               d50_status=status, d50_reason=reason)

    for ms, tag in ((D50_MS, "c50"), (C80_MS, "c80")):
        early = float(e[t_rel <= ms].sum())
        late = etot - early
        if early <= 0.0 or late <= 0.0:
            out.update({tag: None, tag + "_lo": None, tag + "_hi": None,
                        tag + "_status": "refused", tag + "_reason": "degenerate_split"})
            continue
        c = 10.0 * np.log10(early / late)
        half = 10.0 * w + 0.05  # rough dB proxy for the same unresolved-tail share
        out.update({tag: c, tag + "_lo": c - half, tag + "_hi": c + half,
                    tag + "_status": status, tag + "_reason": reason})
    return out


def analyse(bins, dt, t_arrival, meta=None):
    """
    bins: numpy float64 array, receiver energy per time bin from t=0 (emission).
    dt: bin width, s. t_arrival: direct-sound arrival time, s. meta: dict,
    unused here (see DESIGN.md) -- kept in the signature for interface parity
    and future use, and ignored gracefully if absent.

    Returns edt/edt_lo/edt_hi/status/reason, plus the same for ts/d50/c50/c80.
    """
    bins = np.asarray(bins, dtype=np.float64)
    n = bins.size
    if n == 0 or dt <= 0.0:
        return _refuse("no_data")

    k0 = int(round(t_arrival / dt))
    k0 = max(k0, 0)
    if k0 >= n:
        return _refuse("arrival_after_run_end")

    db, e0 = _schroeder_db(bins, k0)
    if e0 <= 0.0 or db.size == 0:
        return _refuse("no_energy_at_arrival")

    t_rel = (np.arange(k0, n) + 0.5) * dt - t_arrival  # bin-center time since arrival (I3)

    out = _fit_edt(t_rel, db, dt)
    out.update(_energy_params(bins[k0:], t_rel))
    return out
