"""The composite on the 16 real fine-step datasets the w1d design used (bracket/common.datasets():
four 0.2 ms M8 cells, POS200 at 1 ms, eleven 1 ms calibration cells), rebinned to coarser steps.
Truth: the same series' shipped midpoint at its fine step, where settled (the design's truth).

Usage: python eval_real.py [workers] -> real_rows.pkl
"""
import os, pickle, sys, time
from concurrent.futures import ProcessPoolExecutor

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(HERE, '..', 'skeptic-gf3'))
import harness as H  # noqa: E402
import ism  # noqa: E402
C = H.C

STEPS_MS = (1, 2, 3, 4, 5, 10)


def work(item):
    name, fine, ri, run = item
    rows = []
    dtf = run['dt']
    R = run['h'] * run['c']
    for r in run['recs']:
        t, h = r['t'], run['h']
        for f, v in r['bands'].items():
            sv = C.shipped(v, dtf, t, h)
            if sv is None:
                continue
            e_f, s_f = sv
            truth = dict(edt=e_f['mid'] if e_f['ok'] else None, ts=s_f['mid'] if s_f['ok'] else None)
            m = ism.m_energy(float(f))
            for ms in STEPS_MS:
                k = int(round(ms / fine))
                if k < 2:
                    continue
                vv = C.rebin(v, k)
                dt = dtf * k
                res = H.read_all(vv, dt, t, h, r['d1'], run['c'], r['clear'], R, m)
                rows.append(dict(ds=name, run=ri, rec=r['label'], F=f, ms=ms, truth=truth, res=res))
    return rows


if __name__ == '__main__':
    workers = int(sys.argv[1]) if len(sys.argv) > 1 else 6
    t0 = time.time()
    items = []
    for name, desc, fine, runs in C.datasets():
        for ri, run in enumerate(runs):
            items.append((name, fine, ri, run))
    print(f'{len(items)} runs loaded in {time.time() - t0:.1f} s', flush=True)
    rows = []
    with ProcessPoolExecutor(workers) as ex:
        for rr in ex.map(work, items):
            rows += rr
    pickle.dump(rows, open(os.path.join(HERE, 'real_rows.pkl'), 'wb'))
    print(f'{len(rows)} series-steps in {time.time() - t0:.1f} s', flush=True)
