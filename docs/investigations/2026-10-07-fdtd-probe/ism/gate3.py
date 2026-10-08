"""PREREG-5 gate 3, done properly (the splice log's 5-bin "direct" mixed in early reflections; see RESULT-5).

  python gate3.py <ROOM> <results json>

(a) Direct sound alone: SPPS's energy over the receiver sphere's crossing span [(d - r)/c, (d + r)/c] against the
    ISM's order 0, rho c W/(4 pi d^2) exp(-m d), for pairs whose first reflection arrives after that span.
    Pass: within 1 dB.
(b) Arm H (specular only, the lower bound) never above SPPS in the early window [0, t_dir + t_tr) by more than
    3 sigma of SPPS's Monte-Carlo scatter, sigma_rel = 1/sqrt(N), N = window energy / mean deposit per crossing.
"""
import json, sys
from pathlib import Path

import numpy as np

ROOM, RES = sys.argv[1], Path(sys.argv[2])
V2 = '--v2' in sys.argv   # PREREG-5 amendment 23:08 (CR4): SPPS's window extends by ceil(r/c/dt) bins
HERE = Path(__file__).parent
REPO = Path(r'C:\repos\Room-Acoustics-Engine\I-Simpa FDTD')
RHO_C = 413.25
d = json.loads(RES.read_text()); s = d['spps']
c, dt, rr = s['speed_of_sound_m_s'], s['time_step_s'], s['receiver_radius_m']
z = np.load(HERE / f'arrivals_{ROOM}.npz'); pg = z['plane_group']; t_tr = float(z['t_tr'])
p = json.loads((REPO / 'app/src-tauri/examples' / f'bras_{ROOM.lower()}.simpa').read_text(encoding='utf-8'))
mat_of_group = {g['id']: g['material'] for g in p['surface_groups']}
mats = {m['id']: m for m in p['materials']}
bands = p['bands']['frequencies_hz']
air = {int(b['freq_hz']): b['air_m_per_metre'] for b in s['reference']['bands']}
pos = {x['name']: (np.array(x['position_m']), x['band_power_w']) for x in s['sources']}

direct, early = [], []
for r in s['point_receivers']:
    R = np.array(r['position_m'])
    for ps in r['per_source']:
        S, W = pos[ps['source']]
        dist = float(np.linalg.norm(S - R))
        sel = (z['src'] == ps['source']) & (z['rcv'] == r['label'])
        L, od = z['length'][sel], z['order'][sel]
        planes = [json.loads(x) for x in z['planes'][sel]]
        L1 = L[od >= 1].min()
        k0, k1 = int(np.floor((dist - rr) / c / dt)), int(np.floor((dist + rr) / c / dt))
        clean = (L1 - rr) / c > (k1 + 1) * dt and (od == 0).any()
        n_hi = int(np.floor(dist / c / dt)) + int(np.floor(t_tr / dt + 1e-9))
        for bi, fc in enumerate(bands):
            b = next(x for x in ps['bands'] if int(x['freq_hz']) == fc)
            e = np.array(b['energy_pa2'], float)
            if clean:
                ism0 = RHO_C * W[bi] / (4 * np.pi * dist ** 2) * np.exp(-air[fc] * dist)
                direct.append((f'{ps["source"]}-{r["label"]}', fc, 10 * np.log10(e[k0:k1 + 1].sum() / ism0)))
            eh = 0.0
            for Li, pl in zip(L, planes):
                if int(np.floor(Li / c / dt)) < n_hi:
                    f = 1.0
                    for q in pl:
                        m = mats[mat_of_group[pg[q]]]; f *= (1 - m['absorption'][bi]) * (1 - m['scattering'][bi])
                    eh += RHO_C * W[bi] / (4 * np.pi * Li ** 2) * np.exp(-air[fc] * Li) * f
            k_r = int(np.ceil(rr / c / dt - 1e-9)) if V2 else 0
            es = e[:n_hi + k_r].sum()
            N = es / b['noise_model']['mean_deposit']
            tol = 10 * np.log10(1 + 3 / np.sqrt(N))
            early.append((f'{ps["source"]}-{r["label"]}', fc, 10 * np.log10(eh / es), tol))

dd = np.array([x[2] for x in direct])
ea = np.array([x[2] for x in early]); tl = np.array([x[3] for x in early])
over = [(x[0], x[1], round(x[2], 2), round(x[3], 2)) for x in early if x[2] > x[3]]
a_ok = len(dd) > 0 and np.all(np.abs(dd) <= 1.0)
b_ok = not over
print(f'{ROOM} receiver radius {rr} m, t_tr {1e3 * t_tr:.1f} ms after the direct sound, gate {"v2 (SPPS window + r/c)" if V2 else "v1"}')
print(f'(a) direct, SPPS/ISM: {len(dd)} pair-bands clean of reflections '
      f'({len({x[0] for x in direct})} of {len({x[0] for x in early})} pairs); '
      f'mean {dd.mean() if len(dd) else float("nan"):+.2f} dB, range {dd.min() if len(dd) else float("nan"):+.2f}..{dd.max() if len(dd) else float("nan"):+.2f} dB -> {"PASS" if a_ok else "FAIL" if len(dd) else "NOT EVALUABLE"}')
print(f'(b) early, H/SPPS: range {ea.min():+.2f}..{ea.max():+.2f} dB, 3-sigma tolerance {tl.min():.2f}..{tl.max():.2f} dB; '
      f'above tolerance {len(over)} of {len(early)} -> {"PASS" if b_ok else "FAIL"}')
for x in over:
    print('   over:', x)
