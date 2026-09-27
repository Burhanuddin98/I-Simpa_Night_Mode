"""Independent synthetic generator + truth for the critique. numpy only, single process.

Energy response at a receiver of ball radius R, distance d (t_arr = d/c):
  direct : energy Ed spread over [t_arr - h, t_arr + h], h = R/c, with SPPS's ball weight
           (R^2 - (rho - D)^2)/rho (as skeptic-gf3/ism.py); h = 0 gives a point impulse.
  reverb : density sum_i A_i exp(-k_i (t - t_r)) for t >= t_r = t_arr + gap.
Histogram bin n = exact integral over [n dt, (n+1) dt), n < N = round(T_run/dt).
Truth: ISO 3382-1 EDT of the POINT-receiver response of infinite length (direct = vertical drop at
t_arr, zero weight, 'Definition A' as the evaluator's t1/ism truths use), computed by OLS on a
0.01 ms grid of the continuous Schroeder curve from t_arr+ to its -10 dB crossing.
"""
import math
import numpy as np

C = 343.2


def ball_atoms(t_arr, h, Ed, n=400):
    if h <= 0:
        return np.array([t_arr]), np.array([Ed])
    tau = t_arr - h + (np.arange(n) + 0.5) * (2 * h / n)
    D, R = C * t_arr, C * h
    rho = C * tau
    w = np.maximum(R * R - (rho - D) ** 2, 0.0) / np.maximum(rho, 1e-9)
    return tau, Ed * w / w.sum()


def rev_integral(t0, t1, t_r, A, k):
    """integral of sum A_i exp(-k_i (t - t_r)) over [t0, t1] clipped to t >= t_r (vectorised on t0,t1)."""
    lo = np.maximum(t0, t_r)
    hi = np.maximum(t1, lo)
    out = np.zeros_like(lo, dtype=float)
    for a, kk in zip(A, k):
        out += a / kk * (np.exp(-kk * (lo - t_r)) - np.exp(-kk * (hi - t_r)))
    return out


def histogram(dt, T_run, t_arr, h, Ed, gap, A, k, extra_atoms=None):
    N = int(round(T_run / dt))
    n = np.arange(N)
    b = rev_integral(n * dt, (n + 1) * dt, t_arr + gap, A, k)
    tau, e = ball_atoms(t_arr, h, Ed)
    if extra_atoms is not None:
        tau = np.concatenate([tau, extra_atoms[0]]); e = np.concatenate([e, extra_atoms[1]])
    idx = np.floor(tau / dt).astype(int)
    ok = (idx >= 0) & (idx < N)
    np.add.at(b, idx[ok], e[ok])
    return b


def truth_edt(t_arr, Ed, gap, A, k, extra_atoms=None, fine=1e-5, top_db=0.0, bottom_db=-10.0):
    """ISO EDT of the point-receiver response (direct = step at t_arr), infinite length.
    OLS on the fine-sampled Schroeder level for u in (0, u10], u = t - t_arr."""
    S_rev_total = sum(a / kk for a, kk in zip(A, k))
    S0 = Ed + S_rev_total + (0.0 if extra_atoms is None else float(np.sum(extra_atoms[1])))
    # find u10 by bisection on the level
    def S_after(u):   # energy strictly after t_arr + u (u > 0)
        t = t_arr + u
        s = 0.0
        for a, kk in zip(A, k):
            s += a / kk * math.exp(-kk * max(0.0, t - (t_arr + gap)))
        if extra_atoms is not None:
            s += float(np.sum(extra_atoms[1][extra_atoms[0] > t]))
        return s
    target = S0 * 10 ** (bottom_db / 10)
    lo, hi = 0.0, 1.0
    while S_after(hi) > target:
        hi *= 2
        if hi > 1e3:
            return float('nan')
    for _ in range(200):
        mid = 0.5 * (lo + hi)
        if S_after(mid) > target:
            lo = mid
        else:
            hi = mid
    u10 = hi
    u = np.arange(1, int(u10 / fine) + 1) * fine
    t = t_arr + u
    s = np.zeros_like(u)
    for a, kk in zip(A, k):
        s += a / kk * np.exp(-kk * np.maximum(0.0, t - (t_arr + gap)))
    if extra_atoms is not None:
        ta, ea = extra_atoms
        order = np.argsort(ta)
        ta, ea = ta[order], ea[order]
        cum_after = np.concatenate([np.cumsum(ea[::-1])[::-1], [0.0]])
        j = np.searchsorted(ta, t, side='right')
        s += cum_after[j]
    y = 10 * np.log10(s / S0)
    if len(u) < 3:
        return float('nan')
    a_, _b = np.polyfit(u, y, 1)
    return -60.0 / a_ if a_ < 0 else float('nan')


def sabine_direct_energy(V, T60, d, S_rev):
    """Ed from the classical direct-to-reverberant ratio A/(16 pi d^2), A = 0.161 V / T60."""
    A = 0.161 * V / T60
    return S_rev * A / (16 * math.pi * d * d)


def noisy(bins, w, rng):
    """Compound Poisson (random-mode SPPS: every hit carries energy w)."""
    lam = np.maximum(bins, 0.0) / w
    return w * rng.poisson(lam).astype(float)
