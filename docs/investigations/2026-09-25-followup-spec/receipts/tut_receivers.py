"""Tutorial 1's two receivers under W1G, noise-free (GF3 skeptic's image-source echogram, runs with
per-step air), octave bands 125 Hz to 16 kHz and 20 kHz, steps 1 and 2 ms. Truth: ideal at 0.02 ms."""
import math, os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE); sys.path.insert(0, os.path.join(HERE, '..', 'skeptic-gf3'))
import numpy as np
import harness as H, ism, attack_ism as AI
L, alpha, src, T = AI.ROOMS['t1']
out = []
P = lambda s: (print(s, flush=True), out.append(s))
for lab, rec in (('Receiver 1', (1.0, 1.0, 1.8)), ('Receiver 2', (3.0, 7.0, 1.8))):
    c = AI.C; R = AI.R_DEFAULT
    d = math.dist(src, rec); d1, wall = AI.first_reflection(src, rec, L)
    t, h = d / c, R / c; dl = c * AI.DT_F
    direct, refl = ism.echogram(L, src, rec, R, alpha, c * T, dl)
    for F in (125, 250, 500, 1000, 2000, 4000, 8000, 16000, 20000):
        m = ism.m_energy(F)
        a_c = ism.air_factor(len(direct), dl, m, c, None)
        te_i, ts_i = AI.truth_ideal(direct * a_c, refl * a_c, t, AI.DT_F)
        cells = []
        for ms in (1.0, 2.0):
            k = int(round(ms * 1e-3 / AI.DT_F)); dtc = k * AI.DT_F
            vc = AI.rebin((direct + refl) * ism.air_factor(len(direct), dl, m, c, dtc), k)
            vc = vc[:int(np.nonzero(vc > 0)[0][-1]) + 1].tolist()
            res = H.read_all(vc, dtc, t, h, d1, c, wall > R, R, m)
            s = []
            for q, tr in (('edt', te_i), ('ts', ts_i)):
                ok, val, why = res['W1G'][q]
                s.append(f"{q.upper()} " + (f"acc {H.err(q, val, tr):.2f}xL" if ok else f"ref ({why})"))
            cells.append(f"{ms:g} ms: " + ', '.join(s))
        P(f"{lab} {F:>5} Hz: EDT {te_i:.3f} s, level after direct {res['feat']['lvl_direct']:.2f} dB | " + ' | '.join(cells))
open(os.path.join(HERE, 'tut_receivers.txt'), 'w').write('\n'.join(out) + '\n')
