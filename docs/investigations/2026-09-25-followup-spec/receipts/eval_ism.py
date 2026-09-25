"""The composite on the GF3 skeptic's exact specular-box echograms (skeptic-gf3/ism.py, validated
against C-E6's 10-seed mean): tutorial 1's room, the dead-floor corridor, 5x4x3 m at alpha 0.4 and
0.6, and the radius set (R 0.1, 0.2, 0.5), bands 125 Hz to 20 kHz, near walls and near the source.

Coarse series are what an SPPS run at that step writes (air absorption applied per step, 'run'
kind). Truths are the skeptic's, reused from its rows: 'ideal' (direct sound a step at r/c, every
reflection where the ball records it, 0.02 ms, continuous air) and 'fine' (the shipped reading at
0.02 ms). A false accept is an accepted value beyond its limit of the ideal truth.

Usage: python eval_ism.py [workers] -> ism_rows.pkl
"""
import json, math, os, pickle, sys, time
from concurrent.futures import ProcessPoolExecutor

HERE = os.path.dirname(os.path.abspath(__file__))
GF3 = os.path.join(HERE, '..', 'skeptic-gf3')
sys.path.insert(0, HERE)
sys.path.insert(0, GF3)
import numpy as np  # noqa: E402
import harness as H  # noqa: E402
import ism  # noqa: E402
import attack_ism as AI  # noqa: E402

STEPS = (1.0, 1.5, 2.0, 3.0, 4.0, 5.0)


def truths():
    out = {}
    for name in ('t1', 'corridor', 'dead', 'deader', 'radius'):
        for line in open(os.path.join(GF3, f'attack_ism_{name}.jsonl')):
            for r in json.loads(line):
                if 'error' in r or r.get('kind') != 'run':
                    continue
                key = (r['name'], tuple(round(x, 6) for x in r['rec']), r['F'], round(r['R'], 6))
                out[key] = dict(te_i=r['te_i'], ts_i=r['ts_i'], te_f=r['te_f'], ts_f=r['ts_f'])
    return out


def jobs():
    out = []
    for name in ('t1', 'corridor', 'dead', 'deader'):
        L, alpha, src, T = AI.ROOMS[name]
        for p in AI.grid(name):
            out.append((name, name, L, alpha, src, p, AI.R_DEFAULT, AI.C * T))
    for name, pts in (('t1', [(1.0, 1.0, 1.8), (3.0, 7.0, 1.8), (2.0, 3.0, 1.0), (4.8, 2.0, 2.0), (1.5, 8.5, 1.2), (5.0, 6.0, 0.6),
                              (2.2, 5.3, 1.8), (3.6, 5.7, 2.2), (0.8, 4.0, 1.5), (5.4, 9.2, 2.5)]),
                      ('dead', [(1.0, 1.0, 1.0), (4.0, 3.0, 2.0), (1.2, 2.0, 1.0), (3.9, 2.9, 1.1), (2.0, 2.6, 1.9), (3.3, 1.2, 1.2)])):
        L, alpha, src, T = AI.ROOMS[name]
        for R in (0.1, 0.2, 0.5):
            for p in pts:
                if min(min(p[i], L[i] - p[i]) for i in range(3)) > R + 0.02 and math.dist(p, src) > R + 0.3:
                    out.append((f'{name}-R{R}', name, L, alpha, src, p, R, AI.C * T))
    return out


TRUTH = None


def work(job):
    global TRUTH
    if TRUTH is None:
        TRUTH = truths()
    label, room, L, alpha, src, rec, R, lmax = job
    d = math.dist(src, rec)
    d1, wall = AI.first_reflection(src, rec, L)
    c = AI.C
    t = d / c
    h = R / c
    clear = wall > R
    dl = c * AI.DT_F
    direct, refl = ism.echogram(L, src, rec, R, alpha, lmax, dl)
    rows = []
    for F in AI.BANDS:
        key = (label, tuple(round(x, 6) for x in rec), F, round(R, 6))
        tr = TRUTH.get(key)
        if tr is None:
            continue
        m = ism.m_energy(F)
        for K in STEPS:
            k = int(round(K * 1e-3 / AI.DT_F))
            dtc = k * AI.DT_F
            vc = AI.rebin((direct + refl) * ism.air_factor(len(direct), dl, m, c, dtc), k)
            nz = np.nonzero(vc > 0)[0]
            if len(nz) == 0:
                continue
            vc = vc[:int(nz[-1]) + 1].tolist()
            try:
                res = H.read_all(vc, dtc, t, h, d1, c, clear, R, m)
            except Exception as ex:  # a failure is a row, never a silent pass
                rows.append(dict(set=label, room=room, rec=list(rec), F=F, step=K, R=R, error=repr(ex)))
                continue
            rows.append(dict(set=label, room=room, rec=list(rec), F=F, step=K, R=R, d=d, d1=d1, wall=wall,
                             truth=tr, res=res))
    return rows


if __name__ == '__main__':
    workers = int(sys.argv[1]) if len(sys.argv) > 1 else 6
    js = jobs()
    t0 = time.time()
    print(len(js), 'receivers', flush=True)
    rows = []
    with ProcessPoolExecutor(workers) as ex:
        for i, rr in enumerate(ex.map(work, js, chunksize=1)):
            rows += rr
            if i % 50 == 0:
                print(f'{i + 1}/{len(js)} {time.time() - t0:.0f} s', flush=True)
    pickle.dump(rows, open(os.path.join(HERE, 'ism_rows.pkl'), 'wb'))
    print(f'{len(rows)} rows, {sum(1 for r in rows if "error" in r)} errors, {time.time() - t0:.0f} s', flush=True)
