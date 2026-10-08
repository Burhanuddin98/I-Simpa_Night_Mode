"""Where in time CR4's image-source early part loses energy against SPPS (base run, 125 Hz, scattering ~0.05, so
the (1 - alpha)-only arrivals should nearly equal SPPS). Cumulative E_ISM(t) / E_SPPS(t + r/c), mean over the 10
pairs, at t = direct + 5, 10, 20, 30, 45, 60, 93 ms; and the same with only orders <= 2 (exhaustive, complete)."""
import json
from pathlib import Path

import numpy as np

HERE = Path(__file__).parent
REPO = Path(r'C:\repos\Room-Acoustics-Engine\I-Simpa FDTD')
RHO_C = 413.25
d = json.loads(Path(r'C:\tmp\nm-spps-runs\cr4_base.json').read_text()); s = d['spps']
c, dt, rr = s['speed_of_sound_m_s'], s['time_step_s'], s['receiver_radius_m']
k_r = int(np.ceil(rr / c / dt - 1e-9))
z = np.load(HERE / 'arrivals_CR4.npz'); pg = z['plane_group']
p = json.loads((REPO / 'app/src-tauri/examples/bras_cr4.simpa').read_text(encoding='utf-8'))
mat_of_group = {g['id']: g['material'] for g in p['surface_groups']}
mats = {m['id']: m for m in p['materials']}
air = {int(b['freq_hz']): b['air_m_per_metre'] for b in s['reference']['bands']}
pos = {x['name']: (np.array(x['position_m']), x['band_power_w']) for x in s['sources']}
OFF = [5, 10, 20, 30, 45, 60, 93]
for bi, fc in [(0, 125), (3, 1000)]:
    allr, low = [], []
    for r in s['point_receivers']:
        for ps in r['per_source']:
            S, W = pos[ps['source']]; dist = float(np.linalg.norm(S - np.array(r['position_m'])))
            sel = (z['src'] == ps['source']) & (z['rcv'] == r['label'])
            e = np.array(next(x for x in ps['bands'] if int(x['freq_hz']) == fc)['energy_pa2'], float)
            ism, ism2 = np.zeros(len(e)), np.zeros(len(e))
            for Li, pl, o in zip(z['length'][sel], z['planes'][sel], z['order'][sel]):
                f = 1.0
                for q in json.loads(pl):
                    f *= 1 - mats[mat_of_group[pg[q]]]['absorption'][bi]
                k = int(np.floor(Li / c / dt))
                v = RHO_C * W[bi] / (4 * np.pi * Li ** 2) * np.exp(-air[fc] * Li) * f
                ism[k] += v
                if o <= 2:
                    ism2[k] += v
            Ci, C2, Cs = np.cumsum(ism), np.cumsum(ism2), np.cumsum(e)
            k0 = int(np.floor(dist / c / dt))
            allr.append([10 * np.log10(Ci[k0 + o] / Cs[k0 + o + k_r]) for o in OFF])
            low.append([10 * np.log10(C2[k0 + o] / Cs[k0 + o + k_r]) for o in OFF])
    print(f'CR4 {fc} Hz, ms after direct {OFF}')
    print('  all orders <= 6 :', np.round(np.mean(allr, 0), 2).tolist(), ' worst pair', np.round(np.min(allr, 0), 2).tolist())
    print('  orders <= 2 only:', np.round(np.mean(low, 0), 2).tolist())
