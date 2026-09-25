"""composite.evaluate with no guards, the w1d split and the fixed 0.95 air factor must reproduce
bracket/rule.bracket(n_extra=1, dsym=True) on real data (POS200 and C-E6/C-R3 at 2-5 ms)."""
import sys, math, time
sys.path.insert(0, '.')
import composite as K
sys.path.insert(0, K.BR)
import common as C, rule
t0 = time.time()
ds = {n: runs for n, d, f, runs in C.datasets() if n in ('POS200', 'C-E6', 'C-R3', 'W-E6')}
n = worst_e = worst_s = 0; mism = 0
for name, runs in ds.items():
    for run in runs[:2]:
        for r in run['recs'][:60]:
            for f, v in r['bands'].items():
                for ms in (2, 3, 5):
                    k = int(round(ms / 1.0)); vv = C.rebin(v, k); dt = run['dt'] * k
                    b = rule.bracket(vv, dt, r['t'], run['h'], r['d1'], run['c'], r['sep'], n_extra=1, dsym=True, clear=r['clear'])
                    x = K.evaluate(vv, dt, r['t'], run['h'], r['d1'], run['c'], r['clear'], 0.31, 0.95, n_extra=1, guards=(), exact_split=False)
                    if b is None:
                        mism += x['edt'].get('why') != 'arrival_misfit'; continue
                    n += 1
                    if b['edt']['mid'] is not None and x['edt'].get('value') is not None:
                        worst_e = max(worst_e, abs(b['edt']['mid'] / x['edt']['value'] - 1))
                    if (b['edt']['ok']) != (x['edt']['ok']):
                        mism += 1
                    worst_s = max(worst_s, abs(b['ts']['mid'] - x['ts']['value']))
                    if b['ts']['ok'] != x['ts']['ok']:
                        mism += 1
print(f'{n} series-steps; verdict mismatches {mism}; worst EDT value diff {worst_e:.2e} rel; worst Ts diff {worst_s:.2e} s; {time.time()-t0:.0f} s')
