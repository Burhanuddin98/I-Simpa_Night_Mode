"""The composite on the two-step skeptic's hill-climbed false accepts (skeptic-twostep/
verify_fa*.txt), rebuilt with the GF3 skeptic's exact image-source echogram (per-step air, as a
run at that step writes it). Truth: the ideal reading at 0.02 ms (continuous air).
Usage: python eval_hunt.py -> hunt.txt"""
import math, os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(HERE, '..', 'skeptic-gf3'))
import numpy as np  # noqa: E402
import harness as H  # noqa: E402
import ism  # noqa: E402
import attack_ism as AI  # noqa: E402
import mirror as M  # noqa: E402

U = lambda a: [(a, a), (a, a), (a, a)]
T1 = [(0.2, 0.2), (0.2, 0.2), (0.1, 0.3)]
CASES = [
    # (label, L, alpha, S, R, Rb, band, step ms, quantity)
    ('hall8x12x4 a0.1', (8, 12, 4), U(0.1), (4.2157, 10.0722, 2.7692), (0.9465, 1.1573, 2.8999), 0.1, 16000, 2.0, 'edt'),
    ('hall8x12x4 a0.1', (8, 12, 4), U(0.1), (4.2313, 10.0828, 2.7858), (0.988, 1.1569, 3.1779), 0.1, 16000, 2.0, 'ts'),
    ('hall15x20x6 a0.15', (15, 20, 6), U(0.15), (13.9411, 12.4484, 3.2931), (0.7711, 0.9716, 2.6086), 0.1, 16000, 1.0, 'edt'),
    ('T1 room', (6, 10, 3), T1, (4.1073, 9.4699, 1.4845), (5.1708, 0.7104, 0.7623), 0.05, 16000, 1.0, 'edt'),
    ('hall15x20x6 a0.15', (15, 20, 6), U(0.15), (2.2877, 17.4766, 2.5764), (14.0719, 0.4528, 1.2405), 0.05, 16000, 1.0, 'edt'),
    ('hall15x20x6 a0.15', (15, 20, 6), U(0.15), (5.2347, 14.3206, 0.7607), (1.0842, 1.1696, 3.2151), 0.05, 16000, 1.0, 'edt'),
    ('hall15x20x6 a0.15', (15, 20, 6), U(0.15), (4.4362, 10.7988, 5.1262), (5.1384, 7.0274, 2.9452), 0.31, 4000, 5.0, 'ts'),
]

out = []
P = lambda s: (print(s, flush=True), out.append(s))
for label, L, alpha, S, R, Rb, F, ms, q in CASES:
    c = AI.C
    d = math.dist(S, R)
    d1, wall = AI.first_reflection(S, R, L)
    t, h = d / c, Rb / c
    dl = c * AI.DT_F
    lmax = c * 0.8
    direct, refl = ism.echogram(L, S, R, Rb, alpha, lmax, dl)
    m = ism.m_energy(F)
    a_c = ism.air_factor(len(direct), dl, m, c, None)
    te_i, ts_i = AI.truth_ideal(direct * a_c, refl * a_c, t, AI.DT_F)
    truth = te_i if q == 'edt' else ts_i
    k = int(round(ms * 1e-3 / AI.DT_F)); dtc = k * AI.DT_F
    vc = AI.rebin((direct + refl) * ism.air_factor(len(direct), dl, m, c, dtc), k)
    vc = vc[:int(np.nonzero(vc > 0)[0][-1]) + 1].tolist()
    res = H.read_all(vc, dtc, t, h, d1, c, wall > Rb, Rb, m)
    P(f'{label} {q.upper()} {F} Hz {ms} ms Rb {Rb}: ideal truth {truth:.6g}')
    for name in ('ship', 'w1d', 'W1G', 'W2G', 'W1G-G12'):
        ok, val, why = res[name][q]
        e = H.err(q, val, truth) if val is not None else None
        P(f'   {name:8s} {"ACCEPTS" if ok else "refuses"} {val if val is None else round(val, 7)}  '
          f'err {"-" if e is None else f"{e:.2f}x L"}  {why or ""}' + ('   <== FALSE ACCEPT' if ok and e and e > 1 else ''))
    P(f'   features {res["feat"]}')
open(os.path.join(HERE, 'hunt.txt'), 'w').write('\n'.join(out) + '\n')
