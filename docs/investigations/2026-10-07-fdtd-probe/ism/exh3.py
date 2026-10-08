"""Diagnosis: exhaustive order-3 image sources for one CR3 pair in a delay window, against the ray-guided set
saved in arrivals_CR3.npz (does the ray-guided generator miss order-3 paths?).

  python exh3.py <SRC> <RCV> <t_lo ms> <t_hi ms>
"""
import json, sys
from pathlib import Path

import numpy as np

from ism import Room

SRC, RCV, T0, T1 = sys.argv[1], sys.argv[2], float(sys.argv[3]), float(sys.argv[4])
p = json.loads(Path(r'C:\repos\Room-Acoustics-Engine\I-Simpa FDTD\app\src-tauri\examples\bras_cr3.simpa').read_text(encoding='utf-8'))
V = np.array(p['geometry']['vertices'], float); F = np.array([f[:3] for f in p['geometry']['faces']]); G = [f[3] for f in p['geometry']['faces']]
room = Room(V, F, G)
mog = {g['id']: g['material'] for g in p['surface_groups']}; mname = {m['id']: m['name'][8:] for m in p['materials']}
S = np.array(next(x['position'] for x in p['sources'] if x['name'] == SRC), float)
R = np.array(next(x['position'] for x in p['point_receivers'] if x['name'] == RCV), float)
c = 343.2
z = np.load(Path(__file__).parent / 'arrivals_CR3.npz')
assert int(z['n_planes']) == room.P
have = {tuple(json.loads(x)) for x, s_, r_ in zip(z['planes'], z['src'], z['rcv']) if s_ == SRC and r_ == RCV}

P = room.P
found = []
for p1 in range(P):
    i1 = room.reflect(S[None], np.array([p1]))
    p2 = np.repeat(np.arange(P), P); p3 = np.tile(np.arange(P), P)
    keep = (p2 != p1) & (p3 != p2)
    p2, p3 = p2[keep], p3[keep]
    i2 = room.reflect(np.repeat(i1, len(p2), 0), p2)
    i3 = room.reflect(i2, p3)
    L = np.linalg.norm(i3 - R, axis=1)
    w = (L / c * 1e3 >= T0) & (L / c * 1e3 <= T1)
    if not w.any():
        continue
    seq = np.stack([np.full(w.sum(), p1), p2[w], p3[w]], 1)
    ok, Lv, _ = room.validate(S, R, seq)
    for s_, l_ in zip(seq[ok], Lv[ok]):
        found.append((tuple(int(x) for x in s_), float(l_)))
miss = [f for f in found if f[0] not in have]
print(f'{SRC}-{RCV}, order 3, {T0}-{T1} ms: exhaustive valid {len(found)}, in the ray-guided set {len(found) - len(miss)}, missed {len(miss)}')
for sq, l_ in sorted(miss, key=lambda x: x[1]):
    print(f'   missed: {1e3 * l_ / c:.2f} ms  {[mname[mog[room.pg[q]]] for q in sq]}')
