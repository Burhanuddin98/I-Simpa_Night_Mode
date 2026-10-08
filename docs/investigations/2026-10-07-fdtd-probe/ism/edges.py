"""Is CR3's per-reflection disagreement with specular SPPS an edge effect of SPPS's receiver sphere?
For each isolated order 1-2 arrival (scattering-0 run, 500 Hz): SPPS/ISM energy ratio, and how far each reflection
point lies inside its plane's polygon (distance to the nearest polygon edge that is not shared with a coplanar
triangle of the same plane). A point receiver sees all of an image or none; a 0.31 m sphere sees part of it when the
reflection point is within about a sphere radius (scaled by the path geometry) of an edge."""
import json
from pathlib import Path

import numpy as np

import os
os.environ['ISM_GPU'] = '0'
from ism import Room  # noqa: E402

RHO_C = 413.25
p = json.loads(Path(r'C:\repos\Room-Acoustics-Engine\I-Simpa FDTD\app\src-tauri\examples\bras_cr3.simpa').read_text(encoding='utf-8'))
V = np.array(p['geometry']['vertices'], float); F = np.array([f[:3] for f in p['geometry']['faces']]); G = [f[3] for f in p['geometry']['faces']]
room = Room(V, F, G)
mog = {g['id']: g['material'] for g in p['surface_groups']}; mats = {m['id']: m for m in p['materials']}
d = json.loads(Path(r'C:\tmp\nm-spps-runs\cr3_s0.json').read_text()); s = d['spps']
c, dt, rr = s['speed_of_sound_m_s'], s['time_step_s'], s['receiver_radius_m']
z = np.load(Path(__file__).parent / 'arrivals_CR3.npz')
assert int(z['n_planes']) == room.P
air = s['reference']['bands'][2]['air_m_per_metre']
pos = {x['name']: (np.array(x['position_m']), x['band_power_w']) for x in s['sources']}

# boundary edges of each plane: triangle edges that occur once within the plane
bedges = []
for tri in room.ptris:
    cnt = {}
    for t in tri:
        vs = [tuple(np.round(room.A[t], 6)), tuple(np.round(room.A[t] + room.E1[t], 6)), tuple(np.round(room.A[t] + room.E2[t], 6))]
        for i in range(3):
            e = tuple(sorted((vs[i], vs[(i + 1) % 3])))
            cnt[e] = cnt.get(e, 0) + 1
    bedges.append([(np.array(a), np.array(b)) for (a, b), k in cnt.items() if k == 1])


def edge_dist(Q, pl):
    best = np.inf
    for a, b in bedges[pl]:
        s_ = np.clip(np.dot(Q - a, b - a) / np.dot(b - a, b - a), 0, 1)
        best = min(best, np.linalg.norm(Q - (a + s_ * (b - a))))
    return best


rows = []
for r in s['point_receivers']:
    R = np.array(r['position_m'])
    for ps in r['per_source']:
        S, W = pos[ps['source']]
        sel = (z['src'] == ps['source']) & (z['rcv'] == r['label'])
        L, od, PL = z['length'][sel], z['order'][sel], z['planes'][sel]
        k_all = np.floor(L / c / dt).astype(int)
        e = np.array(ps['bands'][2]['energy_pa2'], float)
        for i in np.where((od >= 1) & (od <= 2))[0]:
            if not np.all(np.abs(np.delete(k_all, i) - k_all[i]) >= 3):
                continue
            pl = json.loads(PL[i])
            # reflection points by backtrace
            imgs = [S]
            for q in pl:
                n = room.pn[q]; imgs.append(imgs[-1] - 2 * (imgs[-1] @ n - room.pd[q]) * n)
            cur, dmin = R, np.inf
            for j in range(len(pl), 0, -1):
                q = pl[j - 1]; n = room.pn[q]
                a0, a1 = cur @ n - room.pd[q], imgs[j] @ n - room.pd[q]
                Q = cur + a0 / (a0 - a1) * (imgs[j] - cur)
                dmin = min(dmin, edge_dist(Q, q)); cur = Q
            f = np.prod([1 - mats[mog[room.pg[q]]]['absorption'][2] for q in pl])
            en = RHO_C * W[2] / (4 * np.pi * L[i] ** 2) * np.exp(-air * L[i]) * f
            rows.append((f'{ps["source"]}-{r["label"]}', int(od[i]), round(1e3 * L[i] / c, 1), e[k_all[i] - 1:k_all[i] + 2].sum() / en, dmin))

rows.sort(key=lambda x: x[4])
print('pair, order, ms, SPPS/ISM, distance of the reflection point to its polygon edge (m)')
for x in rows:
    print(f'  {x[0]} {x[1]} {x[2]:6.1f}  {x[3]:.3f}  {x[4]:.3f}')
near = [x[3] for x in rows if x[4] < 0.5]; far = [x[3] for x in rows if x[4] >= 0.5]
print(f'edge < 0.5 m: n {len(near)}, |log ratio| median {np.median(np.abs(np.log10(near))) * 10 if near else float("nan"):.2f} dB;  '
      f'edge >= 0.5 m: n {len(far)}, |log ratio| median {np.median(np.abs(np.log10(far))) * 10 if far else float("nan"):.2f} dB')
