"""Is there a simple, data-only noise estimate that predicts the seed-to-seed EDT spread, where the OLS
slope standard error does not? Compound-Poisson bins: var(b) = w * E[b] (w = energy per receiver hit),
estimated from adjacent-bin differences, w_hat = sum (b_i - b_{i+1})^2 / (2 sum b_i) over the decay.
Then the Schroeder level at the -10 dB point has sd ~ 4.343 sqrt(w / S(t10)) dB (a Poisson sum of
N_eff = S(t10)/w hits), and EDT's relative sd ~ that / 10 dB. Compared against the empirical spread
over 200 seeds, same configurations as scan_i5 (random mode), plus leanband's own implied sd."""
import sys, math, warnings
sys.dont_write_bytecode = True
warnings.filterwarnings('ignore')
import numpy as np
import synth as S
from common import CANDS

R = 0.31
h = R / S.C
rng = np.random.default_rng(11)
CONFIGS = [(200, 0.6, 5.0, 150e3, 1.0), (2000, 1.5, 15.0, 150e3, 1.0), (20000, 2.5, 30.0, 150e3, 1.0),
           (20000, 2.5, 30.0, 1.5e6, 1.0), (2000, 1.5, 15.0, 150e3, 10.0)]
print('config | empirical EDT sd | predicted sd (Poisson count) median | leanband implied sd median | N_eff median')
for V, T60, d, N, dt_ms in CONFIGS:
    dt = dt_ms * 1e-3
    k = 6 * math.log(10) / T60
    t_arr = d / S.C
    Ed = S.sabine_direct_energy(V, T60, d, 1.0 / k)
    T_run = t_arr + 1.5 * T60
    b_dir = S.histogram(dt, T_run, t_arr, h, Ed, 1e9, [0.0], [k])
    b_rev = S.histogram(dt, T_run, t_arr, h, 0.0, 0.0, [1.0], [k])
    lam0 = N * math.pi * R * R * S.C / V
    lam_d = N * R * R / (4 * d * d)
    edts, preds, impl, neffs = [], [], [], []
    for s in range(200):
        b = S.noisy(b_dir, Ed / lam_d, rng) + S.noisy(b_rev, 1.0 / lam0, rng)
        r = CANDS['leanband'].analyse(b, dt, t_arr, {})
        if r['edt'] is None:
            continue
        edts.append(r['edt'])
        impl.append((r['edt_hi'] - r['edt_lo']) / 2 / 2.0 / r['edt'])      # 2-sigma half-width -> sd
        k0 = int(math.floor((t_arr + h) / dt)) + 1
        seg = b[k0:]
        Ssum = np.cumsum(seg[::-1])[::-1]
        i10 = int(np.nonzero(Ssum <= 0.1 * Ssum[0])[0][0])
        dec = seg[i10: max(i10 * 3, i10 + 10)]   # diffuse part only: after the -10 dB point (early reflections are structure, not noise)
        w_hat = float(((dec[:-1] - dec[1:]) ** 2).sum() / (2 * dec.sum()))
        n_eff = Ssum[i10] / w_hat
        neffs.append(n_eff)
        preds.append(4.343 / math.sqrt(n_eff) / 10.0)
    e = np.array(edts)
    print('V=%g T60=%g d=%g N=%.0e dt=%gms | %.1f%% | %.1f%% | %.1f%% | %.0f' % (
        V, T60, d, N, dt_ms, 100 * e.std() / e.mean(), 100 * np.median(preds), 100 * np.median(impl), np.median(neffs)))
