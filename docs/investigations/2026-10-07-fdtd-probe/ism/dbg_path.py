"""Why does Room.validate reject this path? Backtrace a plane sequence and report every check.

  python dbg_path.py <ROOM> <SRC> <RCV> <plane,plane,...>
"""
import json, os, sys
from pathlib import Path

os.environ.setdefault('ISM_GPU', '0')
import numpy as np  # noqa: E402

from ism import Room, EPS_T, EPS_ABS, EPS_D  # noqa: E402

ROOM, SRC, RCV, SEQ = sys.argv[1], sys.argv[2], sys.argv[3], [int(x) for x in sys.argv[4].split(',')]
p = json.loads((Path(r'C:\repos\Room-Acoustics-Engine\I-Simpa FDTD\app\src-tauri\examples') / f'bras_{ROOM.lower()}.simpa').read_text(encoding='utf-8'))
V = np.array(p['geometry']['vertices'], float); F = np.array([f[:3] for f in p['geometry']['faces']]); G = [f[3] for f in p['geometry']['faces']]
room = Room(V, F, G)
mog = {g['id']: g['material'] for g in p['surface_groups']}; mn = {m['id']: m['name'].split('_')[-1] for m in p['materials']}
S = np.array(next(x['position'] for x in p['sources'] if x['name'] == SRC), float)
R = np.array(next(x['position'] for x in p['point_receivers'] if x['name'] == RCV), float)


def bary(Q, t):
    A, E1, E2 = room.A[t], room.E1[t], room.E2[t]
    n = np.cross(E1, E2); n /= np.linalg.norm(n); h = (Q - A) @ n; w = Q - A - h * n
    d00, d01, d11 = E1 @ E1, E1 @ E2, E2 @ E2; d20, d21 = w @ E1, w @ E2; den = d00 * d11 - d01 ** 2
    v = (d11 * d20 - d01 * d21) / den; u = (d00 * d21 - d01 * d20) / den
    return u, v, h


def blockers(P0, P1):
    D = P1 - P0; L = np.linalg.norm(D)
    h = np.cross(D, room.E2); det = (h * room.E1).sum(1); ok = np.abs(det) > 1e-14; inv = np.where(ok, 1 / np.where(ok, det, 1), 0)
    sv = P0 - room.A; u = (sv * h).sum(1) * inv; q = np.cross(sv, room.E1); v = (q * D).sum(1) * inv; t = (room.E2 * q).sum(1) * inv
    m = ok & (u >= -1e-9) & (v >= -1e-9) & (u + v <= 1 + 1e-9) & (t > EPS_T) & (t < 1 - EPS_T) & (t * L > EPS_ABS) & ((1 - t) * L > EPS_ABS)
    return [(int(i), int(room.tri_plane[i]), mn[mog[room.tri_group[i]]], round(float(t[i] * L), 4), round(float(L), 3),
             round(float(min(u[i], v[i], 1 - u[i] - v[i])), 5)) for i in np.where(m)[0]]


imgs = [S]
for q in SEQ:
    n = room.pn[q]; imgs.append(imgs[-1] - 2 * (imgs[-1] @ n - room.pd[q]) * n)
print(f'{ROOM} {SRC}-{RCV} planes {SEQ} {[mn[mog[room.pg[q]]] for q in SEQ]}: length {np.linalg.norm(imgs[-1] - R):.3f} m')
cur, pts = R, [R]
for j in range(len(SEQ), 0, -1):
    q = SEQ[j - 1]; n, d = room.pn[q], room.pd[q]
    a0, a1 = cur @ n - d, imgs[j] @ n - d
    print(f' plane {q}: cur side {a0:+.4f}  image side {a1:+.4f}  -> {"crosses" if a0 * a1 < 0 else "NO CROSSING"}')
    Q = cur + a0 / (a0 - a1) * (imgs[j] - cur)
    best = min(((max(-u, -v, u + v - 1, 0), t, h) for t in room.ptris[q] for u, v, h in [bary(Q, t)]), key=lambda x: x[0])
    print(f'   Q {np.round(Q, 4)}: nearest triangle of the plane {best[1]}, outside by (barycentric) {best[0]:.2e}, off-plane {best[2]:+.2e} m')
    ok, _ = room.plane_hit(cur[None], imgs[j][None], q); print(f'   plane_hit: {bool(ok[0])}')
    pts.append(Q); cur = Q
pts.append(S)
for a, b in zip(pts[:-1], pts[1:]):
    bl = blockers(a, b)
    print(f' segment {np.round(a, 2)} -> {np.round(b, 2)}: {"clear" if not bl else bl}')
ok, L, _ = room.validate(S, R, np.array([SEQ])); print('validate:', bool(ok[0]))
