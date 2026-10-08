"""PREREG-5 gate 3c: CR3 with every scattering 0. SPPS is then purely specular and must reproduce the image sources,
two-sided, on the cumulative early energy.

  python gate3c.py [results json, default C:\\tmp\\nm-spps-runs\\cr3_s0.json]

Per pair and band, at t on a 1 ms grid from the direct arrival + r/c to t_c (1 ms before the pair's earliest valid
order-6 arrival):  E_SPPS(t - r/c)/(1 + 3 sigma) <= E_ISM(t) <= E_SPPS(t + r/c)(1 + 3 sigma), sigma = 1/sqrt(N(t)).
E_ISM: orders <= 5, prod (1 - alpha), SPPS's air absorption, rho c 413.25.
"""
import json, sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).parent
RES = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(r'C:\tmp\nm-spps-runs\cr3_s0.json')
ROOM = sys.argv[2] if len(sys.argv) > 2 else 'CR3'
PROJ = Path(sys.argv[3]) if len(sys.argv) > 3 else Path(r'C:\tmp\nm-spps-projects\bras_cr3_s0.simpa')
ARR = Path(sys.argv[4]) if len(sys.argv) > 4 else Path(__file__).parent / f'arrivals_{ROOM}.npz'
REPO = Path(r'C:\repos\Room-Acoustics-Engine\I-Simpa FDTD')
RHO_C = 413.25
d = json.loads(RES.read_text()); s = d['spps']
c, dt, rr = s['speed_of_sound_m_s'], s['time_step_s'], s['receiver_radius_m']
k_r = int(np.ceil(rr / c / dt - 1e-9))
z = np.load(ARR); pg = z['plane_group']; top = int(z['order_ray'])
p = json.loads((REPO / 'app/src-tauri/examples' / f'bras_{ROOM.lower()}.simpa').read_text(encoding='utf-8'))
mat_of_group = {g['id']: g['material'] for g in p['surface_groups']}
mats = {m['id']: m for m in p['materials']}
bands = p['bands']['frequencies_hz']
air = {int(b['freq_hz']): b['air_m_per_metre'] for b in s['reference']['bands']}
pos = {x['name']: (np.array(x['position_m']), x['band_power_w']) for x in s['sources']}
assert all(v == 0 for m in json.loads(PROJ.read_text(encoding='utf-8'))['materials']
           for v in m['scattering']), 'the run project is not scattering 0'

fails, ends, n_checks = [], [], 0
for r in s['point_receivers']:
    R = np.array(r['position_m'])
    for ps in r['per_source']:
        S, W = pos[ps['source']]
        dist = float(np.linalg.norm(S - R))
        sel = (z['src'] == ps['source']) & (z['rcv'] == r['label'])
        L, od = z['length'][sel], z['order'][sel]
        planes = [json.loads(x) for x in z['planes'][sel]]
        t_c = L[od == top].min() / c - dt if (od == top).any() else 0.15
        k_lo, k_c = int(np.floor(dist / c / dt)) + k_r + 1, int(np.floor(t_c / dt))
        for bi, fc in enumerate(bands):
            b = next((x for x in ps['bands'] if int(x['freq_hz']) == fc), None)
            if b is None or not b.get('energy_pa2') or not any(b['energy_pa2']):
                continue                                    # a band the run did not compute
            e = np.array(b['energy_pa2'], float); md = b['noise_model']['mean_deposit']
            ism = np.zeros(len(e))
            for Li, pl, o in zip(L, planes, od):
                if o >= top:
                    continue
                f = 1.0
                for q in pl:
                    f *= 1 - mats[mat_of_group[pg[q]]]['absorption'][bi]
                k = int(np.floor(Li / c / dt))
                if k < len(e):
                    ism[k] += RHO_C * W[d['bands_hz'].index(fc)] / (4 * np.pi * Li ** 2) * np.exp(-air[fc] * Li) * f
            Ci, Cs = np.cumsum(ism), np.cumsum(e)
            for k in range(k_lo, k_c + 1):
                lo, hi = Cs[k - k_r], Cs[min(k + k_r, len(Cs) - 1)]
                tol = 3 / np.sqrt(max(Cs[k] / md, 1.0))
                n_checks += 1
                if not (lo / (1 + tol) <= Ci[k] <= hi * (1 + tol)):
                    fails.append((f'{ps["source"]}-{r["label"]}', fc, round(1e3 * k * dt, 1),
                                  round(10 * np.log10(Ci[k] / Cs[k]), 2), round(10 * np.log10(lo / Cs[k]), 2),
                                  round(10 * np.log10(hi / Cs[k]), 2)))
            ends.append((f'{ps["source"]}-{r["label"]}', fc, 1e3 * t_c, 10 * np.log10(Ci[k_c] / Cs[k_c])))

print(f'{ROOM} scattering 0 ({RES.name}): receiver radius {rr} m (k_r {k_r} bins), {n_checks} checks over 10 pairs x {len(bands)} bands')
er = np.array([x[3] for x in ends])
print(f'E_ISM/E_SPPS at t_c: mean {er.mean():+.3f} dB, range {er.min():+.3f}..{er.max():+.3f} dB; '
      f't_c {min(x[2] for x in ends):.0f}..{max(x[2] for x in ends):.0f} ms')
for fc in bands:
    v = np.array([x[3] for x in ends if x[1] == fc])
    if not len(v):
        continue
    print(f'  {fc:5d} Hz: mean {v.mean():+.3f} dB, mean |.| {np.abs(v).mean():.3f} dB, range {v.min():+.3f}..{v.max():+.3f}')
print(f'GATE 3c {"PASS" if not fails else "FAIL"}: {len(fails)} of {n_checks} checks outside the band')
for x in fails[:25]:
    print('   out: pair, band, t ms, ISM/SPPS dB, lower edge dB, upper edge dB:', x)
