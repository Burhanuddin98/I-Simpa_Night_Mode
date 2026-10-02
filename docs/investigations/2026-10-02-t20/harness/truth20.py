"""The T20 truth (PREREG-S1S2.md, "Truths"): ISO 3382-1 cl. 6, least squares on the Schroeder curve from -5 to
-25 dB, 0 dB the total energy including the direct sound, T20 = -60 / slope.

Round 2's truth_ideal (harness2/m8b/_truth_ideal.py, attack_ism.py:55-77) with the range changed and nothing else:
the direct sound a step at t, every reflection where the ball records it, reflected energy before t lumped at t, and
the fit by the checked exact-integral line, harness2/m8b/_mirror.py `line`, executed only through truth.copies()
(its sha256 gate). Two mechanical differences, neither of which changes a returned value (test_truth20.py holds both
against a literal copy):
- the loop over bins is vectorised;
- pieces `line` would skip (shape 'lin', l1 above top_db, l0 below bottom_db) are left out beforehand, with a 1e-9 dB
  margin so a piece on a boundary is still passed and `line` itself decides (a boundary piece adds zero width).

`anchor='reflected'` (say-NO (c) only) leaves the direct sound out of 0 dB.
"""
import math

import numpy as np

_M = None
MARGIN_DB = 1e-9


def mirror():
    global _M
    if _M is None:
        from m8b import truth as T          # harness2 on sys.path: the caller's job
        _M = T.copies()[0]
    return _M


def pieces_of(direct, refl, t, dt, top_db, bottom_db, anchor='total'):
    nb = len(refl)
    S = np.concatenate([np.cumsum(refl[::-1])[::-1], [0.0]])     # S[k] = energy in bins >= k
    top = (float(direct.sum()) if anchor == 'total' else 0.0) + float(S[0])
    ia = int(math.floor(t / dt))
    frac = t / dt - ia
    s_t = S[ia + 1] + (1 - frac) * refl[ia]
    pieces = []
    u1 = (ia + 1) * dt - t
    shape = lambda s1: 'log' if s1 > 0 else 'lin'
    if u1 > 0:
        pieces.append((0.0, u1, float(s_t), float(S[ia + 1]), shape(S[ia + 1])))
    nz = np.nonzero(refl > 0)[0]
    last = int(nz[-1]) + 1 if len(nz) else 0
    k = np.arange(ia + 1, last)
    if len(k):
        s0, s1 = S[k], S[k + 1]
        with np.errstate(divide='ignore'):
            l0 = 10 * np.log10(s0 / top)
            l1 = 10 * np.log10(s1 / top)
        keep = (s1 > 0) & (l1 <= top_db + MARGIN_DB) & (l0 >= bottom_db - MARGIN_DB)
        for kk in k[keep].tolist():
            pieces.append((kk * dt - t, (kk + 1) * dt - t, float(S[kk]), float(S[kk + 1]), 'log'))
    return top, pieces


def t20(direct, refl, t, dt, top_db=-5.0, bottom_db=-25.0, anchor='total'):
    """(T s, None) or (nan, line's reason)."""
    top, pieces = pieces_of(np.asarray(direct, float), np.asarray(refl, float), t, dt, top_db, bottom_db, anchor)
    res, why = mirror().line(top, pieces, dt, top_db=top_db, bottom_db=bottom_db)
    return (-60.0 / res[0], None) if res else (float('nan'), why)
