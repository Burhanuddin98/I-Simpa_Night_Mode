"""Scan 5: (a) energetic-mode noise (hit count constant, hit energy decays) for the large hall that
failed in random mode; (b) two sources at different distances; (c) degenerate inputs (I6).
Single process, numpy only."""
import sys, math, collections, warnings
sys.dont_write_bytecode = True
warnings.filterwarnings('ignore')
import numpy as np
import synth as S
from common import CANDS, run_all, score

R = 0.31
h = R / S.C
rng = np.random.default_rng(7)

print('(a) energetic-mode noise, V=20000 m3, T60=2.5 s, d=10 m, 1 ms, 200 seeds')
for N in (150e3, 1.5e6):
    V, T60, d, dt = 20000.0, 2.5, 10.0, 1e-3
    k = 6 * math.log(10) / T60
    t_arr = d / S.C
    Ed = S.sabine_direct_energy(V, T60, d, 1.0 / k)
    truth = S.truth_edt(t_arr, Ed, 0.0, [1.0], [k])
    b_dir = S.histogram(dt, t_arr + 1.5 * T60, t_arr, h, Ed, 1e9, [0.0], [k])
    b_rev = S.histogram(dt, t_arr + 1.5 * T60, t_arr, h, 0.0, 0.0, [1.0], [k])
    lam_bin = N * math.pi * R * R * S.C / V * dt          # hits per bin, constant (energetic mode)
    lam_d = N * R * R / (4 * d * d)
    tal = {n: collections.Counter() for n in CANDS}
    for s in range(200):
        rev = np.where(b_rev > 0, b_rev * rng.poisson(lam_bin, size=b_rev.size) / lam_bin, 0.0)
        b = S.noisy(b_dir, Ed / lam_d, rng) + rev
        for n, m in CANDS.items():
            tal[n][score(m.analyse(b, dt, t_arr, {}), truth)[0]] += 1
    print('   N=%.0e hits/bin=%.2f direct hits=%.0f:' % (N, lam_bin, lam_d), {n: dict(t) for n, t in tal.items()})

print('(b) two sources (0 dB at the first direct; the second direct is a later spike), noise-free, 1 ms')
for d1, d2, T60, V in ((3.0, 8.0, 1.0, 1000.0), (2.0, 12.0, 1.5, 3000.0), (1.0, 6.0, 0.6, 300.0), (4.0, 20.0, 2.0, 10000.0)):
    dt = 1e-3
    k = 6 * math.log(10) / T60
    t1, t2 = d1 / S.C, d2 / S.C
    Srev = 2.0 / k                                       # two equal-power sources
    E1 = S.sabine_direct_energy(V, T60, d1, 1.0 / k)
    E2 = S.sabine_direct_energy(V, T60, d2, 1.0 / k)
    atoms = S.ball_atoms(t2, h, E2)
    truth = S.truth_edt(t1, E1, 0.0, [2.0], [k], extra_atoms=atoms)
    b = S.histogram(dt, t1 + 2 * T60, t1, h, E1, 0.0, [2.0], [k], extra_atoms=atoms)
    res = run_all(b, dt, t1)
    print('   d1=%g d2=%g T60=%g truth=%.3f ' % (d1, d2, T60, truth),
          {n: (None if r['edt'] is None else round(r['edt'], 3), r['status'],
               None if r['edt'] is None else '%+.1f%%' % (100 * (r['edt'] / truth - 1))) for n, r in res.items()})

print('(c) degenerate inputs')
cases = {
    'all zero': (np.zeros(500), 1e-3, 0.01),
    'single bin': (np.array([1.0]), 1e-3, 0.0),
    'direct only': (np.r_[np.zeros(10), 1.0, np.zeros(489)], 1e-3, 0.0105),
    'flat (non-decaying)': (np.r_[np.zeros(10), np.ones(490)], 1e-3, 0.0105),
    'rising': (np.r_[np.zeros(10), np.linspace(0.1, 1, 490)], 1e-3, 0.0105),
    'arrival after run end': (np.ones(100), 1e-3, 0.5),
    'NaN bin': (np.r_[np.zeros(10), np.exp(-np.arange(490) / 50.0)] * np.r_[np.ones(200), np.nan, np.ones(299)], 1e-3, 0.0105),
    'negative bin': (np.r_[np.zeros(10), np.exp(-np.arange(490) / 50.0)] * np.r_[np.ones(200), -1.0, np.ones(299)], 1e-3, 0.0105),
    'truncated at -3 dB': (np.r_[np.zeros(10), np.exp(-np.arange(12) / 50.0)], 1e-3, 0.0105),
    't_arrival None': (np.r_[np.zeros(10), np.exp(-np.arange(490) / 50.0)], 1e-3, None),
}
for name, (b, dt, ta) in cases.items():
    out = {}
    for n, m in CANDS.items():
        try:
            r = m.analyse(b, dt, ta, {})
            out[n] = (r['status'], str(r['reason'])[:28], None if r['edt'] is None else round(float(r['edt']), 4))
        except Exception as ex:
            out[n] = ('CRASH', type(ex).__name__)
    print('   %-22s' % name, out)
