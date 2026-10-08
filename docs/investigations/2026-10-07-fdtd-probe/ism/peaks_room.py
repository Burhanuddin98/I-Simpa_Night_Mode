"""Gate 3c diagnosis: CR3 with scattering 0, reflection by reflection. SPPS (10 M particles, r 0.31 m) is a train of
specular peaks; each image-source arrival isolated by >= 3 bins from any other is compared with SPPS's energy in its
bin +-1 (the receiver sphere spreads it by <= 1 bin). And SPPS energy in bins far (>= 3) from every image-source
arrival is the energy of paths the image sources do not have.

  python peaks.py [band index, default 2 = 500 Hz]
"""
import json, sys
from collections import Counter
from pathlib import Path

import numpy as np

HERE = Path(__file__).parent
REPO = Path(r'C:\repos\Room-Acoustics-Engine\I-Simpa FDTD')
RHO_C = 413.25
bi = int(sys.argv[1]) if len(sys.argv) > 1 else 2
NOABS = "--noabs" in sys.argv   # diagnosis: arrival energies without the (1 - alpha) factors
ROOM = sys.argv[3] if len(sys.argv) > 3 else 'CR3'
d = json.loads(Path(sys.argv[4] if len(sys.argv) > 4 else r'C:\tmp\nm-spps-runs\cr3_s0.json').read_text()); s = d['spps']
c, dt = s['speed_of_sound_m_s'], s['time_step_s']
z = np.load(HERE / f'arrivals_{ROOM}.npz'); pg = z['plane_group']; top = int(z['order_ray'])
p = json.loads((REPO / 'app/src-tauri/examples' / f'bras_{ROOM.lower()}.simpa').read_text(encoding='utf-8'))
mat_of_group = {g['id']: g['material'] for g in p['surface_groups']}
mats = {m['id']: m for m in p['materials']}
fc = p['bands']['frequencies_hz'][bi]
air = {int(b['freq_hz']): b['air_m_per_metre'] for b in s['reference']['bands']}
pos = {x['name']: (np.array(x['position_m']), x['band_power_w']) for x in s['sources']}

iso, unmatched_spps, by_order = [], [], {}
for r in s['point_receivers']:
    for ps in r['per_source']:
        S, W = pos[ps['source']]
        sel = (z['src'] == ps['source']) & (z['rcv'] == r['label'])
        L, od, PL = z['length'][sel], z['order'][sel], z['planes'][sel]
        e = np.array(next(x for x in ps['bands'] if int(x['freq_hz']) == fc)['energy_pa2'], float)
        k_all = np.floor(L / c / dt).astype(int)
        t_c = L[od == top].min() / c - dt if (od == top).any() else 0.15
        kc = int(np.floor(t_c / dt))
        en = []
        for Li, pl in zip(L, PL):
            f = 1.0
            for q in json.loads(pl):
                f *= 1 - (0.0 if NOABS else mats[mat_of_group[pg[q]]]["absorption"][bi])
            en.append(RHO_C * W[d['bands_hz'].index(fc)] / (4 * np.pi * Li ** 2) * np.exp(-air[fc] * Li) * f)
        en = np.array(en)
        for i in np.where((k_all < kc) & (od < top))[0]:
            others = np.delete(k_all, i)
            if np.all(np.abs(others - k_all[i]) >= 3):
                k = k_all[i]
                ratio = e[k - 1:k + 2].sum() / en[i]
                iso.append((f'{ps["source"]}-{r["label"]}', int(od[i]), round(1e3 * L[i] / c, 1), ratio, PL[i]))
                by_order.setdefault(int(od[i]), []).append(ratio)
        # SPPS energy far from every image-source arrival, before t_c
        near = np.zeros(len(e), bool)
        for k in k_all:
            near[max(0, k - 2):k + 3] = True
        far = ~near; far[kc:] = False; far[:int(k_all.min())] = False
        unmatched_spps.append((f'{ps["source"]}-{r["label"]}', e[far].sum() / e[:kc].sum(),
                               [round(1e3 * k * dt) for k in np.where(far & (e > 0.02 * e[:kc].max()))[0]][:12]))

R = np.array([x[3] for x in iso])
print(f'{ROOM} scattering 0, {fc} Hz: {len(iso)} isolated image-source arrivals before t_c')
print(f'  SPPS/ISM energy per arrival: median {np.median(R):.3f}, IQR {np.percentile(R, 25):.3f}..{np.percentile(R, 75):.3f}')
for o in sorted(by_order):
    v = np.array(by_order[o])
    print(f'  order {o}: n {len(v):3d}  median {np.median(v):.3f}  below 0.3 (ISM path SPPS lacks): {int((v < 0.3).sum())}  '
          f'above 2 (SPPS has more): {int((v > 2).sum())}')
bad = sorted([x for x in iso if x[3] < 0.3], key=lambda x: x[3])
print(f'arrivals with SPPS/ISM < 0.3 (likely paths SPPS does not have): {len(bad)}')
for x in bad[:15]:
    print('   ', x)
print('SPPS energy far (>= 3 bins) from every image-source arrival, share of early energy, and its big bins (ms):')
for x in unmatched_spps:
    print(f'   {x[0]}: {100 * x[1]:.1f} %  {x[2]}')
