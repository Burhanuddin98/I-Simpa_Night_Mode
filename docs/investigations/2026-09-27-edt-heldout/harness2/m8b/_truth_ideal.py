"""Text copy of the ISM skeptic's truth, Definition A (P15): the direct sound a step at t, every
reflection where the ball records it.

Source: target/agents/followup-design/skeptic-gf3/attack_ism.py (gitignored), sha256 3fb8a84b9eb42569,
lines 55-77 (truth_ideal), copied unchanged below the marker. The source is CRLF; this file is LF
(../.gitattributes). In the source, M is mirror.py (attack_ism.py:25); here it is the checked copy
m8b/_mirror.py, which truth.py puts in this module's namespace before it executes this file, so the
file is never imported on its own. m8b/provenance.json records the source hash, the range and this
file's hashes. truth.py executes this file only after its bytes hash to the value truth.py pins, and
truth.check_copies() compares the body with its source lines. Do not edit below the marker.
"""
import math

import numpy as np

# ---- copied from attack_ism.py (do not edit below this line) ----
def truth_ideal(direct, refl, t, dt):
    """Direct sound a step at t; reflections at their recorded times; reflected energy before t
    lumped at t. (EDT, Ts)."""
    nb = len(refl)
    S = np.concatenate([np.cumsum(refl[::-1])[::-1], [0.0]])     # S[k] = energy in bins >= k
    top = direct.sum() + S[0]
    ia = int(math.floor(t / dt))
    frac = t / dt - ia
    s_t = S[ia + 1] + (1 - frac) * refl[ia]
    pieces = []
    u1 = (ia + 1) * dt - t
    shape = lambda s1: 'log' if s1 > 0 else 'lin'
    if u1 > 0:
        pieces.append((0.0, u1, s_t, S[ia + 1], shape(S[ia + 1])))
    last = nb
    while last > 0 and refl[last - 1] <= 0:
        last -= 1
    for k in range(ia + 1, last):
        pieces.append((k * dt - t, (k + 1) * dt - t, S[k], S[k + 1], shape(S[k + 1])))
    res, why = M.line(top, pieces, dt)
    edt = -60.0 / res[0] if res else float('nan')
    ts = sum(M.integral(p) for p in pieces) / top
    return edt, ts
