"""EDT-simplify, minimal candidate.

Upstream's own algorithm (backward/Schroeder integration -> OLS line on the dB
curve -> extrapolate to a 60 dB drop), unchanged, with exactly one structural
fix and one honest addition:

  FIX   the fit is anchored at the direct-sound arrival (t_arrival, known
        exactly from source/receiver geometry -- SPPS never has to *detect*
        it, unlike a measured impulse response) instead of at emission t=0.
        This is issue I1, upstream's actual bug (READING.md Sec.1.3/6/7:
        0.3%-20%+ overestimate, growing with r/EDT), and every surveyed
        standard/tool anchors here (ISO 3382-1, ODEON, CATT, Treble).
  ADD   the OLS fit's own 95% confidence interval on the slope, propagated to
        EDT. This is the "simplest honest source" for the range Burhan wants
        always shown (I7): no worst-case model, no safety factor, just the
        scatter of the recorded points around the fitted line.

Everything else upstream already does (a plain least-squares line over a
fixed dB window) is kept. Two cheap gates are added, both lifted directly
from a standard tool's *stated, shipped* practice, not invented here:
  - CATT's own minimum acceptable regression coefficient, r^2 >= 0.70
    (CATT-Acoustic release notes, quoted in PRACTICE.md Sec.3.2).
  - CATT's own minimum-decay-range rule: the recorded curve must continue at
    least as far again below the bottom of the fit window (PRACTICE.md
    Sec.3.2), else the result is flagged, not silently trusted.

Deliberately NOT modelled (see DESIGN.md for the measured/researched reason):
  I2 direct-sound weighting (causality already anchors it -- PRACTICE.md 5),
  I3 sub-bin timing (accepted as a resolution choice, not an error term --
     PRACTICE.md 5, SPPS's own docs), I4 unrecorded tail (EDT's 10 dB window
     ends far too early in a normal run for this to bite -- see DESIGN.md),
  I5 particle noise (Schroeder integration already averages it -- Schroeder
     1965, PRACTICE.md 2.1), and any multi-source/premise machinery (never
     fired once in ~94,000 evaluated rows -- INVENTORY.md part (b)).

No global state, no files, deterministic, numpy only.
"""

import numpy as np

Z95 = 1.959963984540054  # two-sided 95% normal quantile


def _ols(t, y):
    """Ordinary least squares y = a*t + b, plus the slope's own std. error
    and R^2. Returns None if the points are degenerate (all at one time)."""
    n = len(t)
    tbar = t.mean()
    ybar = y.mean()
    sxx = float(np.sum((t - tbar) ** 2))
    if sxx <= 0.0:
        return None
    sxy = float(np.sum((t - tbar) * (y - ybar)))
    a = sxy / sxx
    b = ybar - a * tbar
    resid = y - (a * t + b)
    sse = float(np.sum(resid ** 2))
    sst = float(np.sum((y - ybar) ** 2))
    dof = n - 2
    se_a = float(np.sqrt(sse / dof / sxx)) if dof > 0 else float('inf')
    r2 = 1.0 - sse / sst if sst > 0 else 1.0
    return a, se_a, r2, n


def _fit_decay(bins, dt, t_arrival, from_db, to_db):
    """Upstream's own GetTimeRange -> ComputeLinearRegression chain
    (projet_calculation.cpp:143-219), re-anchored at t_arrival instead of
    row_db[0]/t=0 (the I1 fix), plus CATT's r^2 and decay-range checks
    (upstream has neither -- READING.md Sec.4)."""
    n = len(bins)
    if n < 4 or dt <= 0:
        return {'reason': 'insufficient_data'}
    k0 = int(t_arrival // dt)
    if k0 < 0:
        k0 = 0
    if k0 >= n:
        return {'reason': 'arrival_after_run_end'}

    tail = np.asarray(bins[k0:], dtype=np.float64)
    s = np.cumsum(tail[::-1])[::-1]  # s[j] = sum(tail[j:]) = backward Schroeder sum
    if s[0] <= 0.0:
        return {'reason': 'no_energy_at_arrival'}
    level = 10.0 * np.log10(np.maximum(s, 1e-300) / s[0])  # 0 dB at arrival, ISO-style
    t_rel = (np.arange(k0, n) + 1) * dt - t_arrival  # bin-END time, upstream's own label
    # (reportmanager.cpp:526), just re-origined at the true arrival instant.

    j0 = 0
    if from_db > 0:
        j0 = None
        for j in range(len(level)):
            if level[j] <= -from_db:
                j0 = j
                break
        if j0 is None:
            return {'reason': 'run_too_short'}

    k1 = None
    for j in range(j0, len(level)):
        if level[j] <= -to_db:
            k1 = j
            break
    if k1 is None:
        return {'reason': 'run_too_short'}

    near_end = True  # CATT: is there >= to_db of further recorded decay past k1?
    for j in range(k1, len(level)):
        if level[j] <= -2 * to_db:
            near_end = False
            break

    if k1 - j0 + 1 < 3:
        return {'reason': 'insufficient_points'}
    fit = _ols(t_rel[j0:k1 + 1], level[j0:k1 + 1])
    if fit is None:
        return {'reason': 'insufficient_points'}
    a, se_a, r2, n_pts = fit
    return {'a': a, 'se_a': se_a, 'r2': r2, 'n_pts': n_pts, 'near_end': near_end}


def _param_from_fit(fit, target_db):
    """Shared packaging for a Compute_TR_Param-style result: point value,
    a 95%-CI-derived range (I7), status and reason."""
    if 'a' not in fit:
        return None, None, None, 'refused', fit['reason']
    a, se_a, r2 = fit['a'], fit['se_a'], fit['r2']
    if not (a < -1e-9):
        return None, None, None, 'refused', 'not_decaying'
    if r2 < 0.70:
        return None, None, None, 'refused', 'not_linear'

    value = -target_db / a
    a_lo = a - Z95 * se_a          # more negative -> faster decay -> the lower bound
    a_hi = min(a + Z95 * se_a, a * 0.02)  # capped so it can't cross zero / blow up
    slope_uncertain = (a + Z95 * se_a) >= a * 0.02
    value_lo = -target_db / a_lo
    value_hi = -target_db / a_hi

    if slope_uncertain:
        status, reason = 'wide', 'slope_uncertain'
    elif fit['near_end']:
        status, reason = 'wide', 'near_run_end'
    elif r2 < 0.95:
        status, reason = 'wide', 'low_r2'
    else:
        status, reason = 'ok', 'ok'
    return value, value_lo, value_hi, status, reason


def _energy_param(bins, dt, t_arrival, window_s):
    """D50/C50/C80-style direct sum from the (exactly known) arrival, with a
    bin-snap lo/hi range -- the honest I3 uncertainty (which bin the window
    edge falls in), not a modelled one."""
    n = len(bins)
    if n < 2 or dt <= 0:
        return None, None, None, 'refused', 'insufficient_data'
    k0 = int(t_arrival // dt)
    if k0 < 0 or k0 >= n:
        return None, None, None, 'refused', 'arrival_after_run_end'
    total = float(np.sum(bins[k0:]))
    if total <= 0.0:
        return None, None, None, 'refused', 'no_energy_at_arrival'
    w = window_s / dt
    edges = sorted({max(0, min(n - k0, int(np.floor(w)))),
                    max(0, min(n - k0, int(np.ceil(w))))})
    earlys = [float(np.sum(bins[k0:k0 + e])) for e in edges]
    return total, min(earlys), max(earlys), 'ok', 'ok'


def analyse(bins, dt, t_arrival, meta=None):
    bins = np.asarray(bins, dtype=np.float64)
    out = {}

    edt_fit = _fit_decay(bins, dt, t_arrival, from_db=0.0, to_db=10.0)
    edt, edt_lo, edt_hi, status, reason = _param_from_fit(edt_fit, target_db=60.0)
    out.update(edt=edt, edt_lo=edt_lo, edt_hi=edt_hi, status=status, reason=reason)

    # Ts: energy-time centroid from arrival -- a sum, not a fit, so it is not
    # subject to I1/I6 the way EDT is; deterministic, no bin-edge ambiguity.
    n = len(bins)
    k0 = int(t_arrival // dt) if dt > 0 else n
    if 0 <= k0 < n and dt > 0:
        tail = bins[k0:]
        total = float(np.sum(tail))
        if total > 0.0:
            t_rel = (np.arange(len(tail)) + 0.5) * dt
            ts = float(np.sum(t_rel * tail) / total)
            out.update(ts=ts, ts_lo=ts, ts_hi=ts, ts_status='ok', ts_reason='ok')
        else:
            out.update(ts=None, ts_lo=None, ts_hi=None, ts_status='refused',
                       ts_reason='no_energy_at_arrival')
    else:
        out.update(ts=None, ts_lo=None, ts_hi=None, ts_status='refused',
                   ts_reason='arrival_after_run_end')

    total, e50_lo, e50_hi, tstat, treason = _energy_param(bins, dt, t_arrival, 0.050)
    if total is not None:
        late_hi = total - e50_lo
        late_lo = total - e50_hi
        d50 = 100.0 * 0.5 * (e50_lo + e50_hi) / total
        out.update(d50=d50, d50_lo=100.0 * e50_lo / total, d50_hi=100.0 * e50_hi / total,
                   d50_status=tstat, d50_reason=treason)
        if late_lo > 0:
            c50 = 10.0 * np.log10((0.5 * (e50_lo + e50_hi)) / (0.5 * (late_lo + late_hi)))
            c50_lo = 10.0 * np.log10(e50_lo / late_hi) if late_hi > 0 else None
            c50_hi = 10.0 * np.log10(e50_hi / late_lo)
            out.update(c50=c50, c50_lo=c50_lo, c50_hi=c50_hi, c50_status=tstat, c50_reason=treason)
        else:
            out.update(c50=None, c50_lo=None, c50_hi=None, c50_status='refused',
                       c50_reason='no_late_energy')
    else:
        out.update(d50=None, d50_lo=None, d50_hi=None, d50_status=tstat, d50_reason=treason,
                   c50=None, c50_lo=None, c50_hi=None, c50_status=tstat, c50_reason=treason)

    total8, e80_lo, e80_hi, t8stat, t8reason = _energy_param(bins, dt, t_arrival, 0.080)
    if total8 is not None:
        late_hi = total8 - e80_lo
        late_lo = total8 - e80_hi
        if late_lo > 0:
            c80 = 10.0 * np.log10((0.5 * (e80_lo + e80_hi)) / (0.5 * (late_lo + late_hi)))
            c80_lo = 10.0 * np.log10(e80_lo / late_hi) if late_hi > 0 else None
            c80_hi = 10.0 * np.log10(e80_hi / late_lo)
            out.update(c80=c80, c80_lo=c80_lo, c80_hi=c80_hi, c80_status=t8stat, c80_reason=t8reason)
        else:
            out.update(c80=None, c80_lo=None, c80_hi=None, c80_status='refused',
                       c80_reason='no_late_energy')
    else:
        out.update(c80=None, c80_lo=None, c80_hi=None, c80_status=t8stat, c80_reason=t8reason)

    return out
