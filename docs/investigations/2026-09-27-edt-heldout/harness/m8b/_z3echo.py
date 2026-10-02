"""Text copy of the Z3 judge's generator A and its readings, for corpus.py's z3 and z3grid rows.

Source: target/agents/z3-hunt/judge/judge.py (gitignored), sha256 1898400582323f1b, lines 57, 61-62
and 80-435, copied unchanged below the marker, the ranges joined by two blank lines. The source's
module-level imports of the registered W1G rule (composite), skeptic-gf3/ism.py and
skeptic-twostep/synth.py are left out: these lines use none of them. m8b/provenance.json records the
source hash and the ranges; corpus.py compares this copy with its source on every run and refuses to
run on a difference. Do not edit below the marker.
"""
import math

import numpy as np

# ---- copied from judge.py (do not edit below this line) ----
GX8, GW8 = np.polynomial.legendre.leggauss(8)


def f32(x):
    return float(np.float32(x))


# ------------------------------------------------------------------------------------------------
# Kernel integrals
# ------------------------------------------------------------------------------------------------
def _gl(D, a, b, R, m, mom, c):
    half = 0.5 * (b - a)
    mid = 0.5 * (a + b)
    r = mid[:, None] + half[:, None] * GX8[None, :]
    Dm = D[:, None]
    f = (R * R - (r - Dm) ** 2) / (4.0 * r * Dm)
    if m:
        f = f * np.exp(-m * r)
    if mom is not None:
        f = f * (r / c - mom)
    return (f * GW8[None, :]).sum(1) * half


def seg_int(D, a, b, R, m=0.0, mom=None, c=None, nsub=64):
    """Integral over [a, b] (clipped to the pulse [D - R, D + R]) of the ball kernel, times
    exp(-m r) when m, times (r/c - mom) when mom is given. Arrays of one shape (1-D)."""
    D = np.asarray(D, float)
    a = np.maximum(np.asarray(a, float), D - R)
    b = np.minimum(np.asarray(b, float), D + R)
    out = np.zeros(D.shape)
    ok = b > a
    # a single GL-8 is exact to ~1e-13 when the pole of 1/r is far from the piece
    near = ok & (D <= 5 * R) & ((b - a) > 0.02 * a)
    far = ok & ~near
    if far.any():
        out[far] = _gl(D[far], a[far], b[far], R, m, mom, c)
    if near.any():
        Dn, an, bn = D[near], a[near], b[near]
        fr = np.linspace(0.0, 1.0, nsub + 1)
        A = an[:, None] + (bn - an)[:, None] * fr[None, :-1]
        B = an[:, None] + (bn - an)[:, None] * fr[None, 1:]
        Dr = np.repeat(Dn[:, None], nsub, 1)
        out[near] = _gl(Dr.ravel(), A.ravel(), B.ravel(), R, m, mom, c).reshape(-1, nsub).sum(1)
    return out


def expand(lo, hi):
    k = hi - lo + 1
    rep = np.repeat(np.arange(len(lo)), k)
    off = np.arange(int(k.sum())) - np.repeat(np.cumsum(k) - k, k)
    return rep, lo[rep] + off


# ------------------------------------------------------------------------------------------------
# Images
# ------------------------------------------------------------------------------------------------
def _lw(n, a):
    if a >= 1.0:
        return np.where(n > 0, -np.inf, 0.0)
    return n * math.log1p(-a)


def axis_images(Lx, s, r, a_lo, a_hi, reach):
    jmax = int(math.ceil(reach / Lx)) + 2
    j = np.arange(-jmax, jmax + 1)
    coord = np.where(j % 2 == 0, j * Lx + s, (j + 1) * Lx - s)
    aj = np.abs(j)
    n_hi = np.where(j > 0, (aj + 1) // 2, aj // 2)
    n_lo = np.where(j > 0, aj // 2, (aj + 1) // 2)
    lw = _lw(n_lo, a_lo) + _lw(n_hi, a_hi)
    off = coord - r
    keep = (np.abs(off) <= reach) & np.isfinite(lw)
    return off[keep], lw[keep], (j == 0)[keep]


def image_set(L, S, P, al6, R, T, c, m, floor_rel=1e-14, cap=9e6):
    """(D, log weight, is_direct) of every image with D - R < c T whose energy (after air) is at
    least floor_rel of the direct sound's."""
    reach = c * T + R
    (ox, wx, jx), (oy, wy, jy), (oz, wz, jz) = [
        axis_images(L[i], S[i], P[i], al6[2 * i], al6[2 * i + 1], reach) for i in range(3)]
    d = math.dist(S, P)
    lref = math.log(floor_rel) - 2 * math.log(d) - m * d
    YZ2 = oy[:, None] ** 2 + oz[None, :] ** 2
    WYZ = wy[:, None] + wz[None, :]
    JYZ = jy[:, None] & jz[None, :]
    wmax = float(WYZ.max())
    Ds, Ws, Js = [], [], []
    n = 0
    for i in range(len(ox)):
        amin = abs(ox[i])
        if amin > 1e-9 and wx[i] + wmax - 2 * math.log(amin) - m * amin < lref:
            continue
        D = np.sqrt(ox[i] ** 2 + YZ2)
        lw = wx[i] + WYZ
        ok = (D - R < c * T) & (lw - 2 * np.log(np.maximum(D, 1e-12)) - m * D >= lref)
        if not ok.any():
            continue
        Ds.append(D[ok])
        Ws.append(lw[ok])
        Js.append((JYZ & jx[i])[ok])
        n += int(ok.sum())
        if n > cap:
            raise MemoryError('image cap')
    D = np.concatenate(Ds)
    W = np.concatenate(Ws)
    J = np.concatenate(Js)
    if J.sum() != 1:
        raise RuntimeError('direct image count %d' % J.sum())
    if np.any(D <= R * 1.0001):
        raise ValueError('an image inside the receiver ball')
    return D, W, J


def t_max_for(L, al6, m, c, t, cap_s=15.0):
    """Path time to take images to: the slowest of Eyring and each axial pair (+ air) down 80 dB."""
    V = L[0] * L[1] * L[2]
    areas = [L[1] * L[2]] * 2 + [L[0] * L[2]] * 2 + [L[0] * L[1]] * 2
    St = sum(areas)
    ab = sum(a * s for a, s in zip(al6, areas)) / St
    k_ey = c * (-St * math.log(max(1 - ab, 1e-12))) / (4 * V)
    k_ax = min(c * (-math.log(max(1 - al6[2 * i], 1e-12)) - math.log(max(1 - al6[2 * i + 1], 1e-12))) / (2 * L[i])
               for i in range(3))
    k = max(min(k_ey, k_ax), 1e-3) + m * c
    return min(t + 18.42 / k + 0.02, cap_s)


class Echo:
    """One room, source, receiver, R, band (m), c."""

    def __init__(self, L, al6, S, P, R, c, m, T=None, floor_rel=1e-14, cap=9e6, tail=True):
        self.L, self.al6, self.S, self.P = list(L), list(al6), list(S), list(P)
        self.R, self.c, self.m = R, c, m
        self.d = math.dist(S, P)
        self.t = self.d / c
        self.h = R / c
        best = math.inf
        wall = math.inf
        for ax in range(3):
            for pl in (0.0, L[ax]):
                Sp = list(S)
                Sp[ax] = 2 * pl - S[ax]
                best = min(best, math.dist(Sp, P))
                wall = min(wall, abs(P[ax] - pl))
        self.d1 = max(best, self.d)
        self.wall = wall
        self.clear = wall > R
        self.T = T or t_max_for(L, al6, m, c, self.t)
        while True:
            try:
                self.D, self.lw, self.dir = image_set(L, S, P, al6, R, self.T, c, m, floor_rel, cap)
                break
            except MemoryError:
                self.T *= 0.8
        self.w = np.exp(self.lw)
        # every image's energy with continuous air
        E = np.zeros(len(self.D))
        for a in range(0, len(self.D), 400000):
            sl = slice(a, a + 400000)
            E[sl] = seg_int(self.D[sl], self.D[sl] - R, self.D[sl] + R, R, m)
        self.E = E * self.w
        self.E_dir = float(self.E[self.dir].sum())
        self.Tc = self.T + R / c
        self.tail_A = self.tail_k = None
        self.tail_E = self.tail_M = 0.0
        if tail:
            self._fit_tail()
        self.top = float(self.E.sum()) + self.tail_E

    def _fit_tail(self):
        """Exponential in centre time over the last 40 % of [t, T + R/c], 5 ms blocks."""
        c, t = self.c, self.t
        Tc = self.Tc
        tc = self.D[~self.dir] / c
        Er = self.E[~self.dir]
        a = t + 0.6 * (Tc - t)
        nbk = int((Tc - a) / 0.005)
        if nbk < 4:
            return
        edges = Tc - 0.005 * np.arange(nbk, -1, -1)
        hist, _ = np.histogram(tc, bins=edges, weights=Er)
        mid = 0.5 * (edges[1:] + edges[:-1])
        good = hist > 0
        if good.sum() < 4:
            return
        p = np.polyfit(mid[good], np.log(hist[good] / 0.005), 1)
        k = -p[0]
        if not k > 0:
            return
        A = math.exp(p[1])
        self.tail_A, self.tail_k = A, k
        self.tail_E = A / k * math.exp(-k * Tc)
        # int_Tc^inf (tau - t) A e^{-k tau} d tau
        self.tail_M = A * math.exp(-k * Tc) * ((Tc - t) / k + 1 / k ** 2)

    # --------------------------------------------------------------------------------------------
    def run(self, dt, air='step', delay_steps=0, cast=True, with_tail=True):
        """What an SPPS energetic run at step dt writes (expected value)."""
        c, R, m = self.c, self.R, self.m
        cdt = c * dt
        if air == 'step':
            p = f32(np.exp(np.float32(-np.float32(m) * np.float32(np.float32(c) * np.float32(dt)))))
            lp = math.log(p)
        lo = np.floor((self.D - R) / cdt).astype(np.int64)
        hi = np.floor((self.D + R) / cdt).astype(np.int64)
        nb = int(hi.max()) + 2
        H = np.zeros(nb)
        per = int(max(1, 3_000_000 // (int((hi - lo).max()) + 1)))
        for a in range(0, len(self.D), per):
            sl = slice(a, a + per)
            rep, n = expand(lo[sl], hi[sl])
            Dr = self.D[sl][rep]
            if air == 'step':
                val = seg_int(Dr, n * cdt, (n + 1) * cdt, R, 0.0) * np.exp(self.lw[sl][rep] + (n + 1) * lp)
            else:
                val = seg_int(Dr, n * cdt, (n + 1) * cdt, R, m) * self.w[sl][rep]
            H += np.bincount(n, weights=val, minlength=nb)[:nb]
        if with_tail and self.tail_k:
            A, k = self.tail_A, self.tail_k
            n0 = int(math.floor(self.Tc / dt))
            # until the tail's remainder is 1e-12 of the top
            n_end = int(math.ceil((self.Tc + math.log(max(self.tail_E / (1e-12 * self.top), 1.0)) / k) / dt)) + 1
            if n_end > n0:
                n = np.arange(n0, n_end)
                a0 = np.maximum(n * dt, self.Tc)
                b0 = (n + 1) * dt
                tv = A / k * (np.exp(-k * a0) - np.exp(-k * b0))
                if air == 'step':
                    tv = tv * np.exp((n + 1) * lp + m * c * (n + 0.5) * dt)
                if n_end > len(H):
                    H = np.concatenate([H, np.zeros(n_end - len(H))])
                H[n0:n_end] += tv
        if delay_steps:
            H = np.concatenate([np.zeros(delay_steps), H])
        if cast:
            H = H.astype(np.float32).astype(np.float64)
        nz = np.nonzero(H > 0)[0]
        return H[:int(nz[-1]) + 1]

    # --------------------------------------------------------------------------------------------
    def S_after(self, r_cut):
        """Reflected energy (continuous air) recorded beyond path length r_cut, with the tail."""
        R = self.R
        refl = ~self.dir
        full = refl & (self.D - R >= r_cut)
        strad = refl & (self.D - R < r_cut) & (self.D + R > r_cut)
        e = float(self.E[full].sum())
        if strad.any():
            Ds = self.D[strad]
            e += float((seg_int(Ds, np.full(len(Ds), r_cut), Ds + R, R, self.m) * self.w[strad]).sum())
        return e + self.tail_E

    def M_after(self, r_cut, t_ref):
        """sum of the reflected energy beyond r_cut times (tau - t_ref), with the tail."""
        R, c = self.R, self.c
        refl = ~self.dir & (self.D + R > r_cut)
        Ds = self.D[refl]
        mm = seg_int(Ds, np.full(len(Ds), r_cut), Ds + R, R, self.m, mom=t_ref, c=c) * self.w[refl]
        out = float(mm.sum())
        if self.tail_k:
            A, k, Tc = self.tail_A, self.tail_k, self.Tc
            out += A * math.exp(-k * Tc) * ((Tc - t_ref) / k + 1 / k ** 2)
        return out

    def ts_exact(self):
        return self.M_after(self.c * self.t, self.t) / self.top

    def u10(self):
        """Estimate of where the reflected energy after t + u drops below 0.1 of the top."""
        refl = ~self.dir
        o = np.argsort(self.D[refl])
        Dc = self.D[refl][o] / self.c
        Ec = self.E[refl][o]
        suf = np.cumsum(Ec[::-1])[::-1] + self.tail_E
        k = np.nonzero(suf < 0.1 * self.top)[0]
        if len(k) == 0:
            return None
        return max(0.0, Dc[k[0]] - self.t)

    def grid(self, dtf, u_end):
        """S at u_j = j dtf (j = 0..J) after t: exact, continuous air."""
        c, R, t = self.c, self.R, self.t
        r0 = c * t
        cd = c * dtf
        J = int(math.ceil(u_end / dtf))
        rJ = r0 + J * cd
        sel = (~self.dir) & (self.D + R > r0) & (self.D - R < rJ)
        D, w = self.D[sel], self.w[sel]
        j0 = np.clip(np.floor((D - R - r0) / cd), 0, J - 1).astype(np.int64)
        j1 = np.clip(np.floor((D + R - r0) / cd), 0, J - 1).astype(np.int64)
        B = np.zeros(J)
        per = int(max(1, 4_000_000 // (int((j1 - j0).max()) + 1))) if len(D) else 1
        for a in range(0, len(D), per):
            sl = slice(a, a + per)
            rep, j = expand(j0[sl], j1[sl])
            Dr = D[sl][rep]
            ra = r0 + j * cd
            val = seg_int(Dr, ra, ra + cd, R, self.m) * w[sl][rep]
            B += np.bincount(j, weights=val, minlength=J)[:J]
        after = self.S_after(rJ)
        S = np.concatenate([np.cumsum(B[::-1])[::-1] + after, [after]])
        return S, J

    def truth(self, dtfs=(2e-5, 1e-5, 5e-6), bottoms=(-10.0, -9.99, -10.01)):
        u = self.u10()
        u_end = (u if u is not None else 0.05) * 1.1 + 2 * self.R / self.c + 0.003
        out = dict(ts_exact=self.ts_exact(), top=self.top, E_dir=self.E_dir, tail_share=self.tail_E / self.top,
                   T_img=self.T, n_img=int(len(self.D)))
        for dtf in dtfs:
            while True:
                S, J = self.grid(dtf, u_end)
                if S[-1] < 10 ** (-1.0015) * self.top or u_end > 8.0:
                    break
                u_end *= 1.6
            uu = np.arange(J + 1) * dtf
            key = f'{dtf * 1e3:g}ms'
            out['edt_' + key] = {str(b): edt_fit(self.top, uu[:-1], uu[1:], S[:-1], S[1:], dtf, b) for b in bottoms}
            # Ts on the grid: pieces up to u_J, then exact beyond
            rest = self.M_after(self.c * (self.t + uu[-1]), self.t + uu[-1])
            out['ts_' + key] = (piece_integrals(uu[:-1], uu[1:], S[:-1], S[1:]).sum() + rest) / self.top
            out['u_end'] = u_end
        return out


# ------------------------------------------------------------------------------------------------
# Readings
# ------------------------------------------------------------------------------------------------
def edt_fit(top, u0, u1, s0, s1, dt, bottom=-10.0, top_db=0.0):
    """edt3.line (the shipped regression) on log-linear pieces, vectorised; returns EDT or nan."""
    u0, u1, s0, s1 = map(np.asarray, (u0, u1, s0, s1))
    ok = s1 > 0
    u0, u1, s0, s1 = u0[ok], u1[ok], s0[ok], s1[ok]
    l0 = 10 * np.log10(s0 / top)
    l1 = 10 * np.log10(s1 / top)
    use = ~((l1 > top_db) | (l0 < bottom))
    u0, u1, l0, l1 = u0[use], u1[use], l0[use], l1[use]
    q = (l1 - l0) / (u1 - u0)
    with np.errstate(divide='ignore', invalid='ignore'):
        ua = np.where(l0 > top_db, u0 + (top_db - l0) / q, u0)
        ub = np.where(l1 < bottom, u0 + (bottom - l0) / q, u1)
    w = ub - ua
    g = w > 0
    w, ua, ub, q, l0, u0 = w[g], ua[g], ub[g], q[g], l0[g], u0[g]
    mid = 0.5 * (ua + ub)
    lev = l0 + q * (mid - u0)
    span = w.sum()
    if span < 2 * dt * (1 - 1e-9):
        return float('nan')
    mean = (w * mid).sum() / span
    inner = w ** 3 / 12
    suu = (w * (mid - mean) ** 2 + inner).sum()
    sul = (w * (mid - mean) * lev + q * inner).sum()
    slope = sul / suu
    return float(-60.0 / slope) if slope < 0 else float('nan')


def piece_integrals(u0, u1, s0, s1):
    h = u1 - u0
    lg = (s1 > 0) & (s0 - s1 > 1e-12 * s0)
    with np.errstate(divide='ignore', invalid='ignore'):
        v = np.where(lg, h * (s0 - s1) / np.log(np.where(lg, s0 / np.where(s1 > 0, s1, 1.0), 2.0)), h * (s0 + s1) / 2)
    return v

