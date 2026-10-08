"""PREREG-5 gate 3d: specular SPPS in a shoebox against the closed-form image lattice (no polyhedral code).

  python gate3d.py [results json, default C:\\tmp\\nm-spps-runs\\shoebox_s0.json]
"""
import itertools, json, sys
from pathlib import Path

import numpy as np

RES = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(r'C:\tmp\nm-spps-runs\shoebox_s0.json')
RHO_C, LB, ALPHA, NMAX, T_END = 413.25, np.array([7.3, 5.1, 3.2]), 0.10, 12, 0.100
d = json.loads(RES.read_text()); s = d['spps']
c, dt, rr = s['speed_of_sound_m_s'], s['time_step_s'], s['receiver_radius_m']
k_r = int(np.ceil(rr / c / dt - 1e-9))
air = [b['air_m_per_metre'] for b in s['reference']['bands']]
S = np.array(s['sources'][0]['position_m']); W = s['sources'][0]['band_power_w']

# closed-form lattice: per axis x = (1-2q) s + 2 n L, |2n - q| reflections
ax = [[((1 - 2 * q) * S[a] + 2 * n * LB[a], abs(2 * n - q)) for q in (0, 1) for n in range(-7, 8)] for a in range(3)]
img = np.array([[x[0][0], x[1][0], x[2][0], x[0][1] + x[1][1] + x[2][1]] for x in itertools.product(*ax)])
img = img[img[:, 3] <= NMAX]

fails, n_checks, ends, per_arr = [], 0, [], []
for r in s['point_receivers']:
    R = np.array(r['position_m'])
    L = np.linalg.norm(img[:, :3] - R, axis=1); od = img[:, 3]
    keep = L / c < 0.3
    L, od = L[keep], od[keep]
    k_all = np.floor(L / c / dt).astype(int)
    for bi, b in enumerate(r['bands'] if 'energy_pa2' in r['bands'][0] else r['per_source'][0]['bands']):
        e = np.array(b['energy_pa2'], float); md = b['noise_model']['mean_deposit']
        ism = np.zeros(len(e))
        np.add.at(ism, k_all, RHO_C * W[bi] / (4 * np.pi * L ** 2) * np.exp(-air[bi] * L) * (1 - ALPHA) ** od)
        Ci, Cs = np.cumsum(ism), np.cumsum(e)
        k_lo = int(np.floor(np.linalg.norm(S - R) / c / dt)) + k_r + 1
        k_c = int(T_END / dt)
        for k in range(k_lo, k_c + 1):
            lo, hi = Cs[k - k_r], Cs[k + k_r]
            tol = 3 / np.sqrt(max(Cs[k] / md, 1.0)); n_checks += 1
            if not (lo / (1 + tol) <= Ci[k] <= hi * (1 + tol)):
                fails.append((r['label'], int(b['freq_hz']), k, round(10 * np.log10(Ci[k] / Cs[k]), 3)))
        ends.append((r['label'], int(b['freq_hz']), 10 * np.log10(Ci[k_c] / Cs[k_c])))
        if bi == 2:
            for i in range(len(L)):
                if L[i] / c < T_END and np.all(np.abs(np.delete(k_all, i) - k_all[i]) >= 3):
                    per_arr.append((r['label'], int(od[i]), round(1e3 * L[i] / c, 1),
                                    round(e[k_all[i] - 1:k_all[i] + 2].sum() / (ism[k_all[i]]), 3)))

er = np.array([x[2] for x in ends])
print(f'shoebox scattering 0, r {rr} m, {len(s["point_receivers"])} receivers, {n_checks} checks to {1e3 * T_END:.0f} ms')
print(f'E_lattice/E_SPPS at {1e3 * T_END:.0f} ms: mean {er.mean():+.3f} dB, range {er.min():+.3f}..{er.max():+.3f} dB')
print('isolated arrivals at 500 Hz, SPPS/lattice energy (receiver, order, ms, ratio):')
for x in per_arr:
    print('  ', x)
print(f'GATE 3d {"PASS" if not fails else "FAIL"}: {len(fails)} of {n_checks} outside the band')
for x in fails[:12]:
    print('   out:', x)
