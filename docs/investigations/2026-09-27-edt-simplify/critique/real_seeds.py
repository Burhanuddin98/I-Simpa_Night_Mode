"""Real-noise check (I5) on the committed SPPS noise-calibration runs: for each (cell, receiver, band),
run each candidate on every seed's own histogram (as the eval does: 1 ms fine -> 2 ms) and on the
10-seed MEAN histogram. The seed-mean is a lower-noise reference for the same physics (it shares
the I2/I4 treatment, so this isolates noise). Reports, per candidate: the seed-to-seed EDT spread vs
the half-width the candidate itself reports, and P(status ok & |EDT_seed / EDT_mean - 1| > 5%)."""
import sys, json, glob, math, collections
sys.dont_write_bytecode = True
import numpy as np
from common import CANDS
AG = 'B:/repos/I-Simpa_Night_Mode/target/agents'
ALLOWED = ['C-E3', 'C-E4', 'C-E6', 'V-E2', 'V-E5', 'C-R3', 'C-R4', 'C-R6', 'V-R2', 'V-R6']
ROOTS = (AG + '/pm8-noise-scratch/runs/noise-cal-1790307822', AG + '/pm8-noise-scratch/runs/noise-cal-1790310131')
K = 2
stats = {n: collections.Counter() for n in CANDS}
spread_ratio = {n: [] for n in CANDS}
worst = {n: [] for n in CANDS}
for root in ROOTS:
    cells = json.load(open(root + '/cells.json'))
    for c in cells:
        cc = c if 'id' in c else c['cell']
        if cc['id'] not in ALLOWED or abs(cc['time_step_s'] - 0.001) > 1e-9:
            continue
        seeds = sorted(glob.glob(f"{root}/{cc['id']}/seed*/report.json"))
        runs = [json.load(open(p))['spps'] for p in seeds]
        d0 = runs[0]
        for ri, pr0 in enumerate(d0['point_receivers']):
            for bi, b0 in enumerate(pr0['bands']):
                F = b0['freq_hz']
                series = []
                for d in runs:
                    pr = d['point_receivers'][ri]
                    v = np.asarray(pr['bands'][bi]['energy_pa2'], float)
                    series.append(v)
                L = min(len(v) for v in series)
                series = [v[:L] for v in series]
                n = L // K
                reb = [v[:n * K].reshape(n, K).sum(1) for v in series]
                mean = np.mean(reb, axis=0)
                t_arr = pr0['arrival_s']; dt = d0['time_step_s'] * K
                for nm, m in CANDS.items():
                    rm = m.analyse(mean, dt, t_arr, {})
                    if rm['edt'] is None:
                        continue
                    vals, hws, oks = [], [], []
                    for v in reb:
                        r = m.analyse(v, dt, t_arr, {})
                        if r['edt'] is None:
                            stats[nm]['refused'] += 1
                            continue
                        e = r['edt'] / rm['edt'] - 1
                        vals.append(r['edt'])
                        hws.append((r['edt_hi'] - r['edt_lo']) / 2 / r['edt'])
                        if r['status'] == 'ok':
                            stats[nm]['ok_WRONG' if abs(e) > 0.05 else 'ok_good'] += 1
                            if abs(e) > 0.05:
                                worst[nm].append((e, cc['id'], pr0.get('label'), F, r['edt'], rm['edt'], r['edt_lo'], r['edt_hi']))
                        else:
                            stats[nm]['wide'] += 1
                    if len(vals) >= 3:
                        sd = np.std(vals, ddof=1) / np.mean(vals)
                        spread_ratio[nm].append((sd, float(np.median(hws)), cc['id'], F))
for nm in CANDS:
    sr = spread_ratio[nm]
    ratio = np.array([s / max(h / 1.96, 1e-12) for s, h, _, _ in sr])  # seed sd / implied sd
    print(f'{nm}: seeds {dict(stats[nm])}; series {len(sr)}; seed-sd / (half-width/1.96): median {np.median(ratio):.2f}, p90 {np.percentile(ratio, 90):.2f}, max {ratio.max():.2f}')
    print('    seed-to-seed EDT sd (%%): median %.2f p90 %.2f max %.2f' % tuple(100 * np.percentile([s for s, *_ in sr], q) for q in (50, 90, 100)))
    print('    wrong-silent rows by cell:', dict(collections.Counter(w[1] for w in worst[nm])))
    for w in sorted(worst[nm], key=lambda x: -abs(x[0]))[:5]:
        print('    err vs seed-mean %+.1f%% cell=%s rec=%s F=%s edt=%.4f mean=%.4f range=[%.4f, %.4f]' % (100 * w[0], w[1], w[2], w[3], w[4], w[5], w[6], w[7]))
