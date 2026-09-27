"""LeanBand: EDT (+ Ts/C50/C80/D50) for I-Simpa SPPS receiver histograms.

Design in one paragraph (full version in DESIGN.md): anchor the Schroeder
curve's 0 dB reference at the *given* direct-sound arrival time (not at
emission, and not by detecting it -- geometry already gives it to us
exactly), fit an ordinary least-squares line to the 0..-10 dB window, and
report EDT = -60/slope. The window's far edge must be an *observed*, two-bin
-10 dB crossing -- if the recorded histogram never gets there, we refuse
outright ("run_too_short") instead of guessing about a tail we can't see.
The range shown to the user is the regression's own 95%-ish confidence
interval on the fitted slope: real per-run particle noise widens it, a poor
fit widens it, a clean run gives a tight one -- one mechanism standing in
for what used to be a separate noise model, a separate quality gate and a
separate tail-safety-factor. See DESIGN.md for how this maps to I1-I7 and
what it gives up.

Interface: analyse(bins, dt, t_arrival, meta) -> dict. Pure function, numpy
only, no global state, no files.
"""

from __future__ import annotations

import numpy as np

# ---------------------------------------------------------------------------
# Tunables, each with its source (see DESIGN.md section "why these numbers")
# ---------------------------------------------------------------------------

#: ISO 3382-1 Annex A "just noticeable difference" for EDT/T -- borrowed from
#: T's JND, never independently validated for EDT itself (the one dedicated
#: study, Del Solar Dorrego & Vigeant, JASA 2022, measured EDT's real JND
#: near 18%). We use the tighter, standards-quoted 5% as the ok/wide line --
#: one order of magnitude looser than the previous design's 1/10-JND (0.5%)
#: threshold, which is the single biggest reason that design refused 32.7%
#: of real EDTs.
JND_EDT = 0.05

#: Half-width multiplier on the OLS slope's standard error, i.e. a plain
#: "two-sigma" Gaussian interval on the fitted slope (no scipy dependency).
#: Every other tolerance here is already an engineering choice, not a
#: certified bound, so a round number is honest rather than falsely precise.
CI_Z = 2.0

#: If the confidence interval cannot rule out "not decaying" (slope's upper
#: bound is at or past zero), report edt_hi capped at this multiple of the
#: point estimate rather than +inf, always paired with status='wide'.
CI_CAP = 10.0

#: Minimum number of histogram bins inside the 0..-10 dB window for the
#: regression's own standard error to mean anything. Anything shorter
#: (e.g. a direct-only receiver) is refused, not fitted.
MIN_FIT_POINTS = 4

#: A -10 dB crossing must hold for this many consecutive bins before it ends
#: the fit window -- guards against a single noisy dip (I5) closing the
#: window on a false floor.
PERSIST_BINS = 2

#: Bins to look past a nominally-zero-energy arrival bin, to absorb ordinary
#: bin-quantisation slack (I3) in exactly when energy is first recorded.
ARRIVAL_SEARCH = 2

_TINY = 1e-300


def _direct_index(t_arrival: float, dt: float, n: int):
    if t_arrival is None or not np.isfinite(t_arrival) or t_arrival < 0 or dt <= 0:
        return None
    k0 = int(t_arrival // dt)
    return k0 if k0 < n else None


def _schroeder_rel_db(bins: np.ndarray, k0: int):
    """Backward-integrated (Schroeder) tail energy, in dB relative to the
    tail energy at the direct-sound arrival bin (I1: 0 dB is the arrival,
    not emission; I2: the direct bin is simply the first term summed in,
    same estimator as any reflection -- causality already anchors it)."""
    n = len(bins)
    tail = np.cumsum(bins[::-1])[::-1]  # tail[i] = sum(bins[i:])
    for shift in range(ARRIVAL_SEARCH + 1):
        idx = k0 + shift
        if idx < n and tail[idx] > 0:
            k0 = idx
            break
    else:
        return None, None
    e0 = tail[k0]
    rel_db = 10.0 * np.log10(np.maximum(tail[k0:], _TINY) / e0)
    return k0, rel_db


def _persistent_cross(rel_db: np.ndarray, thresh_db: float):
    """First index whose crossing is confirmed by PERSIST_BINS consecutive
    bins *all still inside the recorded array*. A crossing that can only be
    "confirmed" by running off the end of the histogram is not confirmed at
    all: the last bin of any finite array is the sum of just that one bin
    (nothing left to add), so it always reads several dB lower than the true
    curve at that instant would -- an array-truncation cliff, not decay. Only
    accepting a crossing with real recorded bins on both sides of it is what
    keeps that cliff from being mistaken for having reached -10 dB (this is
    what run_too_short guards against, replacing the old tail-safety-factor)."""
    n = len(rel_db)
    for i in range(n):
        if rel_db[i] <= thresh_db:
            end = i + PERSIST_BINS
            if end > n:
                continue  # not enough recorded bins left to confirm this
            if all(rel_db[j] <= thresh_db for j in range(i, end)):
                return i
    return None


def _ols(t: np.ndarray, y: np.ndarray):
    n = len(t)
    tbar, ybar = t.mean(), y.mean()
    dt_, dy = t - tbar, y - ybar
    sxx = float(np.dot(dt_, dt_))
    if sxx <= 0:
        return None
    sxy = float(np.dot(dt_, dy))
    a = sxy / sxx
    b = ybar - a * tbar
    resid = y - (a * t + b)
    sse = float(np.dot(resid, resid))
    sst = float(np.dot(dy, dy))
    r2 = 1.0 - sse / sst if sst > 0 else 1.0
    dof = max(n - 2, 1)
    se_a = float(np.sqrt(max(sse, 0.0) / dof / sxx))
    return a, se_a, r2, n


def _edt(a: float) -> float:
    return -60.0 / a


def _fit_edt(bins: np.ndarray, dt: float, t_arrival: float) -> dict:
    n = len(bins)
    if n == 0 or dt <= 0:
        return dict(edt=None, edt_lo=None, edt_hi=None, status='refused', reason='empty_histogram')

    k0 = _direct_index(t_arrival, dt, n)
    if k0 is None:
        return dict(edt=None, edt_lo=None, edt_hi=None, status='refused', reason='arrival_after_record')

    k0, rel_db = _schroeder_rel_db(bins, k0)
    if k0 is None:
        return dict(edt=None, edt_lo=None, edt_hi=None, status='refused', reason='no_energy_after_arrival')

    i_end = _persistent_cross(rel_db, -10.0)
    if i_end is None:
        return dict(edt=None, edt_lo=None, edt_hi=None, status='refused', reason='run_too_short')
    if i_end < MIN_FIT_POINTS - 1:
        return dict(edt=None, edt_lo=None, edt_hi=None, status='refused', reason='too_few_points')

    idx = np.arange(0, i_end + 1)
    t = (k0 + idx + 0.5) * dt  # bin-centre times (I3: centre, not bin-end label)
    y = rel_db[idx]

    fit = _ols(t, y)
    if fit is None:
        return dict(edt=None, edt_lo=None, edt_hi=None, status='refused', reason='degenerate_window')
    a, se_a, r2, npts = fit

    if a >= -1e-9:
        return dict(edt=None, edt_lo=None, edt_hi=None, status='refused', reason='not_decaying')

    # I4, "is the run long enough": CATT's rule -- the echogram must run at
    # least 10 dB past the fit range's bottom -- checked by *extrapolating
    # the fitted line* to where it would reach -20 dB, not by re-searching
    # the raw curve for a second crossing (the same end-of-array cliff that
    # makes the last bin read low can forge a second "crossing" too, right
    # next to the first). Measured: without this, a run truncated exactly
    # at its own -10 dB point still reports a value, r2=0.99, 27% short of
    # the truth; with it, that run and everything shorter is refused.
    if (n * dt) < t[0] + 20.0 / (-a):
        return dict(edt=None, edt_lo=None, edt_hi=None, status='refused', reason='run_too_short')

    edt = _edt(a)
    a_steep = a - CI_Z * se_a   # more negative -> shorter EDT
    a_shallow = a + CI_Z * se_a  # less negative -> longer EDT

    edt_lo = _edt(a_steep)  # a_steep is always < a < 0, always safe
    if a_shallow < -1e-12:
        edt_hi_raw = _edt(a_shallow)
    else:
        edt_hi_raw = float('inf')
    wide_open = edt_hi_raw >= edt * CI_CAP
    edt_hi = min(edt_hi_raw, edt * CI_CAP)

    rel_halfwidth = (edt_hi - edt_lo) / (2.0 * edt) if edt > 0 else float('inf')
    status = 'ok' if (not wide_open and rel_halfwidth <= JND_EDT and r2 >= 0.5) else 'wide'

    reason = f"r2={r2:.3f};n={npts};halfwidth_pct={rel_halfwidth * 100:.1f}"
    if wide_open:
        reason += ';ci_open_capped'

    return dict(edt=edt, edt_lo=edt_lo, edt_hi=edt_hi, status=status, reason=reason)


def _early_late(bins: np.ndarray, dt: float, k0: int, window_ms: float):
    n = len(bins)
    n_early = max(1, int(round((window_ms / 1000.0) / dt)))
    split = k0 + n_early
    if split >= n:
        return None
    early = float(bins[k0:split].sum())
    late = float(bins[split:].sum())
    if early <= 0:
        return None
    return early, late


def _add_cd(out: dict, bins: np.ndarray, dt: float, k0: int):
    r50 = _early_late(bins, dt, k0, 50.0)
    if r50 is None:
        for key in ('c50', 'd50'):
            out[key] = None
            out[f'{key}_lo'] = out[f'{key}_hi'] = None
            out[f'{key}_status'] = 'refused'
            out[f'{key}_reason'] = 'run_too_short_for_window'
    else:
        early, late = r50
        out['d50'] = 100.0 * early / (early + late)
        out['d50_lo'] = out['d50_hi'] = out['d50']
        out['d50_status'] = 'wide'
        out['d50_reason'] = 'point_estimate_no_ci'
        out['c50'] = 10.0 * np.log10(early / late) if late > 0 else float('inf')
        out['c50_lo'] = out['c50_hi'] = out['c50']
        out['c50_status'] = 'wide'
        out['c50_reason'] = 'point_estimate_no_ci' if late > 0 else 'no_late_energy_recorded'

    r80 = _early_late(bins, dt, k0, 80.0)
    if r80 is None:
        out['c80'] = None
        out['c80_lo'] = out['c80_hi'] = None
        out['c80_status'] = 'refused'
        out['c80_reason'] = 'run_too_short_for_window'
    else:
        early, late = r80
        out['c80'] = 10.0 * np.log10(early / late) if late > 0 else float('inf')
        out['c80_lo'] = out['c80_hi'] = out['c80']
        out['c80_status'] = 'wide'
        out['c80_reason'] = 'point_estimate_no_ci' if late > 0 else 'no_late_energy_recorded'


def _add_ts(out: dict, bins: np.ndarray, dt: float, k0: int):
    seg = bins[k0:]
    total = float(seg.sum())
    if total <= 0:
        out['ts'] = None
        out['ts_lo'] = out['ts_hi'] = None
        out['ts_status'] = 'refused'
        out['ts_reason'] = 'no_energy_after_arrival'
        return
    t_rel = (np.arange(len(seg)) + 0.5) * dt
    ts = float(np.dot(t_rel, seg) / total)
    out['ts'] = ts
    out['ts_lo'] = out['ts_hi'] = ts
    out['ts_status'] = 'wide'  # no tail correction applied -- see DESIGN.md
    out['ts_reason'] = 'point_estimate_no_tail_correction'


def analyse(bins, dt, t_arrival, meta=None) -> dict:
    """Compute EDT (required) and Ts/C50/C80/D50 (best-effort, optional).

    bins: 1-D array-like, energy per time bin from t=0 (emission), bin k
        covers [k*dt, (k+1)*dt).
    dt: bin width, seconds.
    t_arrival: direct-sound arrival time, seconds (distance / speed of sound).
    meta: optional dict; not required by this method (see DESIGN.md I2-I4 for
        why total_energy/trans_epsilon/particles_per_source/n_sources/
        air_rate/rho_c are all safely ignorable for EDT specifically).

    Returns a dict with edt/edt_lo/edt_hi/status/reason, plus, best-effort,
    ts/ts_lo/ts_hi/ts_status/ts_reason and the same for c50/c80/d50.
    """
    del meta  # accepted for interface compatibility; unused, see DESIGN.md

    bins = np.asarray(bins, dtype=np.float64).ravel()
    out = _fit_edt(bins, float(dt), float(t_arrival))

    n = len(bins)
    k0 = _direct_index(t_arrival, dt, n) if n and dt > 0 else None
    if k0 is not None:
        k0_e, _ = _schroeder_rel_db(bins, k0)
        if k0_e is not None:
            k0 = k0_e
    if k0 is None:
        for key in ('ts', 'c50', 'c80', 'd50'):
            out[key] = None
            out[f'{key}_lo'] = out[f'{key}_hi'] = None
            out[f'{key}_status'] = 'refused'
            out[f'{key}_reason'] = 'no_direct_arrival_in_record'
        return out

    _add_ts(out, bins, dt, k0)
    _add_cd(out, bins, dt, k0)
    return out
