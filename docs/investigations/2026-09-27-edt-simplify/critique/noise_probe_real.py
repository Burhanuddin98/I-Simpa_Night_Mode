"""Same Poisson-count noise estimate as noise_probe.py, on the repo's real SPPS seeds (10 seeds per
cell, the 10 cells of the evaluator's real set, 1 ms -> 2 ms): per (cell, receiver, band) compare the
seed-to-seed sd of LeanBand's EDT with (a) the sd LeanBand's own range implies and (b) the Poisson
prediction from a single seed's bins. Read-only, single process."""
import sys, json, glob, math, warnings
sys.dont_write_bytecode = True
warnings.filterwarnings('ignore')
import numpy as np
from common import CANDS
AG = 'B:/repos/I-Simpa_Night_Mode/target/agents'
ALLOWED = ['C-E3', 'C-E4', 'C-E6', 'V-E2', 'V-E5', 'C-R3', 'C-R4', 'C-R6', 'V-R2', 'V-R6']
ROOTS = (AG + '/pm8-noise-scratch/runs/noise-cal-1790307822', AG + '/pm8-noise-scratch/runs/noise-cal-1790310131')
K = 2
H = 0.31 / 343.2
lb = CANDS['leanband']


def poisson_pred(b, dt, t_arr):
    k0 = int(math.floor((t_arr + H) / dt)) + 1
    seg = b[k0:]
    S_ = np.cumsum(seg[::-1])[::-1]
    i10 = int(np.nonzero(S_ <= 0.1 * S_[0])[0][0])
    dec = seg[i10: max(i10 * 3, i10 + 10)]   # diffuse part only: after the -10 dB point (early reflections are structure, not noise)
    w = float(((dec[:-1] - dec[1:]) ** 2).sum() / (2 * dec.sum()))
    return 4.343 / math.sqrt(S_[i10] / w) / 10.0


ratios_impl, ratios_pred, rows = [], [], []
for root in ROOTS:
    for c in json.load(open(root + '/cells.json')):
        cc = c if 'id' in c else c['cell']
        if cc['id'] not in ALLOWED or abs(cc['time_step_s'] - 0.001) > 1e-9:
            continue
        runs = [json.load(open(p))['spps'] for p in sorted(glob.glob(f"{root}/{cc['id']}/seed*/report.json"))]
        for ri, pr0 in enumerate(runs[0]['point_receivers']):
            for bi, b0 in enumerate(pr0['bands']):
                vals, impl, pred = [], [], []
                for d in runs:
                    v = np.asarray(d['point_receivers'][ri]['bands'][bi]['energy_pa2'], float)
                    n = len(v) // K
                    vb = v[:n * K].reshape(n, K).sum(1)
                    dt = d['time_step_s'] * K
                    r = lb.analyse(vb, dt, pr0['arrival_s'], {})
                    if r['edt'] is None:
                        continue
                    vals.append(r['edt'])
                    impl.append((r['edt_hi'] - r['edt_lo']) / 4.0 / r['edt'])
                    try:
                        pred.append(poisson_pred(vb, dt, pr0['arrival_s']))
                    except Exception:
                        pass
                if len(vals) >= 5 and pred:
                    sd = float(np.std(vals, ddof=1) / np.mean(vals))
                    rows.append((cc['id'], b0['freq_hz'], sd, float(np.median(impl)), float(np.median(pred))))
sd = np.array([r[2] for r in rows]); im = np.array([r[3] for r in rows]); pr = np.array([r[4] for r in rows])
print('series:', len(rows))
print('seed-to-seed EDT sd %%: median %.2f  p90 %.2f  max %.2f' % tuple(100 * np.percentile(sd, q) for q in (50, 90, 100)))
print('ratio sd / leanband-implied sd: median %.2f  p10 %.2f  p90 %.2f' % tuple(np.percentile(sd / im, q) for q in (50, 10, 90)))
print('ratio sd / Poisson-predicted sd: median %.2f  p10 %.2f  p90 %.2f' % tuple(np.percentile(sd / pr, q) for q in (50, 10, 90)))
for cell in sorted(set(r[0] for r in rows)):
    sel = [r for r in rows if r[0] == cell]
    print('  %s: sd %.2f%%  implied %.2f%%  Poisson %.2f%%' % (cell, 100 * np.median([r[2] for r in sel]),
                                                         100 * np.median([r[3] for r in sel]), 100 * np.median([r[4] for r in sel])))
