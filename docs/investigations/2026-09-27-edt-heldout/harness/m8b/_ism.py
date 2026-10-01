"""Exact expected SPPS ball-receiver echogram of a specular box room (image sources), for the skeptic.

For an image source at distance D from the receiver centre, emitting energy W isotropically, the
expected energy x length SPPS records (reportmanager.cpp:196-224: energie * Lintersect per step) in
the path-length interval [l0, l1] is
    (W / 4) (1 / D) * integral_{l0}^{l1} (R^2 - (D - rho)^2) / rho  d rho,   rho in [D - R, D + R]
(the volume integral of rho^-2 over the ball, A_cap(rho)/rho^2 = pi (R^2-(D-rho)^2)/(rho D)).
Integrated per fine bin with 3-point Gauss-Legendre (the integrand is smooth, zero at the ends).

Image indexing (Allen & Berkley): coordinate 2 n L + (-1)^p s; reflections off the lower wall |n-p|,
off the upper wall |n|. Energy weight = product of (1 - alpha) per reflection.

Air absorption: SPPS multiplies the energy by p = exp(-m c dt) at the START of every step
(CalculationCore.cpp:50-58, base_core_configuration.cpp:114-115), so what step n records carries
p^(n+1). `air_factor` gives that for a run at step dt, or exp(-m rho) (continuous) when dt is None.
m (Np/m energy) = ISO 9613-1 alpha(dB/m) * ln(10)/10 as Coef_Att_Atmos.cpp computes it.
"""
import math
import numpy as np

GL_X = np.array([-math.sqrt(3 / 5), 0.0, math.sqrt(3 / 5)])
GL_W = np.array([5 / 9, 8 / 9, 5 / 9])


def iso9613_db_per_m(F, H=50.0, P=101325.0, T_c=20.0):
    K = T_c + 273.15
    K01, Pref, Kref = 273.16, 101325.0, 293.15
    C = -6.8346 * (K01 / K) ** 1.261 + 4.6151
    Ps = Pref * 10 ** C
    hmol = H * Ps / Pref
    cson = 343.2 * math.sqrt(K / Kref)
    Acr = (Pref / P) * 1.60e-10 * math.sqrt(K / Kref) * F ** 2
    FmolO, KvibO, FmolN, KvibN = 0.209, 2239.1, 0.781, 3352.0
    Fr = (P / Pref) * (24. + 4.04e4 * hmol * (0.02 + hmol) / (0.391 + hmol))
    Am = 1.559 * FmolO * math.exp(-KvibO / K) * (KvibO / K) ** 2
    AvibO = Am * (F / cson) * 2. * (F / Fr) / (1 + (F / Fr) ** 2)
    Fr = (P / Pref) * math.sqrt(Kref / K) * (9. + 280. * hmol * math.exp(-4.170 * ((K / Kref) ** (-1. / 3.) - 1)))
    Am = 1.559 * FmolN * math.exp(-KvibN / K) * (KvibN / K) ** 2
    AvibN = Am * (F / cson) * 2. * (F / Fr) / (1 + (F / Fr) ** 2)
    return Acr + AvibO + AvibN


def m_energy(F):
    return iso9613_db_per_m(F) * math.log(10) / 10


def images(L, s, alpha, lmax):
    """(positions (N,3), weights (N,), orders (N,)) of every image within lmax of the room.
    alpha: [(x_lo, x_hi), (y_lo, y_hi), (z_lo, z_hi)]."""
    per_axis = []
    for ax in range(3):
        nmax = int(math.ceil(lmax / (2 * L[ax]))) + 1
        cs, ws, os_ = [], [], []
        for n in range(-nmax, nmax + 1):
            for p in (0, 1):
                cs.append(2 * n * L[ax] + (-1) ** p * s[ax])
                lo, hi = abs(n - p), abs(n)
                ws.append((1 - alpha[ax][0]) ** lo * (1 - alpha[ax][1]) ** hi)
                os_.append(lo + hi)
        per_axis.append((np.array(cs), np.array(ws), np.array(os_)))
    X, Y, Z = np.meshgrid(per_axis[0][0], per_axis[1][0], per_axis[2][0], indexing='ij')
    W = (per_axis[0][1][:, None, None] * per_axis[1][1][None, :, None] * per_axis[2][1][None, None, :])
    O = (per_axis[0][2][:, None, None] + per_axis[1][2][None, :, None] + per_axis[2][2][None, None, :])
    P = np.stack([X.ravel(), Y.ravel(), Z.ravel()], axis=1)
    return P, W.ravel(), O.ravel()


def echogram(L, s, r, R, alpha, lmax, dl, chunk=3000):
    """Expected recorded energy per path-length bin of width dl, no air absorption, split into
    (direct, reflected). Bin i covers [i dl, (i+1) dl)."""
    P, W, O = images(L, s, alpha, lmax + R)
    D = np.linalg.norm(P - np.asarray(r, float)[None, :], axis=1)
    keep = (D - R < lmax) & (W > 0)
    P, W, O, D = P[keep], W[keep], O[keep], D[keep]
    nb = int(math.ceil(lmax / dl))
    direct = np.zeros(nb)
    refl = np.zeros(nb)
    nspan = int(math.ceil(2 * R / dl)) + 2
    for a in range(0, len(D), chunk):
        Dc, Wc, Oc = D[a:a + chunk], W[a:a + chunk], O[a:a + chunk]
        assert np.all(Dc > R), 'receiver ball contains an image'
        i0 = np.floor((Dc - R) / dl).astype(np.int64)
        idx = i0[:, None] + np.arange(nspan)[None, :]            # bins touched
        lo = np.maximum(idx * dl, (Dc - R)[:, None])
        hi = np.minimum((idx + 1) * dl, (Dc + R)[:, None])
        wid = np.clip(hi - lo, 0, None)
        mid = 0.5 * (lo + hi)
        acc = np.zeros_like(wid)
        for x, w in zip(GL_X, GL_W):
            rho = mid + 0.5 * wid * x
            u = rho - Dc[:, None]
            acc += w * (R * R - u * u) / rho
        val = (Wc / (4 * Dc))[:, None] * acc * 0.5 * wid
        val[wid <= 0] = 0
        ok = (idx < nb)
        isdir = (Oc == 0)
        for tgt, sel in ((direct, isdir), (refl, ~isdir)):
            if sel.any():
                ii = idx[sel][ok[sel]]
                vv = val[sel][ok[sel]]
                tgt += np.bincount(ii, weights=vv, minlength=nb)[:nb]
    return direct, refl


def air_factor(nb, dl, m, c, dt=None):
    """Per fine bin (width dl in path length): exp(-m rho_mid) if dt is None, else SPPS's step-wise
    p^(n+1) with n the step (of length c*dt) holding the fine bin."""
    rho_mid = (np.arange(nb) + 0.5) * dl
    if dt is None:
        return np.exp(-m * rho_mid)
    step = c * dt
    n = np.floor((np.arange(nb) * dl) / step + 1e-9)
    return np.exp(-m * step * (n + 1))
