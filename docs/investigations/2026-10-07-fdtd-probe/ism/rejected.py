"""Diagnosis: order <= 2 image-source candidates for one pair whose delay falls in a window, and why each is
rejected (plane missed and by how much, or which segment is blocked and by which triangle's material).

  python rejected.py <SRC> <RCV> <t_lo ms> <t_hi ms>
"""
import json, sys
from pathlib import Path

import numpy as np

from ism import Room, EPS_T

SRC, RCV, T0, T1 = sys.argv[1], sys.argv[2], float(sys.argv[3]), float(sys.argv[4])
p = json.loads(Path(r'C:\repos\Room-Acoustics-Engine\I-Simpa FDTD\app\src-tauri\examples\bras_cr3.simpa').read_text(encoding='utf-8'))
V = np.array(p['geometry']['vertices'], float); F = np.array([f[:3] for f in p['geometry']['faces']]); G = [f[3] for f in p['geometry']['faces']]
room = Room(V, F, G)
mog = {g['id']: g['material'] for g in p['surface_groups']}; mname = {m['id']: m['name'][8:] for m in p['materials']}
S = np.array(next(x['position'] for x in p['sources'] if x['name'] == SRC), float)
R = np.array(next(x['position'] for x in p['point_receivers'] if x['name'] == RCV), float)
c = 343.2


def dist_to_plane_polygon(Q, pl):
    """Distance from Q (on the plane) to the nearest triangle of plane pl (0 if inside)."""
    best = np.inf
    for t in room.ptris[pl]:
        a, e1, e2 = room.A[t], room.E1[t], room.E2[t]
        # sample-free: distance to triangle = min over edges if outside
        P = [a, a + e1, a + e2]
        n = np.cross(e1, e2); inside = True
        for i in range(3):
            u, v = P[i], P[(i + 1) % 3]
            if np.dot(np.cross(v - u, Q - u), n) < 0:
                inside = False
        if inside:
            return 0.0
        for i in range(3):
            u, v = P[i], P[(i + 1) % 3]
            s = np.clip(np.dot(Q - u, v - u) / np.dot(v - u, v - u), 0, 1)
            best = min(best, np.linalg.norm(Q - (u + s * (v - u))))
    return best


def blocker(P0, P1):
    D = P1 - P0
    h = np.cross(D, room.E2); det = (h * room.E1).sum(1); ok = np.abs(det) > 1e-14
    inv = np.where(ok, 1 / np.where(ok, det, 1), 0); sv = P0 - room.A
    u = (sv * h).sum(1) * inv; q = np.cross(sv, room.E1); v = (q * D).sum(1) * inv; t = (room.E2 * q).sum(1) * inv
    hit = ok & (u >= -1e-9) & (v >= -1e-9) & (u + v <= 1 + 1e-9) & (t > EPS_T) & (t < 1 - EPS_T)
    i = np.where(hit)[0]
    return [(mname[mog[room.tri_group[j]]], round(float(t[j]) * np.linalg.norm(D), 3), round(float(min(u[j], v[j], 1 - u[j] - v[j])), 4)) for j in i[:3]]


for k, seq in enumerate(room.exhaustive(S, 2)[1:], 1):
    imgs = [np.repeat(S[None], len(seq), 0)]
    for j in range(k):
        imgs.append(room.reflect(imgs[-1], seq[:, j]))
    L = np.linalg.norm(imgs[k] - R, axis=1)
    sel = np.where((L / c * 1e3 >= T0) & (L / c * 1e3 <= T1))[0]
    ok, _, _ = room.validate(S, R, seq[sel]) if len(sel) else (np.array([], bool), None, None)
    for i, good in zip(sel, ok):
        names = [mname[mog[room.pg[q]]] for q in seq[i]]
        # backtrace by hand to say why
        cur, why = R, []
        pts = [R]
        for j in range(k, 0, -1):
            pl = seq[i, j - 1]; n, d = room.pn[pl], room.pd[pl]
            a0, a1 = cur @ n - d, imgs[j][i] @ n - d
            if a0 * a1 >= 0:
                why.append(f'no crossing of {names[j - 1]}'); break
            Q = cur + a0 / (a0 - a1) * (imgs[j][i] - cur)
            dd = dist_to_plane_polygon(Q, pl)
            if dd > 0:
                why.append(f'misses {names[j - 1]} polygon by {dd:.3f} m')
            pts.append(Q); cur = Q
        pts.append(S)
        if not why:
            for a, b in zip(pts[:-1], pts[1:]):
                bl = blocker(a, b)
                if bl:
                    why.append(f'blocked by {bl}')
        print(f'order {k} {names} t {1e3 * L[i] / c:.2f} ms valid {bool(good)}  {"; ".join(why)}')
