"""Coplanar triangle merge for the BRAS rooms: how many distinct reflecting planes each room has.

  python planes.py            prints per room: triangles, planes, planes carrying 90 % of the area
"""
import json
from pathlib import Path

import numpy as np

EX = Path(r'C:\repos\Room-Acoustics-Engine\I-Simpa FDTD\app\src-tauri\examples')


def load(room):
    p = json.loads((EX / f'bras_{room.lower()}.simpa').read_text(encoding='utf-8'))
    V = np.array(p['geometry']['vertices'], float)
    F = np.array([f[:3] for f in p['geometry']['faces']], int)
    G = np.array([f[3] for f in p['geometry']['faces']])
    return p, V, F, G


def planes(V, F, G, ang_tol=1e-3, off_tol=5e-3):
    """Group triangles by (unit normal, offset, surface group). Normals are not sign-flipped: the two faces of a
    thin panel are two planes. Returns list of (n, d, tri_idx) with n.x = d on the plane."""
    a, b, c = V[F[:, 0]], V[F[:, 1]], V[F[:, 2]]
    n = np.cross(b - a, c - a)
    area = 0.5 * np.linalg.norm(n, axis=1)
    n = n / np.maximum(2 * area, 1e-300)[:, None]
    d = np.einsum('ij,ij->i', n, a)
    out = []
    left = np.where(area > 1e-12)[0]
    while len(left):
        i = left[0]
        same = (np.einsum('ij,j->i', n[left], n[i]) > 1 - ang_tol) & (np.abs(d[left] - d[i]) < off_tol) \
            & (G[left] == G[i])
        out.append((n[i], d[i], left[same]))
        left = left[~same]
    return out, area


if __name__ == '__main__':
    for room in ['CR2', 'CR3', 'CR4']:
        p, V, F, G = load(room)
        P, area = planes(V, F, G)
        A = np.sort([area[t].sum() for _, _, t in P])[::-1]
        k90 = int(np.searchsorted(np.cumsum(A) / A.sum(), 0.9)) + 1
        print(f'{room}: {len(F)} triangles, {len(P)} planes, {k90} planes carry 90 % of the area, '
              f'largest {A[:5].round(1).tolist()} m2')
