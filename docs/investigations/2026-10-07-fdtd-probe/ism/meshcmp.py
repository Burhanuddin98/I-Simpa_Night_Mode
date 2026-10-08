"""Is the surface SPPS reflects from (TetGen's boundary faces in the run folder) the .simpa geometry the image
sources use? Area per boundary marker against area per surface group, and 20 000 random points on the .simpa
triangles: distance to the nearest TetGen boundary face (should be ~0 everywhere).

  python meshcmp.py <run folder>
"""
import json, sys
from pathlib import Path

import numpy as np

run = Path(sys.argv[1])
p = json.loads(Path(r'C:\repos\Room-Acoustics-Engine\I-Simpa FDTD\app\src-tauri\examples\bras_cr3.simpa').read_text(encoding='utf-8'))
V = np.array(p['geometry']['vertices'], float); F = np.array([f[:3] for f in p['geometry']['faces']]); G = np.array([f[3] for f in p['geometry']['faces']])


def read_tet(stem):
    node = np.loadtxt(str(stem) + '.node', comments='#', skiprows=1)
    face = np.loadtxt(str(stem) + '.face', comments='#', skiprows=1).astype(int)
    first = int(node[0, 0])
    X = {int(r[0]): r[1:4] for r in node}
    tri = np.array([[X[a], X[b], X[c]] for a, b, c in face[:, 1:4]])
    marker = face[:, 4] if face.shape[1] > 4 else np.zeros(len(face), int)
    return tri, marker, first


tri, marker, _ = read_tet(run / 'mesh' / 'scene_mesh.1')
ta = 0.5 * np.linalg.norm(np.cross(tri[:, 1] - tri[:, 0], tri[:, 2] - tri[:, 0]), axis=1)
a = V[F[:, 0]]; b = V[F[:, 1]]; c = V[F[:, 2]]
sa = 0.5 * np.linalg.norm(np.cross(b - a, c - a), axis=1)
print(f'.simpa: {len(F)} triangles, {sa.sum():.2f} m2   TetGen boundary: {len(tri)} faces, {ta.sum():.2f} m2')
print('TetGen area per marker:', {int(m): round(float(ta[marker == m].sum()), 2) for m in np.unique(marker)})
print('.simpa area per group :', {str(g)[:8]: round(float(sa[G == g].sum()), 2) for g in np.unique(G)})

rng = np.random.default_rng(0)
k = rng.choice(len(F), 20000, p=sa / sa.sum())
u, v = rng.random(20000), rng.random(20000); flip = u + v > 1; u[flip], v[flip] = 1 - u[flip], 1 - v[flip]
P = a[k] + u[:, None] * (b[k] - a[k]) + v[:, None] * (c[k] - a[k])
# distance point -> TetGen triangle planes restricted to triangles (approximate: plane distance where the projection
# falls inside, else vertex distance)
A0, E1, E2 = tri[:, 0], tri[:, 1] - tri[:, 0], tri[:, 2] - tri[:, 0]
n = np.cross(E1, E2); n /= np.linalg.norm(n, axis=1)[:, None]
d00, d01, d11 = (E1 * E1).sum(1), (E1 * E2).sum(1), (E2 * E2).sum(1); den = d00 * d11 - d01 ** 2
best = np.full(len(P), np.inf)
for s in range(0, len(P), 200):
    Q = P[s:s + 200, None, :] - A0[None]
    h = (Q * n[None]).sum(2)
    proj = Q - h[..., None] * n[None]
    d20, d21 = (proj * E1[None]).sum(2), (proj * E2[None]).sum(2)
    vv = (d11 * d20 - d01 * d21) / den; ww = (d00 * d21 - d01 * d20) / den
    ins = (vv >= -1e-6) & (ww >= -1e-6) & (vv + ww <= 1 + 1e-6)
    dist = np.where(ins, np.abs(h), np.inf)
    best[s:s + 200] = dist.min(1)
gk = G[k]
print(f'points on .simpa triangles with no TetGen face within 1 mm: {(best > 1e-3).sum()} of {len(P)}')
for g in np.unique(gk):
    m = gk == g
    print(f'   group {str(g)[:8]}: {(best[m] > 1e-3).sum()} of {m.sum()} (max dist {np.where(np.isfinite(best[m]), best[m], -1).max():.4f} m)')
