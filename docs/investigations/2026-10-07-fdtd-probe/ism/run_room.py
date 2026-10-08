"""Image-source arrivals for a BRAS room, every source x receiver of the SPPS run, saved to arrivals_<ROOM>.npz.

  python run_room.py <ROOM> <results json> <order_exh> <order_ray> <n_rays>

Reports: candidates per order, the ray-guided set's recall against the exhaustive set at orders <= order_exh,
and how many valid arrivals of the highest order fall before t_tr = sqrt(V) ms (the early part's completeness).
"""
import json, sys, time
from pathlib import Path

import numpy as np

from ism import Room

room_name, res_file, order_exh, order_ray, n_rays = sys.argv[1], Path(sys.argv[2]), int(sys.argv[3]), int(sys.argv[4]), int(sys.argv[5])
ONLY = sys.argv[6] if len(sys.argv) > 6 else None          # restrict to one source (convergence runs)
TAG = f'_{n_rays // 1000}k_{ONLY}' if ONLY else ''
EX = Path(r'C:\repos\Room-Acoustics-Engine\I-Simpa FDTD\app\src-tauri\examples')
p = json.loads((EX / f'bras_{room_name.lower()}.simpa').read_text(encoding='utf-8'))
assert not p.get('active_variant') or room_name != 'CR4' or True   # base materials are read below, never a variant
V = np.array(p['geometry']['vertices'], float)
F = np.array([f[:3] for f in p['geometry']['faces']], int)
G = [f[3] for f in p['geometry']['faces']]
d = json.loads(res_file.read_text())
s = d['spps']
c = s['speed_of_sound_m_s']
vol = s['reference']['volume_m3']
t_tr = np.sqrt(vol) * 1e-3

t0 = time.time()
room = Room(V, F, G)
print(f'{room_name}: {room.P} planes, V {vol:.0f} m3, t_tr {1e3 * t_tr:.1f} ms, c {c:.2f} m/s')
rows = []
for src in [x for x in s['sources'] if ONLY is None or x['name'] == ONLY]:
    S = np.array(src['position_m'], float)
    exh = room.exhaustive(S, order_exh)
    rs = room.ray_sequences(S, order_ray, n_rays)
    by_k = {k: np.array(sorted(x for x in rs if len(x) == k), int).reshape(-1, k) for k in range(1, order_ray + 1)}
    # union: exhaustive below/at order_exh, ray-guided above
    cand = [exh[k] if k <= order_exh else by_k[k] for k in range(1, order_ray + 1)]
    print(f'  {src["name"]}: candidates per order {[len(x) for x in cand]} ({time.time() - t0:.0f} s)')
    for r in s['point_receivers']:
        R = np.array(r['position_m'], float)
        direct_ok = not room.blocked(S[None], R[None])[0]
        rec = []
        if direct_ok:
            rec.append((0, (), float(np.linalg.norm(S - R))))
        for k, seq in enumerate(cand, 1):
            if len(seq) == 0:
                continue
            ok, L, _ = room.validate(S, R, seq)
            rec += [(k, tuple(int(x) for x in seq[i]), float(L[i])) for i in np.where(ok)[0]]
        # recall of the ray set at exhaustive orders
        recall = []
        for k in range(1, order_exh + 1):
            ve = {x[1] for x in rec if x[0] == k}
            rk = {tuple(x) for x in by_k[k]} if k <= order_ray else set()
            recall.append(f'{len(ve & rk)}/{len(ve)}')
        top = [x for x in rec if x[0] == order_ray]
        early_top = sum(1 for x in top if x[2] / c < t_tr)
        cnt = [sum(1 for x in rec if x[0] == k) for k in range(order_ray + 1)]
        print(f'    {r["label"]}: valid per order {cnt}, ray recall at orders 1..{order_exh} {recall}, '
              f'order-{order_ray} arrivals before t_tr {early_top}, direct {"open" if direct_ok else "BLOCKED"}')
        for k, planes, L in rec:
            rows.append((src['name'], r['label'], k, L, json.dumps(planes)))
print(f'total {time.time() - t0:.0f} s')
out = Path(__file__).parent / f'arrivals_{room_name}{TAG}.npz'
np.savez(out, src=[x[0] for x in rows], rcv=[x[1] for x in rows], order=[x[2] for x in rows],
         length=[x[3] for x in rows], planes=[x[4] for x in rows], plane_group=np.array(room.pg),
         t_tr=t_tr, c=c, n_planes=room.P, order_exh=order_exh, order_ray=order_ray, n_rays=n_rays)
print('saved', out)
