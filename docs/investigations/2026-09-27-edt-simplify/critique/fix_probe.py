"""Does the I1/I2 part of LeanBand's failures go away with three small changes? A probe, not a candidate.
  (1) origin bin = floor((t_arr - R/c)/dt): keep the front half of the ball-smeared direct sound;
  (2) Schroeder samples at bin START boundaries (S(i dt) = sum(bins[i:]) is exact there, no I3 guess),
      0 dB = total from the origin bin;
  (3) the 0 dB sample itself is NOT a regression point (ISO on a finely sampled response gives the
      direct sound's drop ~zero weight): fit boundaries from t_arr + R/c to the first one <= -10 dB;
  (4) occluded path: if the first recorded energy comes more than one bin after t_arr + R/c, the
      origin moves to that onset (ISO: 0 dB at the first arrival).
Everything else is LeanBand as submitted (OLS, 2-sigma slope CI, ok iff half-width <= 5 %, the
CATT run-length gate). Evaluated on this critique's scan 1 grid and on the ISM eval rows probed."""
import sys, math, itertools, collections, warnings
sys.dont_write_bytecode = True
warnings.filterwarnings('ignore')
import numpy as np
import synth as S
from common import CANDS, score


def probe_edt(bins, dt, t_arr, R=0.31, c=343.2):
    b = np.asarray(bins, float)
    n = len(b)
    h = R / c
    k0 = max(int(math.floor((t_arr - h) / dt)), 0)
    if k0 >= n or b[k0:].sum() <= 0:
        return dict(edt=None, edt_lo=None, edt_hi=None, status='refused', reason='no_energy')
    nz = np.nonzero(b[k0:] > 0)[0]
    k_on = k0 + int(nz[0])
    t_start = t_arr + h
    if k_on * dt > t_arr + h + dt:          # occluded: first energy well after the geometric arrival
        k0 = k_on
        t_start = (k_on + 1) * dt
    S_ = np.cumsum(b[::-1])[::-1]            # S_[i] = energy after i*dt (exact)
    lvl = 10 * np.log10(np.maximum(S_[k0:], 1e-300) / S_[k0])
    T = np.arange(k0, n) * dt
    idx = np.nonzero(T >= t_start - 1e-12)[0]
    below = np.nonzero(lvl <= -10.0)[0]
    if len(below) == 0 or len(idx) == 0:
        return dict(edt=None, edt_lo=None, edt_hi=None, status='refused', reason='run_too_short')
    i_end = int(below[0])
    sel = idx[idx <= i_end]
    if len(sel) < 4:
        return dict(edt=None, edt_lo=None, edt_hi=None, status='refused', reason='too_few_points')
    t, y = T[sel], lvl[sel]
    tb, yb = t.mean(), y.mean()
    sxx = float(((t - tb) ** 2).sum())
    a = float(((t - tb) * (y - yb)).sum() / sxx)
    if a >= 0:
        return dict(edt=None, edt_lo=None, edt_hi=None, status='refused', reason='not_decaying')
    resid = y - (a * (t - tb) + yb)
    se = math.sqrt(float((resid ** 2).sum()) / max(len(t) - 2, 1) / sxx)
    if n * dt < t[0] + 20.0 / (-a):
        return dict(edt=None, edt_lo=None, edt_hi=None, status='refused', reason='run_too_short')
    edt = -60 / a
    lo = -60 / (a - 2 * se)
    hi = -60 / (a + 2 * se) if a + 2 * se < 0 else 10 * edt
    hw = (hi - lo) / 2 / edt
    return dict(edt=edt, edt_lo=lo, edt_hi=hi, status='ok' if hw <= 0.05 else 'wide', reason='hw=%.3f' % hw)


if __name__ == '__main__':
    R = 0.31
    h = R / S.C
    tal = {'leanband': collections.Counter(), 'probe': collections.Counter()}
    worst = []
    for V, T60, d, gap_ms, dt_ms in itertools.product((200.0, 2000.0, 20000.0), (0.3, 0.6, 1.0, 2.0, 3.0),
                                                       (0.7, 1.0, 1.5, 2.0, 3.0, 5.0, 8.0, 12.0, 20.0, 30.0),
                                                       (0.0, 2.0, 5.0, 10.0), (1.0, 2.0, 5.0, 10.0)):
        if d > 1.6 * V ** (1 / 3):
            continue
        dt = dt_ms * 1e-3
        k = 6 * math.log(10) / T60
        for phase in (0.05, 0.5, 0.95):
            t_arr = (math.floor((d / S.C) / dt) + phase) * dt
            if t_arr * S.C < 0.6:
                continue
            Ed = S.sabine_direct_energy(V, T60, t_arr * S.C, 1.0 / k)
            if 10 * math.log10((1 / k) / (Ed + 1 / k)) < -8:
                continue                            # ill-conditioned rows excluded, as in scan 1
            truth = S.truth_edt(t_arr, Ed, gap_ms * 1e-3, [1.0], [k])
            b = S.histogram(dt, 2.0 * T60 + t_arr + gap_ms * 1e-3, t_arr, h, Ed, gap_ms * 1e-3, [1.0], [k])
            tal['leanband'][score(CANDS['leanband'].analyse(b, dt, t_arr, {}), truth)[0]] += 1
            kind, e = score(probe_edt(b, dt, t_arr), truth)
            tal['probe'][kind] += 1
            if kind == 'ok_WRONG':
                worst.append((e, V, T60, d, gap_ms, dt_ms, phase))
    print('scan-1 grid, well-conditioned rows:')
    for kk, v in tal.items():
        print('  ', kk, dict(v))
    for w in sorted(worst, key=lambda x: -abs(x[0]))[:5]:
        print('   probe wrong-silent err=%+.1f%% V=%g T60=%g d=%g gap=%gms dt=%gms phase=%g' % (100 * w[0], *w[1:]))
    # occluded rows (scan 4 construction)
    tal2 = {'leanband': collections.Counter(), 'probe': collections.Counter()}
    for T60, delay_ms, dt_ms in itertools.product((0.4, 0.8, 1.5, 3.0), (2.0, 5.0, 10.0, 20.0, 40.0), (1.0, 2.0, 10.0)):
        dt = dt_ms * 1e-3
        k = 6 * math.log(10) / T60
        t_arr = 0.030 + 0.41 * dt
        truth = S.truth_edt(t_arr + delay_ms * 1e-3, 0.0, 0.0, [1.0], [k])
        b = S.histogram(dt, t_arr + delay_ms * 1e-3 + 2 * T60, t_arr, h, 0.0, delay_ms * 1e-3, [1.0], [k])
        tal2['leanband'][score(CANDS['leanband'].analyse(b, dt, t_arr, {}), truth)[0]] += 1
        tal2['probe'][score(probe_edt(b, dt, t_arr), truth)[0]] += 1
    print('occluded rows:')
    for kk, v in tal2.items():
        print('  ', kk, dict(v))
