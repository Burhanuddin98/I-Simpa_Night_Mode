"""Independent check of SPPS: a from-scratch energy ray tracer with fully diffuse (Lambert, scattering 1) walls.

Same mesh and base materials as Night Mode's example (no variant), same air absorption per metre as SPPS's
reference. Rays leave the source uniformly; each wall hit multiplies the ray's energy by (1 - alpha) and sends
it off in a cosine-weighted direction about the wall normal facing the ray. The room-wide energy E(t), the
sum over rays alive at t, gives T30 directly (-5..-35 dB), with no receiver sampling at all.

  python vtracer.py CR4 [rays]
Imports nothing from Night Mode or PFFDTD.
"""
import json, math, sys, time
from pathlib import Path

import numpy as np
from numba import njit, prange

ROOM = sys.argv[1]
NRAYS = int(sys.argv[2]) if len(sys.argv) > 2 else 20000
SIMPA = Path(r'C:\repos\Room-Acoustics-Engine\I-Simpa FDTD\app\src-tauri\examples') / f'bras_{ROOM.lower()}.simpa'
C = 343.2
DT = 0.001
TMAX = 6.0
BANDS = [125, 250, 500]
AIR = {125: 0.00010127115679716098}   # SPPS's reference value at 125 Hz (m^-1, energy); others read below


@njit(cache=True)
def hit(o, d, A, E1, E2):
    best_t = 1e30; best = -1
    for k in range(A.shape[0]):
        hx = d[1] * E2[k, 2] - d[2] * E2[k, 1]
        hy = d[2] * E2[k, 0] - d[0] * E2[k, 2]
        hz = d[0] * E2[k, 1] - d[1] * E2[k, 0]
        det = E1[k, 0] * hx + E1[k, 1] * hy + E1[k, 2] * hz
        if abs(det) < 1e-14:
            continue
        inv = 1.0 / det
        sx = o[0] - A[k, 0]; sy = o[1] - A[k, 1]; sz = o[2] - A[k, 2]
        u = (sx * hx + sy * hy + sz * hz) * inv
        if u < 0.0 or u > 1.0:
            continue
        qx = sy * E1[k, 2] - sz * E1[k, 1]
        qy = sz * E1[k, 0] - sx * E1[k, 2]
        qz = sx * E1[k, 1] - sy * E1[k, 0]
        v = (d[0] * qx + d[1] * qy + d[2] * qz) * inv
        if v < 0.0 or u + v > 1.0:
            continue
        t = (E2[k, 0] * qx + E2[k, 1] * qy + E2[k, 2] * qz) * inv
        if t > 1e-7 and t < best_t:
            best_t = t; best = k
    return best, best_t


@njit(cache=True)
def one_ray(i, src, A, E1, E2, N, alpha, m, nbins, dt, c, seed, row):
    np.random.seed(seed + i)
    o = np.empty(3); d = np.empty(3)
    z = 2.0 * np.random.random() - 1.0; ph = 2.0 * math.pi * np.random.random(); r = math.sqrt(1.0 - z * z)
    d[0] = r * math.cos(ph); d[1] = r * math.sin(ph); d[2] = z
    o[0] = src[0]; o[1] = src[1]; o[2] = src[2]
    w = 1.0; t = 0.0
    while t < nbins * dt and w > 1e-9:
        k, s = hit(o, d, A, E1, E2)
        if k < 0:
            return w
        t1 = t + s / c
        b0 = int(t / dt); b1 = min(int(t1 / dt), nbins - 1)
        for b in range(b0, b1 + 1):            # energy alive in each bin, with air loss along the path
            tm = (b + 0.5) * dt
            if tm >= t and tm <= t1:
                row[b] += w * math.exp(-m * c * (tm - t))
        w *= math.exp(-m * s) * (1.0 - alpha[k])
        o[0] += s * d[0]; o[1] += s * d[1]; o[2] += s * d[2]; t = t1
        nx = N[k, 0]; ny = N[k, 1]; nz = N[k, 2]
        if nx * d[0] + ny * d[1] + nz * d[2] > 0.0:
            nx = -nx; ny = -ny; nz = -nz
        # tangent frame about n, then a cosine-weighted direction
        if abs(nx) < 0.9:
            ax, ay, az = 1.0, 0.0, 0.0
        else:
            ax, ay, az = 0.0, 1.0, 0.0
        tx = ny * az - nz * ay; ty = nz * ax - nx * az; tz = nx * ay - ny * ax
        tl = math.sqrt(tx * tx + ty * ty + tz * tz); tx /= tl; ty /= tl; tz /= tl
        bx = ny * tz - nz * ty; by = nz * tx - nx * tz; bz = nx * ty - ny * tx
        u1 = np.random.random(); u2 = np.random.random()
        rr = math.sqrt(u1); th = 2.0 * math.pi * u2; cn = math.sqrt(1.0 - u1)
        d[0] = rr * math.cos(th) * tx + rr * math.sin(th) * bx + cn * nx
        d[1] = rr * math.cos(th) * ty + rr * math.sin(th) * by + cn * ny
        d[2] = rr * math.cos(th) * tz + rr * math.sin(th) * bz + cn * nz
        o[0] += 1e-6 * nx; o[1] += 1e-6 * ny; o[2] += 1e-6 * nz
    return 0.0


@njit(parallel=True, cache=True)
def trace(src, A, E1, E2, N, alpha, m, nrays, nbins, dt, c, seed):
    hist = np.zeros((nrays, nbins))
    lost = np.zeros(nrays)
    for i in prange(nrays):
        lost[i] = one_ray(i, src, A, E1, E2, N, alpha, m, nbins, dt, c, seed, hist[i])
    return hist.sum(axis=0), lost.sum()


p = json.loads(SIMPA.read_text(encoding='utf-8'))
V = np.array(p['geometry']['vertices'], float)
F = np.array([f[:3] for f in p['geometry']['faces']], np.int64)
g2m = {g['id']: g['material'] for g in p['surface_groups']}
mats = {m['id']: m for m in p['materials']}
bands = p['bands']['frequencies_hz']
A_, B_, C_ = V[F[:, 0]], V[F[:, 1]], V[F[:, 2]]
E1, E2 = B_ - A_, C_ - A_
Nn = np.cross(E1, E2); Nn /= np.linalg.norm(Nn, axis=1, keepdims=True)
src = np.array(p['sources'][0]['position'], float)
ref_json = Path(r'C:\tmp\nm-spps-runs') / {'CR2': 'cr2_s1.json', 'CR3': 'cr3_s1.json', 'CR4': 'cr4_base_s1.json'}[ROOM]
ref = {b['freq_hz']: b for b in json.loads(ref_json.read_text())['spps']['reference']['bands']}
spps = json.loads(ref_json.read_text())['spps']
for fc in BANDS:
    bi = bands.index(fc)
    alpha = np.array([mats[g2m[f[3]]]['absorption'][bi] for f in p['geometry']['faces']], float)
    m = ref[fc]['air_m_per_metre']
    t0 = time.time()
    hist, lost = trace(src, A_, E1, E2, Nn, alpha, m, NRAYS, int(TMAX / DT), DT, C, 12345)
    L = 10 * np.log10(hist / hist.max() + 1e-300); t = np.arange(len(L)) * DT
    sel = (L <= -5) & (L >= -35)
    T30 = -60 / np.polyfit(t[sel], L[sel], 1)[0]
    v = [next(b for b in ps['bands'] if int(b['freq_hz']) == fc)['parameters']['t30_s']['value']
         for r in spps['point_receivers'] for ps in r['per_source']]
    print(f'{ROOM} {fc} Hz: tracer T30 {T30:.2f} s | SPPS (scattering 1) {np.mean(v):.2f} s | Kuttruff '
          f'{ref[fc]["kuttruff_s"]["value"]:.2f} s | Eyring {ref[fc]["eyring_s"]["value"]:.2f} s | lost '
          f'{100 * lost / NRAYS:.3f} % | {time.time() - t0:.0f} s', flush=True)
