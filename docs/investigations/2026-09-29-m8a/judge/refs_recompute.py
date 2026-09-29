#!/usr/bin/env python3
"""M8a judge, lens "references and transport cross-check": everything recomputed from scratch.

Nothing here imports, calls or copies simpa code. Inputs, read only:
  - beds/m8a.json                                   (rooms, source, receivers, atmosphere, cells)
  - C:/tmp/nm-m8a-bed/20260929T093134Z/report.json  (the claims checked: references, per-seed T30, gate C)
Outputs:
  - judge/refs_recompute.json                       (every number the report refs.md quotes)
  - C:/tmp/nm-judge-refs/transport_cache.json       (this script's own transport, so a re-run can skip it)

Run: python refs_recompute.py [--no-transport] [--rays-scale X]
Single process, numpy and scipy only (no multiprocessing).

Parts
  1. gamma^2 of each box, two ways:
     (a) exact, from integral geometry: for isotropic uniform random (IUR) chords of a convex body K,
         E[l^2] = (2/(pi*S)) * Int_KxK |x-y|^-2 dx dy     (Blaschke-Petkantschin, dx dy = |t1-t2|^2 dt1 dt2 dG,
         with the line measure normalised so that lines meeting K have measure pi*S/2; mean chord 4V/S).
         The pair integral is taken over the box's covariogram, 8*Int_[0,a]x[0,b]x[0,c] (a-x)(b-y)(c-z)/|z|^2,
         in spherical coordinates: the radial integral in closed form, the two angles by adaptive quadrature
         split at the kinks. Lambert reflection in a closed convex room keeps the field uniform and isotropic,
         so its free paths are IUR chords.
     (b) Monte Carlo: rays started in the stationary state (uniform on the surface, cosine direction), followed
         through successive Lambert reflections; standard error over 40 independent batches of rays.
  2. Kuttruff / plain Eyring / Sabine per cell and band, air term m from ISO 9613-1 (own transcription) and from
     upstream's Coef_Att_Atmos.cpp form (own transcription), the latter being what the solvers apply.
  3. Gate C recomputed from report.json's per-seed T30 and transport values, verdict rule re-implemented.
  4. The formula-vs-transport gap against gate A's band means, with the bed's transport and with this script's
     own Lambert transport (box, point source, 0.31 m receiver balls, 1 ms bins, own T30 estimator).
"""
import argparse
import json
import math
import os
import time

import numpy as np
from scipy import integrate, stats

WT = 'B:/repos/I-Simpa_Night_Mode/.claude/worktrees/m8a'
BED = WT + '/beds/m8a.json'
REPORT = 'C:/tmp/nm-m8a-bed/20260929T093134Z/report.json'
OUT = WT + '/docs/investigations/2026-09-29-m8a/judge/refs_recompute.json'
CACHE_DIR = 'C:/tmp/nm-judge-refs'
CACHE = CACHE_DIR + '/transport_cache.json'

C_SOUND = 343.2                      # SPPS's c at 20 C (upstream Celerite_du_son.cpp: 343.2*sqrt(T/293.15))
K_PHYS = 24.0 * math.log(10.0) / C_SOUND
T975_9 = float(stats.t.ppf(0.975, 9))
T975_19 = float(stats.t.ppf(0.975, 19))


# ----------------------------------------------------------------------------------------------
# 1a. gamma^2 exact, by integral geometry
# ----------------------------------------------------------------------------------------------
def box_pair_moment(a, b, c, k):
    """8 * Int_{octant} dOmega Int_0^R(u) r^k (a - r u1)(b - r u2)(c - r u3) dr.
    k = 0 gives Int_KxK |x-y|^-2 dx dy; k = 2 gives V^2 (a check of the angular quadrature)."""
    def radial(R, u1, u2, u3):
        c0 = a * b * c
        c1 = b * c * u1 + a * c * u2 + a * b * u3
        c2 = a * u2 * u3 + b * u1 * u3 + c * u1 * u2
        c3 = u1 * u2 * u3
        return (c0 * R ** (k + 1) / (k + 1) - c1 * R ** (k + 2) / (k + 2)
                + c2 * R ** (k + 3) / (k + 3) - c3 * R ** (k + 4) / (k + 4))

    def f(th, cp, sp):
        st, ct = math.sin(th), math.cos(th)
        u1, u2, u3 = st * cp, st * sp, ct
        R = math.inf
        if u1 > 0:
            R = min(R, a / u1)
        if u2 > 0:
            R = min(R, b / u2)
        if u3 > 0:
            R = min(R, c / u3)
        return radial(R, u1, u2, u3) * st

    def inner(phi):
        cp, sp = math.cos(phi), math.sin(phi)
        m = min(a / cp if cp > 0 else math.inf, b / sp if sp > 0 else math.inf)
        thk = math.atan2(m, c)          # where the ray's exit face changes from the z face to an x/y face
        o = dict(epsabs=0.0, epsrel=1e-12, limit=400)
        return (integrate.quad(f, 0.0, thk, args=(cp, sp), **o)[0]
                + integrate.quad(f, thk, math.pi / 2, args=(cp, sp), **o)[0])

    phk = math.atan2(b, a)              # where the exit face changes from x to y
    o = dict(epsabs=0.0, epsrel=1e-11, limit=400)
    return 8.0 * (integrate.quad(inner, 0.0, phk, **o)[0] + integrate.quad(inner, phk, math.pi / 2, **o)[0])


def gamma2_exact(a, b, c):
    V, S = a * b * c, 2 * (a * b + b * c + a * c)
    J = box_pair_moment(a, b, c, 0)
    v2 = box_pair_moment(a, b, c, 2)
    e2 = 2.0 * J / (math.pi * S)
    mfp = 4 * V / S
    return dict(gamma2=e2 / mfp ** 2 - 1.0, mean_free_path=mfp, mean_square=e2,
                pair_integral=J, v2_check_rel=v2 / V ** 2 - 1.0)


# ----------------------------------------------------------------------------------------------
# 1b. gamma^2 by Monte Carlo of successive Lambert reflections from the stationary state
# ----------------------------------------------------------------------------------------------
def lambert_dirs(rng, ax, inward_sign):
    """Cosine-weighted directions about the inward normal inward_sign * e_ax."""
    n = ax.size
    u = rng.random(n)
    ct, st = np.sqrt(u), np.sqrt(1.0 - u)
    ps = 2.0 * np.pi * rng.random(n)
    d = np.empty((n, 3))
    ar = np.arange(n)
    d[ar, ax] = inward_sign * ct
    d[ar, (ax + 1) % 3] = st * np.cos(ps)
    d[ar, (ax + 2) % 3] = st * np.sin(ps)
    return d


def next_wall(p, d, size):
    with np.errstate(divide='ignore', invalid='ignore'):
        inv = 1.0 / d
        tt = np.where(d > 0, (size - p) * inv, np.where(d < 0, -p * inv, np.inf))
    ax = tt.argmin(1)
    run = tt[np.arange(p.shape[0]), ax]
    return ax, run


def gamma2_mc(size, rays, paths, seed, batches=40, chunk=250_000):
    size = np.asarray(size, float)
    a, b, c = size
    areas = np.array([b * c, b * c, a * c, a * c, a * b, a * b])
    rng = np.random.default_rng(seed)
    per_ray_L, per_ray_Q, per_ray_P = [], [], []
    done = 0
    while done < rays:
        n = min(chunk, rays - done)
        done += n
        face = rng.choice(6, size=n, p=areas / areas.sum())
        ax = face // 2
        hi = face % 2 == 1
        p = rng.random((n, 3)) * size
        ar = np.arange(n)
        p[ar, ax] = np.where(hi, size[ax], 0.0)
        d = lambert_dirs(rng, ax, np.where(hi, -1.0, 1.0))
        L = np.zeros(n); Q = np.zeros(n); P = np.zeros(n); prev = None
        for _ in range(paths):
            ax, run = next_wall(p, d, size)
            L += run; Q += run * run
            if prev is not None:
                P += prev * run
            prev = run
            p = p + d * run[:, None]
            hi = d[ar, ax] > 0
            p[ar, ax] = np.where(hi, size[ax], 0.0)
            d = lambert_dirs(rng, ax, np.where(hi, -1.0, 1.0))
        per_ray_L.append(L); per_ray_Q.append(Q); per_ray_P.append(P)
    L = np.concatenate(per_ray_L); Q = np.concatenate(per_ray_Q); P = np.concatenate(per_ray_P)

    def stat(Ls, Qs, Ps):
        N = Ls.size * paths
        m = Ls.sum() / N
        m2 = Qs.sum() / N
        var = m2 - m * m
        lag1 = (Ps.sum() / (Ls.size * (paths - 1)) - m * m) / var
        eff = (Ls / paths).var() * paths / (m * m)      # Var(sum of K paths)/(K*mean^2)
        return m, m2 / (m * m) - 1.0, lag1, eff

    m, g2, lag1, eff = stat(L, Q, P)
    idx = np.array_split(np.arange(L.size), batches)
    bs = np.array([stat(L[i], Q[i], P[i]) for i in idx])
    se = bs.std(axis=0, ddof=1) / math.sqrt(batches)
    return dict(rays=int(rays), paths_per_ray=int(paths), paths=int(rays * paths),
                mean_free_path=m, mean_free_path_se=se[0], gamma2=g2, gamma2_se=se[1],
                lag1=lag1, lag1_se=se[2], effective_gamma2_K=eff, effective_gamma2_K_se=se[3],
                four_v_over_s=4 * a * b * c / (2 * (a * b + b * c + a * c)))


# ----------------------------------------------------------------------------------------------
# 2. Air term and the three formulas
# ----------------------------------------------------------------------------------------------
def iso9613_db_per_m(f, T_c, rh, p_pa):
    """ISO 9613-1:1993 eqs (3)-(5), humidity from Annex B (own transcription of the standard)."""
    pr, T0, T01 = 101325.0, 293.15, 273.16
    T = T_c + 273.15
    C = -6.8346 * (T01 / T) ** 1.261 + 4.6151
    h = rh * 10.0 ** C / (p_pa / pr)
    frO = (p_pa / pr) * (24.0 + 4.04e4 * h * (0.02 + h) / (0.391 + h))
    frN = (p_pa / pr) * (T / T0) ** -0.5 * (9.0 + 280.0 * h * math.exp(-4.170 * ((T / T0) ** (-1 / 3) - 1.0)))
    return 8.686 * f * f * (1.84e-11 * (pr / p_pa) * (T / T0) ** 0.5
                            + (T / T0) ** -2.5 * (0.01275 * math.exp(-2239.1 / T) / (frO + f * f / frO)
                                                   + 0.1068 * math.exp(-3352.0 / T) / (frN + f * f / frN)))


def upstream_db_per_m(F, H, P, T_c):
    """Upstream Coef_Att_Atmos.cpp (B:/repos/I-Simpa-upstream, read only), transcribed here."""
    Pref, Kref, K01 = 101325.0, 293.15, 273.16
    K = T_c + 273.15
    cson = 343.2 * math.sqrt(K / Kref)
    Cc = -6.8346 * (K01 / K) ** 1.261 + 4.6151
    hmol = H * Pref * 10.0 ** Cc / Pref
    Acr = (Pref / P) * 1.60e-10 * math.sqrt(K / Kref) * F * F
    Fr = (P / Pref) * (24.0 + 4.04e4 * hmol * (0.02 + hmol) / (0.391 + hmol))
    Am = 1.559 * 0.209 * math.exp(-2239.1 / K) * (2239.1 / K) ** 2
    AvO = Am * (F / cson) * 2.0 * (F / Fr) / (1.0 + (F / Fr) ** 2)
    Fr = (P / Pref) * math.sqrt(Kref / K) * (9.0 + 280.0 * hmol * math.exp(-4.170 * ((K / Kref) ** (-1 / 3) - 1.0)))
    Am = 1.559 * 0.781 * math.exp(-3352.0 / K) * (3352.0 / K) ** 2
    AvN = Am * (F / cson) * 2.0 * (F / Fr) / (1.0 + (F / Fr) ** 2)
    return Acr + AvO + AvN


def m_of(db_per_m):
    return db_per_m * math.log(10.0) / 10.0


def formulas(V, S, alpha, g2, m):
    ln = math.log1p(-alpha)
    AK = -S * ln * (1.0 + 0.5 * g2 * ln)
    AE = -S * ln
    AS = S * alpha
    air = 4.0 * m * V
    return dict(kuttruff=K_PHYS * V / (air + AK), eyring=K_PHYS * V / (air + AE), sabine=K_PHYS * V / (air + AS))


# ----------------------------------------------------------------------------------------------
# 4b. Renewal decay: free paths independent, every cumulant kept (what Kuttruff truncates at two)
# ----------------------------------------------------------------------------------------------
def iur_chords(size, n, seed, chunk=1_000_000):
    size = np.asarray(size, float)
    a, b, c = size
    areas = np.array([b * c, b * c, a * c, a * c, a * b, a * b])
    rng = np.random.default_rng(seed)
    out = []
    left = n
    while left > 0:
        k = min(chunk, left)
        left -= k
        face = rng.choice(6, size=k, p=areas / areas.sum())
        ax = face // 2
        hi = face % 2 == 1
        p = rng.random((k, 3)) * size
        p[np.arange(k), ax] = np.where(hi, size[ax], 0.0)
        d = lambert_dirs(rng, ax, np.where(hi, -1.0, 1.0))
        out.append(next_wall(p, d, size)[1])
    return np.concatenate(out)


def renewal_t60(chords, alpha, m):
    """With independent free paths l, E[(1-alpha)^N(t)] decays as exp(-s c t), s the root of
    (1-alpha) E[exp(s l)] = 1 (renewal equation, Laplace transform); air adds m c exactly. T = 6 ln10/lambda.
    Truncating ln E[exp(s l)] after its second cumulant and solving to first order in gamma^2 gives Kuttruff's
    A_K = -S ln(1-alpha) [1 + (gamma^2/2) ln(1-alpha)]."""
    from scipy.optimize import brentq
    lnq = math.log1p(-alpha)
    mean = chords.mean()
    f = lambda s: lnq + math.log(np.mean(np.exp(s * chords)))
    s = brentq(f, 1e-9, 2.0 * (-lnq) / mean)
    lam = s * C_SOUND + m * C_SOUND
    return 6.0 * math.log(10.0) / lam


# ----------------------------------------------------------------------------------------------
# 4. Own transport: point source, Lambert walls, receiver balls, 1 ms bins
# ----------------------------------------------------------------------------------------------
def deposit(acc, diff, dt, nb, s, e, rate):
    """Add energy at `rate` over [s, e) into bins of width dt: acc gets the partial first/last bins,
    diff a difference array for the full bins between (both of length nb + 2; the extra slots collect
    what falls beyond the last bin and are discarded)."""
    if s.size == 0:
        return
    js = np.floor(s / dt).astype(np.int64)
    je = np.floor(e / dt).astype(np.int64)
    same = js == je
    lim = nb + 1
    if same.any():
        acc += np.bincount(np.minimum(js[same], lim), weights=rate[same] * (e[same] - s[same]), minlength=nb + 2)
    o = ~same
    if o.any():
        js, je, s, e, r = js[o], je[o], s[o], e[o], rate[o]
        acc += np.bincount(np.minimum(js, lim), weights=r * ((js + 1) * dt - s), minlength=nb + 2)
        acc += np.bincount(np.minimum(je, lim), weights=r * (e - je * dt), minlength=nb + 2)
        full = je > js + 1
        if full.any():
            diff += np.bincount(np.minimum(js[full] + 1, lim), weights=r[full] * dt, minlength=nb + 2)
            diff -= np.bincount(np.minimum(je[full], lim), weights=r[full] * dt, minlength=nb + 2)


def own_transport(size, src, recs, radius, alpha, dur, dt, rays, replicas, seed, chunk=1_000_000):
    size = np.asarray(size, float)
    src = np.asarray(src, float)
    recs = [np.asarray(q, float) for q in recs]
    nb = int(math.ceil(dur / dt - 1e-9))
    r2 = radius * radius
    rng = np.random.default_rng(seed)
    room = np.zeros((replicas, nb))
    rec = np.zeros((replicas, len(recs), nb))
    per_rep = rays // replicas
    for k in range(replicas):
        acc_room = np.zeros(nb + 2); dif_room = np.zeros(nb + 2)
        acc_rec = [np.zeros(nb + 2) for _ in recs]; dif_rec = [np.zeros(nb + 2) for _ in recs]
        left = per_rep
        while left > 0:
            n = min(chunk, left)
            left -= n
            p = np.tile(src, (n, 1))
            z = 2.0 * rng.random(n) - 1.0
            ph = 2.0 * np.pi * rng.random(n)
            sz = np.sqrt(np.maximum(0.0, 1.0 - z * z))
            d = np.stack([sz * np.cos(ph), sz * np.sin(ph), z], 1)
            t = np.zeros(n)
            w = np.ones(n)
            while t.size:
                ax, run = next_wall(p, d, size)
                t1 = t + run / C_SOUND
                deposit(acc_room, dif_room, dt, nb, t, t1, w)
                for i, q in enumerate(recs):
                    mm = p - q
                    bb = np.einsum('ij,ij->i', mm, d)
                    disc = bb * bb - (np.einsum('ij,ij->i', mm, mm) - r2)
                    sel = np.nonzero(disc > 0)[0]
                    if sel.size:
                        root = np.sqrt(disc[sel])
                        x0 = np.maximum(-bb[sel] - root, 0.0)
                        x1 = np.minimum(-bb[sel] + root, run[sel])
                        ok = x1 > x0
                        if ok.any():
                            s_ = sel[ok]
                            deposit(acc_rec[i], dif_rec[i], dt, nb,
                                    t[s_] + x0[ok] / C_SOUND, t[s_] + x1[ok] / C_SOUND, w[s_] * C_SOUND)
                ar = np.arange(t.size)
                p = p + d * run[:, None]
                hi = d[ar, ax] > 0
                p[ar, ax] = np.where(hi, size[ax], 0.0)
                d = lambert_dirs(rng, ax, np.where(hi, -1.0, 1.0))
                w = w * (1.0 - alpha)
                t = t1
                keep = t < dur
                if not keep.all():
                    p, d, t, w = p[keep], d[keep], t[keep], w[keep]
        room[k] = (acc_room + np.cumsum(dif_room))[:nb]
        for i in range(len(recs)):
            rec[k, i] = (acc_rec[i] + np.cumsum(dif_rec[i]))[:nb]
    return room, rec


def t30_own(series, dt, t_start, t_after):
    """T30 from a 1 ms energy series: Schroeder backward sums at bin edges, 0 dB = everything from t_start
    on (the direct sound included), least-squares line through the edges at or after t_after whose level is
    within [-35, -5] dB; T30 = -60/slope."""
    S = np.concatenate([np.cumsum(series[::-1])[::-1], [0.0]])
    k0 = int(math.floor(t_start / dt + 1e-9))
    top = S[k0]
    edges = np.arange(S.size) * dt
    with np.errstate(divide='ignore'):
        L = 10.0 * np.log10(S / top)
    sel = (edges >= t_after - 1e-12) & (L <= -5.0) & (L >= -35.0)
    if sel.sum() < 3:
        return float('nan')
    slope = np.polyfit(edges[sel], L[sel], 1)[0]
    return -60.0 / slope


def air_factor(nb, dt, m):
    if not m:
        return np.ones(nb)
    x = m * C_SOUND * dt
    return np.exp(-x * np.arange(nb)) * (1.0 - math.exp(-x)) / x


def own_transport_cells(bed, room_name, alpha, rays, replicas, seed, bands_on):
    room = next(r for r in bed['rooms'] if r['name'] == room_name)
    a, b, c = room['size_m']
    V, S = a * b * c, 2 * (a * b + b * c + a * c)
    dt = 0.001
    te_off = formulas(V, S, alpha, 0.0, 0.0)['eyring']
    dur = 1.4 * te_off
    t0 = time.time()
    rm, rc = own_transport(room['size_m'], room['source_m'], room['receivers_m'], 0.31, alpha, dur, dt,
                           rays, replicas, seed)
    wall = time.time() - t0
    src = np.asarray(room['source_m'])
    arr = [float(np.linalg.norm(np.asarray(q) - src)) / C_SOUND for q in room['receivers_m']]
    hw = 0.31 / C_SOUND
    out = {}
    for label, m in [('off', 0.0)] + [(str(f), mm) for f, mm in bands_on]:
        dur_b = 1.4 * formulas(V, S, alpha, 0.0, m)['eyring']
        nb = int(math.ceil(dur_b / dt - 1e-9))
        fac = air_factor(nb, dt, m)
        # per replica: receivers' mean T30, and room energy T30
        rep_rec = []
        rep_room = []
        for k in range(replicas):
            vals = [t30_own(rc[k, i, :nb] * fac, dt, arr[i] - hw, arr[i] + hw) for i in range(len(arr))]
            rep_rec.append(float(np.mean(vals)))
            rep_room.append(t30_own(rm[k, :nb] * fac, dt, 0.0, 0.0))
        rep_rec = np.array(rep_rec); rep_room = np.array(rep_room)
        pooled = [t30_own(rc[:, i, :nb].sum(0) * fac, dt, arr[i] - hw, arr[i] + hw) for i in range(len(arr))]
        # jackknife over replicas of the pooled receivers' mean
        jk = []
        for k in range(replicas):
            keep = [j for j in range(replicas) if j != k]
            jk.append(np.mean([t30_own(rc[keep, i, :nb].sum(0) * fac, dt, arr[i] - hw, arr[i] + hw)
                               for i in range(len(arr))]))
        jk = np.array(jk)
        jk_se = math.sqrt((replicas - 1) / replicas * ((jk - jk.mean()) ** 2).sum())
        out[label] = dict(m=m, duration_s=dur_b,
                          t=float(rep_rec.mean()), se=float(rep_rec.std(ddof=1) / math.sqrt(replicas)),
                          pooled_t=float(np.mean(pooled)), pooled_se_jackknife=jk_se,
                          room_t=float(rep_room.mean()), room_se=float(rep_room.std(ddof=1) / math.sqrt(replicas)))
    return dict(room=room_name, alpha=alpha, rays=rays, replicas=replicas, seed=seed, wall_s=wall, bands=out)


# ----------------------------------------------------------------------------------------------
def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--no-transport', action='store_true')
    ap.add_argument('--rays-scale', type=float, default=1.0)
    args = ap.parse_args()
    bed = json.load(open(BED))
    rep = json.load(open(REPORT))
    res = dict(inputs=dict(bed=BED, report=REPORT, report_git_commit=rep['meta']['git_commit'],
                           report_bed_sha256=rep['meta']['bed_raw_sha256']),
               constants=dict(c=C_SOUND, K=K_PHYS, t975_9=T975_9, t975_19=T975_19))

    # ---- 1. gamma^2
    rooms = {r['name']: r for r in bed['rooms']}
    g2 = {}
    for name, dims, seed in [('cube', (1.0, 1.0, 1.0), 101)] + [(n, tuple(r['size_m']), 100 + i)
                                                               for i, (n, r) in enumerate(rooms.items())]:
        t0 = time.time()
        ex = gamma2_exact(*dims)
        mc = gamma2_mc(dims, rays=1_000_000, paths=64, seed=seed)
        g2[name] = dict(size=dims, exact=ex, mc=mc, seconds=time.time() - t0)
        print(f"gamma2 {name}: exact {ex['gamma2']:.6f} (V^2 check {ex['v2_check_rel']:+.1e}); "
              f"MC {mc['gamma2']:.6f} +- {mc['gamma2_se']:.6f} (mfp {mc['mean_free_path']:.5f} vs 4V/S "
              f"{mc['four_v_over_s']:.5f}); lag1 {mc['lag1']:.4f}; eff(K=64) {mc['effective_gamma2_K']:.4f}", flush=True)
    rg = {}
    for cell in rep['cells']:
        rr = cell['reference']
        rg.setdefault(cell['room'], set()).add((rr['gamma2'], rr['gamma2_se']))
    res['gamma2'] = g2
    res['gamma2_report'] = {k: sorted(v) for k, v in rg.items()}
    for name, vals in rg.items():
        for gv, gse in vals:
            ex = g2[name]['exact']['gamma2']
            print(f"  report {name}: {gv:.6f} +- {gse:.6f}; minus exact {gv - ex:+.6f} = {(gv - ex) / gse:+.2f} SE "
                  f"({100 * (gv / ex - 1):+.3f} % of gamma2)")

    # ---- 2. air and formulas
    atm = bed['atmosphere']
    air = {}
    for f in bed['bands_air_on_hz']:
        iso = iso9613_db_per_m(f, atm['temperature_c'], atm['relative_humidity_percent'], atm['pressure_pa'])
        up = upstream_db_per_m(f, atm['relative_humidity_percent'], atm['pressure_pa'], atm['temperature_c'])
        air[f] = dict(iso_db=iso, upstream_db=up, m_iso=m_of(iso), m_upstream=m_of(up))
    res['air'] = air
    rows = []
    worst = dict(kuttruff=0.0, eyring=0.0, sabine=0.0, a_reference=0.0, kuttruff_iso_air=0.0,
                 kuttruff_report_gamma2=0.0, m=0.0, mc_sd=0.0)
    for cell in rep['cells']:
        room = rooms[cell['room']]
        a, b, c = room['size_m']
        V, S = a * b * c, 2 * (a * b + b * c + a * c)
        gx = g2[cell['room']]['exact']['gamma2']
        rr = cell['reference']
        for bi, band in enumerate(rr['bands']):
            f = band['freq_hz']
            m = air[f]['m_upstream'] if cell['air'] else 0.0
            mi = air[f]['m_iso'] if cell['air'] else 0.0
            mine = formulas(V, S, cell['alpha'], gx, m)
            mine_iso = formulas(V, S, cell['alpha'], gx, mi)
            with_rep_g2 = formulas(rr['volume_m3'], rr['area_m2'], cell['alpha'], rr['gamma2'],
                                   band['air_m_per_metre'] or 0.0)
            aref = cell['a']['bands'][bi]['reference_s'] if cell.get('a') else band['kuttruff_s']
            # the sd Kuttruff inherits from gamma2's SE: |dT/dgamma2| * se
            ln = math.log1p(-cell['alpha'])
            AK = -S * ln * (1 + 0.5 * rr['gamma2'] * ln)
            sd = with_rep_g2['kuttruff'] * S * ln * ln / (2 * (4 * (band['air_m_per_metre'] or 0.0) * V + AK)) * rr['gamma2_se']
            row = dict(cell=cell['id'], gated=cell['gated'], freq_hz=f,
                       m_mine=m, m_report=band['air_m_per_metre'],
                       kuttruff_mine=mine['kuttruff'], kuttruff_report=band['kuttruff_s'],
                       eyring_mine=mine['eyring'], eyring_report=band['eyring_s'],
                       sabine_mine=mine['sabine'], sabine_report=band['sabine_s'],
                       a_reference_s=aref, kuttruff_iso_air=mine_iso['kuttruff'],
                       kuttruff_with_report_gamma2=with_rep_g2['kuttruff'],
                       kuttruff_mc_sd_mine=sd, kuttruff_mc_sd_report=band['kuttruff_mc_sd'])
            row['d_kuttruff'] = band['kuttruff_s'] / mine['kuttruff'] - 1
            row['d_eyring'] = band['eyring_s'] / mine['eyring'] - 1
            row['d_sabine'] = band['sabine_s'] / mine['sabine'] - 1
            row['d_a_reference'] = aref / mine['kuttruff'] - 1
            row['d_kuttruff_iso_air'] = band['kuttruff_s'] / mine_iso['kuttruff'] - 1
            row['d_kuttruff_report_gamma2'] = band['kuttruff_s'] / with_rep_g2['kuttruff'] - 1
            row['d_m'] = (band['air_m_per_metre'] / m - 1) if cell['air'] else 0.0
            row['d_mc_sd'] = band['kuttruff_mc_sd'] / sd - 1
            rows.append(row)
            if cell['gated']:
                for k, key in [('kuttruff', 'd_kuttruff'), ('eyring', 'd_eyring'), ('sabine', 'd_sabine'),
                               ('a_reference', 'd_a_reference'), ('kuttruff_iso_air', 'd_kuttruff_iso_air'),
                               ('kuttruff_report_gamma2', 'd_kuttruff_report_gamma2'), ('m', 'd_m'),
                               ('mc_sd', 'd_mc_sd')]:
                    worst[k] = max(worst[k], abs(row[key]))
    res['references'] = rows
    res['references_worst_gated'] = worst
    print('references, worst |relative difference| over gated cell-bands:', {k: f'{v:.2e}' for k, v in worst.items()})
    ro = [r for r in rows if not r['gated']]
    print('reported 20x8x4: worst', max(abs(r['d_kuttruff']) for r in ro), max(abs(r['d_eyring']) for r in ro))

    # ---- 3. gate C recomputed, and A's band means
    gc = []
    for cell in rep['cells']:
        vals = [[[x['t'] for x in rb] for rb in s['t30']] for s in cell['seeds']]
        if cell.get('c') is None or cell.get('a') is None or any(v is None for s in vals for r in s for v in r):
            gc.append(dict(cell=cell['id'], gated=cell['gated'],
                           skipped='not judged in the report (E6 refusal), no C to recompute'))
            continue
        vals = np.array(vals, float)                       # [seed][receiver][band]
        bands = cell['reference']['bands']
        ttr = np.array([bb['transport_t'] for bb in bands])
        str_ = np.array([bb['transport_se'] for bb in bands])
        tk = np.array([bb['kuttruff_s'] for bb in bands])
        y = (vals / ttr[None, None, :]).mean(axis=(1, 2)) - 1.0
        k = y.size
        d = y.mean(); sd = y.std(ddof=1)
        se_tr = float(np.mean(str_ / ttr))
        se = math.sqrt(sd * sd / k + se_tr * se_tr)
        tq = float(stats.t.ppf(0.975, k - 1))
        lo, hi = d - tq * se, d + tq * se
        reach = abs(d) + tq * se
        if reach <= 0.005:
            verdict = 'pass'
        elif (lo > 0 or hi < 0) and abs(d) > 0.005:
            verdict = 'fail'
        else:
            verdict = 'inconclusive'
        rc = cell['c']
        # A per band
        xa = (vals.mean(axis=1) / tk[None, :]) - 1.0     # [seed][band]
        a_means = xa.mean(0)
        a_half = tq * xa.std(0, ddof=1) / math.sqrt(k)
        a_rep = np.array([bb['interval']['mean'] for bb in cell['a']['bands']])
        # formula-vs-transport gap per band, and the identity (1+A_b) = (1+c_b)(1+g_b)
        g = tk / ttr - 1.0                                 # Kuttruff relative to the bed's transport
        cb = (vals.mean(axis=1) / ttr[None, :]).mean(0) - 1.0
        gc.append(dict(cell=cell['id'], gated=cell['gated'], method=cell['method'], air=cell['air'],
                       room=cell['room'], alpha=cell['alpha'],
                       d=d, sd=sd, se_tr=se_tr, se=se, lo=lo, hi=hi, reach=reach, verdict=verdict,
                       report_d=rc['interval']['mean'], report_lo=rc['interval']['lo'], report_hi=rc['interval']['hi'],
                       report_se=rc['interval']['se'], report_se_tr=rc['se_transport'], report_verdict=rc['verdict'],
                       per_seed_max_diff=float(np.max(np.abs(y - np.array(rc['per_seed'])))),
                       a_band_means=a_means.tolist(), a_band_half=a_half.tolist(),
                       a_band_means_report_max_diff=float(np.max(np.abs(a_means - a_rep))),
                       a_worst_edge=float(np.max(np.abs(a_means) + a_half)),
                       formula_minus_transport=g.tolist(), transport_over_formula=(ttr / tk - 1).tolist(),
                       c_per_band=cb.tolist(), freqs=[bb['freq_hz'] for bb in bands]))
    res['gate_c'] = gc
    # renewal (independent paths, all cumulants) against Kuttruff and the bed's transport, air off
    ren = []
    for i, (name, room) in enumerate(rooms.items()):
        a_, b_, c_ = room['size_m']
        V, S = a_ * b_ * c_, 2 * (a_ * b_ + b_ * c_ + a_ * c_)
        ch = iur_chords(room['size_m'], 10_000_000, 500 + i)
        half = ch.size // 2
        for alpha in (0.05, 0.1, 0.2, 0.4):
            tr_ = renewal_t60(ch, alpha, 0.0)
            spread_ = abs(renewal_t60(ch[:half], alpha, 0.0) - renewal_t60(ch[half:], alpha, 0.0)) / 2
            f_ = formulas(V, S, alpha, g2[name]['exact']['gamma2'], 0.0)
            cell = next((x for x in rep['cells'] if x['room'] == name and x['alpha'] == alpha
                         and not x['air'] and x['method'] == 'energetic'), None)
            bt = cell['reference']['bands'][0] if cell else None
            ren.append(dict(room=name, alpha=alpha, chords=int(ch.size), chord_mean=float(ch.mean()),
                            chord_gamma2=float(ch.var() / ch.mean() ** 2),
                            renewal_t=tr_, renewal_t_halfdiff=spread_, kuttruff=f_['kuttruff'], eyring=f_['eyring'],
                            renewal_over_kuttruff=tr_ / f_['kuttruff'] - 1,
                            bed_transport_t=bt['transport_t'] if bt else None,
                            bed_transport_room_t=bt['transport_room_t'] if bt else None,
                            bed_transport_over_renewal=(bt['transport_t'] / tr_ - 1) if bt else None,
                            bed_transport_over_kuttruff=(bt['transport_t'] / f_['kuttruff'] - 1) if bt else None))
            print(f"renewal {name} a {alpha}: T {tr_:.6f} (+-{spread_:.6f}); /Kuttruff-1 {100*(tr_/f_['kuttruff']-1):+.3f} %; "
                  + (f"bed transport/renewal-1 {100*(bt['transport_t']/tr_-1):+.3f} %, bed transport/Kuttruff-1 "
                     f"{100*(bt['transport_t']/f_['kuttruff']-1):+.3f} %" if bt else ''), flush=True)
    res['renewal'] = ren
    for r in gc:
        if 'skipped' in r:
            print(f"C {r['cell']}: skipped, {r['skipped']}")
            continue
        print(f"C {r['cell']:36s} d {100*r['d']:+.3f} % [{100*r['lo']:+.3f}, {100*r['hi']:+.3f}] reach {100*r['reach']:.3f} % "
              f"{r['verdict']:12s} report d {100*r['report_d']:+.3f} [{100*r['report_lo']:+.3f}, {100*r['report_hi']:+.3f}] "
              f"{r['report_verdict']}; |dy| {r['per_seed_max_diff']:.1e}")

    # ---- 4. own transport
    trans = {}
    if os.path.exists(CACHE):
        trans = json.load(open(CACHE))
    if not args.no_transport:
        os.makedirs(CACHE_DIR, exist_ok=True)
        bands_on = [(f, air[f]['m_upstream']) for f in bed['bands_air_on_hz']]
        for i, (room_name, alpha) in enumerate([(r, a) for r in ('6x10x3', '5x4x3') for a in (0.05, 0.1, 0.2, 0.4)]):
            key = f'{room_name}|{alpha}'
            rays = int(round(args.rays_scale * 1_000_000 * alpha / 0.05 / 16)) * 16
            if key in trans and trans[key]['rays'] == rays:
                continue
            r = own_transport_cells(bed, room_name, alpha, rays, 16, 20260929 + i, bands_on)
            trans[key] = r
            json.dump(trans, open(CACHE, 'w'), indent=1)
            print(f"own transport {key}: {rays} rays, {r['wall_s']:.0f} s; air off: receivers {r['bands']['off']['t']:.6f} "
                  f"+- {r['bands']['off']['se']:.6f}, room {r['bands']['off']['room_t']:.6f} +- {r['bands']['off']['room_se']:.6f}",
                  flush=True)
    res['own_transport'] = trans
    # compare with the bed's transport and with Kuttruff
    cmp_rows = []
    for cell in rep['cells']:
        if not cell['gated']:
            continue
        key = f"{cell['room']}|{cell['alpha']}"
        if key not in trans:
            continue
        for band in cell['reference']['bands']:
            lab = str(band['freq_hz']) if cell['air'] else 'off'
            mine = trans[key]['bands'][lab]
            cmp_rows.append(dict(cell=cell['id'], freq_hz=band['freq_hz'],
                                 own_t=mine['t'], own_se=mine['se'], bed_t=band['transport_t'], bed_se=band['transport_se'],
                                 own_over_bed=mine['t'] / band['transport_t'] - 1,
                                 z=(mine['t'] - band['transport_t']) / math.hypot(mine['se'], band['transport_se']),
                                 own_room_t=mine['room_t'], own_room_se=mine['room_se'],
                                 bed_room_t=band['transport_room_t'], bed_room_se=band['transport_room_se'],
                                 room_z=(mine['room_t'] - band['transport_room_t']) / math.hypot(mine['room_se'], band['transport_room_se']),
                                 own_pooled_t=mine['pooled_t'],
                                 own_over_kuttruff=mine['t'] / band['kuttruff_s'] - 1,
                                 bed_over_kuttruff=band['transport_t'] / band['kuttruff_s'] - 1))
    res['own_vs_bed_transport'] = cmp_rows
    # gate C's statistic again, with this script's transport in place of the bed's; and A against the gap
    c_own = []
    for cell in rep['cells']:
        key = f"{cell['room']}|{cell['alpha']}"
        if not cell['gated'] or key not in trans or cell.get('c') is None:
            continue
        vals = np.array([[[x['t'] for x in rb] for rb in s['t30']] for s in cell['seeds']], float)
        bands = cell['reference']['bands']
        labs = [str(bb['freq_hz']) if cell['air'] else 'off' for bb in bands]
        tt = np.array([trans[key]['bands'][l]['t'] for l in labs])
        ts = np.array([trans[key]['bands'][l]['se'] for l in labs])
        tk = np.array([bb['kuttruff_s'] for bb in bands])
        y = (vals / tt[None, None, :]).mean(axis=(1, 2)) - 1.0
        k = y.size
        d = y.mean(); sd = y.std(ddof=1); se_tr = float(np.mean(ts / tt))
        se = math.sqrt(sd * sd / k + se_tr * se_tr)
        tq = float(stats.t.ppf(0.975, k - 1))
        reach = abs(d) + tq * se
        a_mean = float(((vals.mean(axis=1) / tk[None, :]) - 1.0).mean())
        g_own = float(np.mean(tt / tk - 1.0))
        g_bed = float(np.mean([bb['transport_t'] / bb['kuttruff_s'] - 1.0 for bb in bands]))
        c_own.append(dict(cell=cell['id'], method=cell['method'], alpha=cell['alpha'], air=cell['air'],
                          d=d, se=se, lo=d - tq * se, hi=d + tq * se, reach=reach,
                          verdict='pass' if reach <= 0.005 else ('fail' if (abs(d) > 0.005 and (d - tq * se > 0 or d + tq * se < 0)) else 'inconclusive'),
                          a_cell_mean=a_mean, gap_own=g_own, gap_bed=g_bed,
                          a_minus_gap_own=a_mean - g_own, a_minus_gap_bed=a_mean - g_bed))
        print(f"C vs own transport {cell['id']:34s} d {100*d:+.3f} % [{100*(d-tq*se):+.3f}, {100*(d+tq*se):+.3f}] "
              f"reach {100*reach:.3f} %; A mean {100*a_mean:+.3f} %, own gap {100*g_own:+.3f} %, bed gap {100*g_bed:+.3f} %")
    res['gate_c_own_transport'] = c_own
    if cmp_rows:
        z = np.array([r['z'] for r in cmp_rows]); rz = np.array([r['room_z'] for r in cmp_rows])
        print(f"own vs bed transport, receivers: z mean {z.mean():+.2f}, rms {math.sqrt((z*z).mean()):.2f}, max |z| {np.abs(z).max():.2f}; "
              f"room: z mean {rz.mean():+.2f}, rms {math.sqrt((rz*rz).mean()):.2f}, max |z| {np.abs(rz).max():.2f} over {z.size} cell-bands")
    json.dump(res, open(OUT, 'w'), indent=1, default=float)
    print('wrote', OUT)


if __name__ == '__main__':
    main()
