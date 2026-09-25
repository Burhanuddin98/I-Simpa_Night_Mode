"""The composite on the w1d skeptic's noise-free synthetic specular shoeboxes (bracket-skeptic/
synth.py, checked against V-E5's 10-seed mean: EDT within 0.2 %): its main receiver set
(synth_eval.receivers: random, near the source, near walls, corners, heights; R 0.31, 0.5, 0.9 at
1 kHz), the corridor W, the dense near-source scan (d 0.4-2 m, R 0.31) and the 1-3 m scan (R 0.31
and 0.5). Continuous air absorption, exact rebins (as the skeptic tested). Truth: the shipped
midpoint at 0.02 ms, where settled (the skeptic's truth).

Usage: python eval_synth.py [workers] -> synth_rows.pkl
"""
import math, os, pickle, sys, time
from concurrent.futures import ProcessPoolExecutor

HERE = os.path.dirname(os.path.abspath(__file__))
BS = os.path.join(HERE, '..', 'bracket-skeptic')
sys.path.insert(0, HERE)
import harness as H  # noqa: E402  (imports bracket/common as 'common' first)
sys.path.insert(0, BS)
import numpy as np  # noqa: E402
import synth  # noqa: E402
import synth_eval as S  # noqa: E402
import synth_near  # noqa: E402
import synth_dist  # noqa: E402
C = H.C
STEPS = (1, 2, 3, 4, 5)
# synth's bands carry m (Np/m) directly; the composite's air factor needs it.


def items():
    out = []
    for room in S.ROOMS:
        L, src, alpha, tmax, bands = S.ROOMS[room]
        recs = S.receivers(room, L, src)
        for band in bands:
            for R in ((0.31, 0.5, 0.9) if band == '1k' else (0.31,)):
                for kind, p in recs:
                    out.append((room, band, R, kind, p))
    out += synth_near.items_for(0.31, 80)
    out += synth_dist.items_for(80)
    return out


def work(item):
    room, band, R, kind, rec = item
    L, src, alpha, tmax, bands = S.ROOMS[room]
    m = bands[band]
    Hh, d = synth.histogram(L, src, rec, alpha, m, R, S.C_SOUND, S.DTF, tmax)
    if d <= R * 1.05:
        return []
    c = S.C_SOUND
    t = d / c
    h = R / c
    d1, sep, clear = synth.geometry(L, src, rec, R)
    sv = C.shipped(Hh, S.DTF, t, h)
    if sv is None:
        return []
    e_f, s_f = sv
    truth = dict(edt=e_f['mid'] if e_f['ok'] else None, ts=s_f['mid'] if s_f['ok'] else None)
    rows = []
    for ms in STEPS:
        k = int(round(ms * 1e-3 / S.DTF))
        vv = C.rebin(Hh, k)
        dt = S.DTF * k
        try:
            res = H.read_all(vv, dt, t, h, d1, c, clear, R, m)
        except Exception as ex:
            rows.append(dict(room=room, band=band, R=R, kind=kind, rec=[round(x, 3) for x in rec], ms=ms, error=repr(ex)))
            continue
        rows.append(dict(room=room, band=band, R=R, kind=kind, rec=[round(x, 3) for x in rec], d=d, d1=d1,
                         clear=clear, ms=ms, truth=truth, res=res))
    return rows


if __name__ == '__main__':
    workers = int(sys.argv[1]) if len(sys.argv) > 1 else 4
    its = items()
    t0 = time.time()
    print(len(its), 'series', flush=True)
    rows = []
    with ProcessPoolExecutor(workers) as ex:
        for i, rr in enumerate(ex.map(work, its, chunksize=4)):
            rows += rr
            if i % 200 == 0:
                print(f'{i + 1}/{len(its)} {time.time() - t0:.0f} s', flush=True)
    pickle.dump(rows, open(os.path.join(HERE, 'synth_rows.pkl'), 'wb'))
    print(f'{len(rows)} rows, {sum(1 for r in rows if "error" in r)} errors, {time.time() - t0:.0f} s', flush=True)
