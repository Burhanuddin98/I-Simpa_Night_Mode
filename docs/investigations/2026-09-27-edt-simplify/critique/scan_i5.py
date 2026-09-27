"""Scan 3 (I5): Monte-Carlo particle noise, random-mode SPPS model (each receiver hit carries a fixed
energy, so the hit rate decays with the energy): reverberant hits per ms at the receiver
lam0 = N pi R^2 c / V /1000, direct hits lam_d = N R^2 / (4 d^2). Truth = the noise-free Definition-A
EDT. 200 seeds per configuration, run 1.5 x T60 (no I4 confound). Reports P(status ok & |err|>5%),
plus the same candidate's noise-free value (to separate noise from the I2 bias)."""
import sys, math, itertools, collections, json
sys.dont_write_bytecode = True
import numpy as np
import synth as S
from common import CANDS, score

R = 0.31; h = R / S.C
CONFIGS = [  # V, T60, d, N particles, dt_ms
    (200, 0.6, 2.0, 150e3, 1.0), (200, 0.6, 5.0, 150e3, 1.0), (200, 0.6, 5.0, 150e3, 10.0),
    (2000, 1.5, 5.0, 150e3, 1.0), (2000, 1.5, 15.0, 150e3, 1.0), (2000, 1.5, 15.0, 150e3, 10.0),
    (20000, 2.5, 10.0, 150e3, 1.0), (20000, 2.5, 30.0, 150e3, 1.0), (20000, 2.5, 30.0, 150e3, 10.0),
    (20000, 2.5, 30.0, 1.5e6, 1.0), (20000, 2.5, 30.0, 15e3, 1.0), (2000, 1.5, 15.0, 15e3, 1.0),
]
NSEED = 200
out = []
rng = np.random.default_rng(20260927)
for V, T60, d, N, dt_ms in CONFIGS:
    dt = dt_ms * 1e-3
    k = 6 * math.log(10) / T60
    t_arr = d / S.C
    gap = 0.0
    A = [1.0]; Srev = 1.0 / k
    Ed = S.sabine_direct_energy(V, T60, d, Srev)
    truth = S.truth_edt(t_arr, Ed, gap, A, [k])
    T_run = t_arr + 1.5 * T60
    lam0 = N * math.pi * R * R * S.C / V            # hits per second at t_r (all particles alive)
    lam_d = N * R * R / (4 * d * d)
    # split the clean histogram into direct and reverberant parts, noise each with its own hit energy
    b_dir = S.histogram(dt, T_run, t_arr, h, Ed, 1e9, [0.0], [k])      # gap=1e9: no reverb
    b_rev = S.histogram(dt, T_run, t_arr, h, 0.0, gap, A, [k])
    clean = b_dir + b_rev
    w_rev = 1.0 / lam0                                # density A=1 per s  <->  lam0 hits per s
    w_dir = Ed / lam_d
    clean_res = {n: m.analyse(clean, dt, t_arr, {}) for n, m in CANDS.items()}
    tallies = {n: collections.Counter() for n in CANDS}
    errs = {n: [] for n in CANDS}
    for s in range(NSEED):
        b = S.noisy(b_dir, w_dir, rng) + S.noisy(b_rev, w_rev, rng)
        for n, m in CANDS.items():
            r = m.analyse(b, dt, t_arr, {})
            kind, e = score(r, truth)
            tallies[n][kind] += 1
            if e is not None:
                errs[n].append(e)
    lvl = 10 * math.log10(Srev / (Ed + Srev))
    print(f'V={V} T60={T60} d={d} N={N:.0e} dt={dt_ms}ms  lam0={lam0/1000:.1f} hits/ms  direct hits={lam_d:.1f}  after-direct {lvl:.1f} dB  truth={truth:.3f}')
    for n in CANDS:
        cr = clean_res[n]
        ce = None if cr['edt'] is None else cr['edt'] / truth - 1
        ee = np.array(errs[n]) if errs[n] else np.array([np.nan])
        print(f'   {n:9s} noise-free: {cr["status"]:7s} err={"n/a" if ce is None else "%+.1f%%" % (100*ce)}   noisy: {dict(tallies[n])}  err sd={100*np.nanstd(ee):.1f}%  |err| p95={100*np.nanpercentile(np.abs(ee),95):.1f}%')
        out.append(dict(V=V, T60=T60, d=d, N=N, dt_ms=dt_ms, cand=n, tally=dict(tallies[n]), clean_err=ce,
                        err_sd=float(np.nanstd(ee)), p95=float(np.nanpercentile(np.abs(ee), 95))))
    sys.stdout.flush()
json.dump(out, open('scan_i5.json', 'w'))
