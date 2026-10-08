"""Polyhedral image-source method with exact validity (visibility) checks. Written from scratch for PREREG-5.

Room = triangles, each with a surface group. Coplanar triangles of one group merge into a plane; the sign of a
plane's normal is canonical, so a double-sided face is one plane reflecting from either side (every BRAS material
is double-sided). A candidate is a sequence of planes; it is a valid specular path from S to R when the backtrace
from R through the images hits each plane inside one of its triangles, in order, and no segment of the path is
blocked by any triangle.

Candidates: exhaustive up to `order_exh` (all plane sequences, no plane twice in a row), and ray-guided above it
(plane sequences hit by M specular rays from the source). The exhaustive set at low orders is the ray set's recall
check.
"""
import os

import numpy as np

try:                       # the ray and occlusion kernels run on the GPU when CuPy is there (ISM_GPU=0 forces CPU)
    import cupy as cp
    GPU = os.environ.get('ISM_GPU', '1') != '0'
except ImportError:
    cp, GPU = None, False
xp = cp if GPU else np


def host(a):
    return a.get() if GPU and isinstance(a, cp.ndarray) else np.asarray(a)


# Fused kernels: one thread per ray or segment, looping over the triangles (no ray x triangle temporaries).
# nearest_f32: the ray search only proposes candidates, so float32 (the RTX 2060 runs float64 at 1/32 rate).
# anyhit_f64: occlusion decides validity, so it keeps float64 and the CPU path's tolerances.
_SRC = r'''
extern "C" __global__ void nearest_f32(const float* O, const float* D, const float* A, const float* E1,
                                       const float* E2, int T, int K, float* tmin, int* tri) {
    int i = blockIdx.x * blockDim.x + threadIdx.x; if (i >= K) return;
    float ox = O[3*i], oy = O[3*i+1], oz = O[3*i+2], dx = D[3*i], dy = D[3*i+1], dz = D[3*i+2];
    float best = 1e30f; int bi = -1;
    for (int t = 0; t < T; ++t) {
        float e1x = E1[3*t], e1y = E1[3*t+1], e1z = E1[3*t+2], e2x = E2[3*t], e2y = E2[3*t+1], e2z = E2[3*t+2];
        float hx = dy*e2z - dz*e2y, hy = dz*e2x - dx*e2z, hz = dx*e2y - dy*e2x;
        float det = hx*e1x + hy*e1y + hz*e1z;
        if (fabsf(det) < 1e-12f) continue;
        float inv = 1.0f / det;
        float sx = ox - A[3*t], sy = oy - A[3*t+1], sz = oz - A[3*t+2];
        float u = (sx*hx + sy*hy + sz*hz) * inv; if (u < 0.f || u > 1.f) continue;
        float qx = sy*e1z - sz*e1y, qy = sz*e1x - sx*e1z, qz = sx*e1y - sy*e1x;
        float v = (dx*qx + dy*qy + dz*qz) * inv; if (v < 0.f || u + v > 1.f) continue;
        float tt = (e2x*qx + e2y*qy + e2z*qz) * inv;
        if (tt > 1e-4f && tt < best) { best = tt; bi = t; }
    }
    tmin[i] = bi >= 0 ? best : -1.f; tri[i] = bi;
}
extern "C" __global__ void anyhit_f64(const double* P0, const double* P1, const double* A, const double* E1,
                                      const double* E2, int T, int K, double eps_t, double eps_abs, bool* out) {
    int i = blockIdx.x * blockDim.x + threadIdx.x; if (i >= K) return;
    double ox = P0[3*i], oy = P0[3*i+1], oz = P0[3*i+2];
    double dx = P1[3*i] - ox, dy = P1[3*i+1] - oy, dz = P1[3*i+2] - oz;
    bool hit = false;
    for (int t = 0; t < T && !hit; ++t) {
        double e1x = E1[3*t], e1y = E1[3*t+1], e1z = E1[3*t+2], e2x = E2[3*t], e2y = E2[3*t+1], e2z = E2[3*t+2];
        double hx = dy*e2z - dz*e2y, hy = dz*e2x - dx*e2z, hz = dx*e2y - dy*e2x;
        double det = hx*e1x + hy*e1y + hz*e1z;
        if (fabs(det) <= 1e-14) continue;
        double inv = 1.0 / det;
        double sx = ox - A[3*t], sy = oy - A[3*t+1], sz = oz - A[3*t+2];
        double u = (sx*hx + sy*hy + sz*hz) * inv; if (u < -1e-9 || u > 1 + 1e-9) continue;
        double qx = sy*e1z - sz*e1y, qy = sz*e1x - sx*e1z, qz = sx*e1y - sy*e1x;
        double v = (dx*qx + dy*qy + dz*qz) * inv; if (v < -1e-9 || u + v > 1 + 1e-9) continue;
        double tt = (e2x*qx + e2y*qy + e2z*qz) * inv;
        double len = sqrt(dx*dx + dy*dy + dz*dz);
        if (tt > eps_t && tt < 1 - eps_t && tt * len > eps_abs && (1 - tt) * len > eps_abs) hit = true;
    }
    out[i] = hit;
}
'''
_K = {}


def kernel(name):
    if name not in _K:
        _K[name] = cp.RawModule(code=_SRC).get_function(name)
    return _K[name]


EPS_T = 1e-7     # segment parameter margin: an endpoint on a surface is not an occluder
EPS_ABS = 2.5e-3  # m: and no occluder within this of either end. Must exceed the plane merge's off_tol (2 mm):
                  # a reflection point lies on the merged plane, its own triangle up to off_tol off it, and with
                  # a relative margin alone the reflecting triangle occluded its own reflection (found 2026-10-09
                  # 01:13 by tracer.py: CR4 LS2-MP2 first-order brickwall path, 59 rays within 2 mm, rejected)
EPS_D = 1e-6     # m: a point within this of a plane is on it


class Room:
    def __init__(self, V, F, G, ang_tol=1e-6, off_tol=1e-3):
        """Strict merge (2026-10-09 01:15): a triangle joins a plane only if its normal is parallel (either sign)
        and all three of its vertices lie within off_tol of the plane. The first merge (angle 1e-4 in cos, offset
        checked at one triangle) let a 0.8-degree tilt put far triangles 18.6 mm off their plane (CR4 plane 192,
        triangle 4660), so a reflection point sat in front of its own triangle, which then occluded it."""
        self.V, self.F, self.G = V, F, np.asarray(G)
        a, b, c = V[F[:, 0]], V[F[:, 1]], V[F[:, 2]]
        n = np.cross(b - a, c - a)
        area = 0.5 * np.linalg.norm(n, axis=1)
        keep = area > 1e-12
        self.A, self.E1, self.E2 = a[keep], (b - a)[keep], (c - a)[keep]
        self.tri_group = self.G[keep]
        n = n[keep] / (2 * area[keep])[:, None]
        # canonical sign: first nonzero component positive
        k = np.argmax(np.abs(n) > 1e-9, axis=1)
        s = np.sign(n[np.arange(len(n)), k]); n *= s[:, None]
        d = np.einsum('ij,ij->i', n, self.A)
        self.tri_n = n
        pn, pd, pg, ptris = [], [], [], []
        tri_plane = np.full(len(n), -1)
        left = np.arange(len(n))
        while len(left):
            i = left[0]
            ni, di = n[i], d[i]
            vdist = np.maximum.reduce([np.abs(self.A[left] @ ni - di), np.abs((self.A[left] + self.E1[left]) @ ni - di),
                                       np.abs((self.A[left] + self.E2[left]) @ ni - di)])
            same = (np.abs(n[left] @ ni) > 1 - ang_tol) & (vdist < off_tol) & (self.tri_group[left] == self.tri_group[i])
            idx = left[same]
            tri_plane[idx] = len(pn)
            pn.append(n[i]); pd.append(d[i]); pg.append(self.tri_group[i]); ptris.append(idx)
            left = left[~same]
        self.pn, self.pd, self.pg, self.ptris, self.tri_plane = np.array(pn), np.array(pd), np.array(pg), ptris, tri_plane
        self.P = len(pn)

    # ------------------------------------------------------------ geometry kernels
    def reflect(self, X, p):
        """Mirror points X (K,3) in planes p (K,)."""
        n = self.pn[p]
        return X - 2 * (np.einsum('ij,ij->i', X, n) - self.pd[p])[:, None] * n

    def dev(self):
        """The triangle arrays on the compute device, made once."""
        if not hasattr(self, '_dev'):
            self._dev = {k: xp.asarray(getattr(self, k)) for k in ('A', 'E1', 'E2', 'tri_n', 'tri_plane')}
        return self._dev

    def blocked(self, P0, P1, chunk=8_000_000):
        """True where segment P0->P1 hits any triangle strictly between its ends (Moller-Trumbore)."""
        g = self.dev(); A, E1, E2 = g['A'], g['E1'], g['E2']
        K, T = len(P0), len(self.A)
        if GPU and K:
            out = cp.zeros(K, cp.bool_)
            kernel('anyhit_f64')(((K + 255) // 256,), (256,),
                                 (cp.ascontiguousarray(cp.asarray(P0, cp.float64)), cp.ascontiguousarray(cp.asarray(P1, cp.float64)),
                                  A, E1, E2, np.int32(T), np.int32(K), np.float64(EPS_T), np.float64(EPS_ABS), out))
            return host(out)
        out = np.zeros(K, bool)
        step = max(1, chunk // T)
        P0d, Dd = xp.asarray(P0), xp.asarray(P1 - P0)
        for s in range(0, K, step):
            o, D = P0d[s:s + step, None, :], Dd[s:s + step, None, :]
            h = xp.cross(D, E2[None])
            det = xp.einsum('ktj,tj->kt', h, E1)
            ok = xp.abs(det) > 1e-14
            inv = xp.where(ok, 1.0 / xp.where(ok, det, 1.0), 0.0)
            sv = o - A[None]
            u = xp.einsum('ktj,ktj->kt', sv, h) * inv
            q = xp.cross(sv, E1[None])
            v = xp.einsum('ktj,ktj->kt', D, q) * inv
            t = xp.einsum('tj,ktj->kt', E2, q) * inv
            Ls = xp.linalg.norm(D, axis=2)
            hit = (ok & (u >= -1e-9) & (v >= -1e-9) & (u + v <= 1 + 1e-9) & (t > EPS_T) & (t < 1 - EPS_T)
                   & (t * Ls > EPS_ABS) & ((1 - t) * Ls > EPS_ABS))
            out[s:s + step] = host(hit.any(1))
        return out

    def plane_hit(self, P0, P1, p):
        """Segment P0->P1 crosses plane p (all rows share plane p) inside one of its triangles: (ok, Q)."""
        n, d = self.pn[p], self.pd[p]
        a0, a1 = P0 @ n - d, P1 @ n - d
        ok = (a0 * a1 < 0) & (np.abs(a0) > EPS_D) & (np.abs(a1) > EPS_D)
        t = np.where(ok, a0 / np.where(ok, a0 - a1, 1.0), 0.0)
        Q = P0 + t[:, None] * (P1 - P0)
        tri = self.ptris[p]
        A, E1, E2 = self.A[tri], self.E1[tri], self.E2[tri]
        w = Q[:, None, :] - A[None]
        d00, d01, d11 = (E1 * E1).sum(1), (E1 * E2).sum(1), (E2 * E2).sum(1)
        d20, d21 = np.einsum('ktj,tj->kt', w, E1), np.einsum('ktj,tj->kt', w, E2)
        den = d00 * d11 - d01 ** 2
        v = (d11 * d20 - d01 * d21) / den
        u = (d00 * d21 - d01 * d20) / den
        inside = ((v >= -1e-9) & (u >= -1e-9) & (u + v <= 1 + 1e-9)).any(1)
        return ok & inside, Q

    # ------------------------------------------------------------ candidates
    def exhaustive(self, S, order):
        """All plane sequences up to `order`, no plane twice in a row, each image off its next plane."""
        seqs = [np.zeros((1, 0), int)]
        imgs = [S[None].copy()]
        for k in range(1, order + 1):
            prev, pimg = seqs[-1], imgs[-1]
            K = len(prev)
            rep = np.repeat(np.arange(K), self.P)
            p = np.tile(np.arange(self.P), K)
            keep = np.ones(len(p), bool) if k == 1 else (p != prev[rep, -1])
            dist = np.abs(np.einsum('ij,ij->i', pimg[rep], self.pn[p]) - self.pd[p])
            keep &= dist > EPS_D
            rep, p = rep[keep], p[keep]
            seqs.append(np.concatenate([prev[rep], p[:, None]], 1))
            imgs.append(self.reflect(pimg[rep], p))
        return seqs

    def ray_sequences(self, S, order, n_rays, seed=0, chunk=8_000_000):
        """Plane sequences (as tuples) hit by n_rays specular rays from S, all prefixes up to `order`."""
        g = self.dev(); A, E1, E2, TN, TP = g['A'], g['E1'], g['E2'], g['tri_n'], g['tri_plane']
        i = xp.arange(n_rays) + 0.5
        phi = xp.arccos(1 - 2 * i / n_rays); th = np.pi * (1 + 5 ** 0.5) * i
        D = xp.stack([xp.cos(th) * xp.sin(phi), xp.sin(th) * xp.sin(phi), xp.cos(phi)], 1)
        O = xp.repeat(xp.asarray(S)[None], n_rays, 0)
        seq = xp.full((n_rays, order), -1)
        alive = xp.ones(n_rays, bool)
        T = len(self.A); step = max(1, chunk // T)
        if GPU:
            A32, E132, E232 = (cp.ascontiguousarray(x.astype(cp.float32)) for x in (A, E1, E2))
        for k in range(order):
            idx = xp.where(alive)[0]
            if GPU:
                K = len(idx)
                tmin = cp.empty(K, cp.float32); tri = cp.empty(K, cp.int32)
                kernel('nearest_f32')(((K + 255) // 256,), (256,),
                                      (cp.ascontiguousarray(O[idx].astype(cp.float32)),
                                       cp.ascontiguousarray(D[idx].astype(cp.float32)),
                                       A32, E132, E232, np.int32(T), np.int32(K), tmin, tri))
                tmin = cp.where(tri >= 0, tmin.astype(cp.float64), cp.inf); tri = tri.astype(cp.int64)
            else:
                tmin = xp.full(len(idx), xp.inf); tri = xp.full(len(idx), -1)
            for s in (range(0, len(idx), step) if not GPU else ()):
                o, dd = O[idx[s:s + step], None, :], D[idx[s:s + step], None, :]
                h = xp.cross(dd, E2[None]); det = xp.einsum('ktj,tj->kt', h, E1)
                ok = xp.abs(det) > 1e-14
                inv = xp.where(ok, 1.0 / xp.where(ok, det, 1.0), 0.0)
                sv = o - A[None]
                u = xp.einsum('ktj,ktj->kt', sv, h) * inv
                q = xp.cross(sv, E1[None]); v = xp.einsum('ktj,ktj->kt', dd, q) * inv
                t = xp.einsum('tj,ktj->kt', E2, q) * inv
                t = xp.where(ok & (u >= 0) & (v >= 0) & (u + v <= 1) & (t > 1e-6), t, xp.inf)
                j = xp.argmin(t, 1)
                tmin[s:s + step] = t[xp.arange(len(j)), j]; tri[s:s + step] = j
            hit = xp.isfinite(tmin)
            alive[idx[~hit]] = False
            idx, tmin, tri = idx[hit], tmin[hit], tri[hit]
            seq[idx, k] = TP[tri]
            O[idx] = O[idx] + tmin[:, None] * D[idx]
            n = TN[tri]
            D[idx] = D[idx] - 2 * xp.einsum('ij,ij->i', D[idx], n)[:, None] * n
        seq = host(seq)
        out = set()
        for k in range(1, order + 1):
            s = seq[:, :k]
            s = s[(s >= 0).all(1)]
            # a ray that hit two coplanar triangles of one plane in a row is a numerical double hit: drop
            if k > 1:
                s = s[(s[:, 1:] != s[:, :-1]).all(1)]
            out |= set(map(tuple, np.unique(s, axis=0)))
        return out

    # ------------------------------------------------------------ validation
    def plane_table(self):
        """Per plane, its triangle indices padded with -1 to the longest plane, on the device."""
        if not hasattr(self, '_ptab'):
            M = max(len(t) for t in self.ptris)
            tab = np.full((self.P, M), -1)
            for i, t in enumerate(self.ptris):
                tab[i, :len(t)] = t
            self._ptab = xp.asarray(tab)
            self._pn_d, self._pd_d = xp.asarray(self.pn), xp.asarray(self.pd)
        return self._ptab

    def validate(self, S, R, seq, chunk=8_000_000):
        """seq (K,k) plane indices. Returns (valid mask, path length, image) for the paths S -> planes -> R.
        On the GPU every candidate is tested at once (in chunks); on the CPU, plane by plane (the reference)."""
        if not GPU:
            return self.validate_cpu(S, R, seq)
        tab = self.plane_table(); g = self.dev(); A, E1, E2 = g['A'], g['E1'], g['E2']
        pn, pd = self._pn_d, self._pd_d
        K, k = seq.shape
        sq = xp.asarray(seq)
        imgs = [xp.repeat(xp.asarray(S)[None], K, 0)]
        for j in range(k):
            n = pn[sq[:, j]]
            imgs.append(imgs[-1] - 2 * ((imgs[-1] * n).sum(1) - pd[sq[:, j]])[:, None] * n)
        valid = xp.ones(K, bool)
        cur = xp.repeat(xp.asarray(R)[None], K, 0)
        pts = [cur]
        step = max(1, chunk // tab.shape[1])
        for j in range(k, 0, -1):
            p = sq[:, j - 1]; n = pn[p]; d = pd[p]
            a0, a1 = (cur * n).sum(1) - d, (imgs[j] * n).sum(1) - d
            ok = (a0 * a1 < 0) & (xp.abs(a0) > EPS_D) & (xp.abs(a1) > EPS_D)
            t = xp.where(ok, a0 / xp.where(ok, a0 - a1, 1.0), 0.0)
            Q = cur + t[:, None] * (imgs[j] - cur)
            inside = xp.zeros(K, bool)
            for s in range(0, K, step):
                tr = tab[p[s:s + step]]                       # (k, M), -1 padded
                m = tr >= 0; tr = xp.where(m, tr, 0)
                w = Q[s:s + step, None, :] - A[tr]
                e1, e2 = E1[tr], E2[tr]
                d00, d01, d11 = (e1 * e1).sum(2), (e1 * e2).sum(2), (e2 * e2).sum(2)
                d20, d21 = (w * e1).sum(2), (w * e2).sum(2)
                den = d00 * d11 - d01 ** 2
                vv = (d11 * d20 - d01 * d21) / den; uu = (d00 * d21 - d01 * d20) / den
                inside[s:s + step] = (m & (vv >= -1e-9) & (uu >= -1e-9) & (uu + vv <= 1 + 1e-9)).any(1)
            valid &= ok & inside
            pts.append(Q); cur = Q
        pts.append(imgs[0])
        for a, b in zip(pts[:-1], pts[1:]):
            m = xp.where(valid)[0]
            if len(m) == 0:
                break
            bl = self.blocked(host(a[m]), host(b[m]))
            valid[m[xp.asarray(bl)]] = False
        L = xp.linalg.norm(imgs[k] - xp.asarray(R), axis=1)
        return host(valid), host(L), host(imgs[k])

    def validate_cpu(self, S, R, seq):
        """seq (K,k) plane indices. Returns (valid mask, path length) for the paths S -> planes -> R."""
        K, k = seq.shape
        imgs = [np.repeat(S[None], K, 0)]
        for j in range(k):
            imgs.append(self.reflect(imgs[-1], seq[:, j]))
        valid = np.ones(K, bool)
        Rr = np.repeat(R[None], K, 0)
        pts = [Rr]
        cur = Rr
        for j in range(k, 0, -1):
            Q = np.zeros((K, 3))
            p = seq[:, j - 1]
            for pl in np.unique(p[valid]):
                m = valid & (p == pl)
                ok, q = self.plane_hit(cur[m], imgs[j][m], pl)
                Q[m] = q
                valid[np.where(m)[0][~ok]] = False
            pts.append(Q); cur = Q
        pts.append(imgs[0])
        for a, b in zip(pts[:-1], pts[1:]):
            m = np.where(valid)[0]
            if len(m) == 0:
                break
            valid[m[self.blocked(a[m], b[m])]] = False
        L = np.linalg.norm(imgs[k] - R, axis=1)
        return valid, L, imgs[k]


def arrivals(room, S, R, seqs_by_order):
    """Valid paths: list of (order, planes tuple, length, image position)."""
    out = []
    for seq in seqs_by_order:
        if len(seq) == 0:
            continue
        ok, L, img = room.validate(S, R, seq)
        for i in np.where(ok)[0]:
            out.append((seq.shape[1], tuple(int(x) for x in seq[i]), float(L[i]), img[i]))
    return out
