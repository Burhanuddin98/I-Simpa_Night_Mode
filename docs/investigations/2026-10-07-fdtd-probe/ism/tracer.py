"""Specular GPU ray tracer that logs the plane sequence of every ray passing a receiver: finds the paths the image
sources miss. Rays from S (Fibonacci sphere), nearest hit by the fused float32 kernel, specular reflection; at every
segment, the closest approach to each receiver; a pass within r_log is logged with its path length and the planes
hit before it. A logged path with no image-source arrival within dL of its length is a missing path; its plane
sequence is then run through Room.validate, and the reason it fails is printed.

  python tracer.py <ROOM> <SRC> <RCV,RCV,...> <n_rays> <max order> <t_max ms> [r_log m]
"""
import json, os, sys
from collections import defaultdict
from pathlib import Path

import numpy as np
import cupy as cp

from ism import Room, kernel

ROOM, SRC, RCVS, N, KMAX, TMAX = sys.argv[1], sys.argv[2], sys.argv[3].split(','), int(sys.argv[4]), int(sys.argv[5]), float(sys.argv[6]) * 1e-3
R_LOG = float(sys.argv[7]) if len(sys.argv) > 7 else 0.05
HERE = Path(__file__).parent
p = json.loads((Path(r'C:\repos\Room-Acoustics-Engine\I-Simpa FDTD\app\src-tauri\examples') / f'bras_{ROOM.lower()}.simpa').read_text(encoding='utf-8'))
V = np.array(p['geometry']['vertices'], float); F = np.array([f[:3] for f in p['geometry']['faces']]); G = [f[3] for f in p['geometry']['faces']]
room = Room(V, F, G)
mog = {g['id']: g['material'] for g in p['surface_groups']}; mname = {m['id']: m['name'].split('_')[-1] for m in p['materials']}
S = np.array(next(x['position'] for x in p['sources'] if x['name'] == SRC), float)
Rs = {r: np.array(next(x['position'] for x in p['point_receivers'] if x['name'] == r), float) for r in RCVS}
c = 343.2
z = np.load(HERE / f'arrivals_{ROOM}.npz')
assert int(z['n_planes']) == room.P

g = room.dev()
A32, E132, E232 = (cp.ascontiguousarray(g[k].astype(cp.float32)) for k in ('A', 'E1', 'E2'))
TN, TP = g['tri_n'], g['tri_plane']
T = len(room.A)
logs = defaultdict(list)            # rcv -> [(L, seq tuple, closest)]
BATCH = 4_000_000
for b0 in range(0, N, BATCH):
    n = min(BATCH, N - b0)
    i = cp.arange(b0, b0 + n, dtype=cp.float64) + 0.5
    phi = cp.arccos(1 - 2 * i / N); th = np.pi * (1 + 5 ** 0.5) * i
    D = cp.stack([cp.cos(th) * cp.sin(phi), cp.sin(th) * cp.sin(phi), cp.cos(phi)], 1)
    O = cp.repeat(cp.asarray(S)[None], n, 0)
    Ltot = cp.zeros(n); seq = cp.full((n, KMAX), -1, cp.int64)
    alive = cp.arange(n)
    for k in range(KMAX + 1):
        K = len(alive)
        if K == 0:
            break
        tmin = cp.empty(K, cp.float32); tri = cp.empty(K, cp.int32)
        kernel('nearest_f32')(((K + 255) // 256,), (256,),
                              (cp.ascontiguousarray(O[alive].astype(cp.float32)), cp.ascontiguousarray(D[alive].astype(cp.float32)),
                               A32, E132, E232, np.int32(T), np.int32(K), tmin, tri))
        hit = tri >= 0
        seglen = cp.where(hit, tmin.astype(cp.float64), 1e9)
        Oa, Da, La = O[alive], D[alive], Ltot[alive]
        for rn, R in Rs.items():
            w = cp.asarray(R)[None] - Oa
            s = cp.clip((w * Da).sum(1), 0, seglen)
            close = cp.linalg.norm(w - s[:, None] * Da, axis=1)
            m = (close < R_LOG) & ((La + s) / c < TMAX)
            if int(m.sum()):
                idx = cp.where(m)[0]
                Lh, ch, sq = cp.asnumpy(La[idx] + s[idx]), cp.asnumpy(close[idx]), cp.asnumpy(seq[alive[idx], :k])
                logs[rn] += [(float(a), tuple(int(x) for x in q), float(cc)) for a, q, cc in zip(Lh, sq, ch)]
        if k == KMAX:
            break
        keep = hit & ((La + seglen) / c < TMAX)
        alive_k, tri_k, t_k = alive[keep], tri[keep].astype(cp.int64), tmin[keep].astype(cp.float64)
        O[alive_k] = O[alive_k] + t_k[:, None] * D[alive_k]
        Ltot[alive_k] = Ltot[alive_k] + t_k
        nn = TN[tri_k]
        D[alive_k] = D[alive_k] - 2 * (D[alive_k] * nn).sum(1)[:, None] * nn
        seq[alive_k, k] = TP[tri_k]
        alive = alive_k


def names(sq):
    return [mname[mog[room.pg[q]]] for q in sq]


for rn, ev in logs.items():
    sel = (z['src'] == SRC) & (z['rcv'] == rn)
    Lism = np.sort(z['length'][sel])
    # group logged rays by plane sequence: one physical path is many rays
    by = defaultdict(list)
    for L, sq, cc in ev:
        by[sq].append((L, cc))
    paths = sorted(((np.mean([x[0] for x in v]), sq, len(v), min(x[1] for x in v)) for sq, v in by.items()), key=lambda x: x[0])
    missing = [x for x in paths if not len(Lism) or np.min(np.abs(Lism - x[0])) > 0.05]
    print(f'{ROOM} {SRC}-{rn}: {len(paths)} distinct traced paths within {R_LOG} m of the receiver before {1e3 * TMAX:.0f} ms; '
          f'{len(missing)} have no image-source arrival within 5 cm of their length')
    for L, sq, nr, cc in missing[:25]:
        ok, Lv, _ = room.validate(S, Rs[rn], np.array([sq], int)) if len(sq) else (np.array([True]), None, None)
        print(f'   {1e3 * L / c:7.2f} ms  order {len(sq)}  rays {nr:4d}  closest {cc:.3f} m  {names(sq)}  '
              f'ISM validate on this sequence: {"VALID" if ok[0] else "rejected"}  planes {list(sq)}')
