"""W1G read at the 1 ms step itself on the twelve real 1 ms datasets (refusals only: no finer truth)."""
import os, sys, collections
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE); sys.path.insert(0, os.path.join(HERE, '..', 'skeptic-gf3'))
import harness as H, ism
import composite as K
C = H.C
cnt = collections.Counter(); per = collections.defaultdict(collections.Counter)
for name, desc, fine, runs in C.datasets():
    if fine != 1.0:
        continue
    for run in runs:
        R = run['h'] * run['c']
        for r in run['recs']:
            for f, v in r['bands'].items():
                m = ism.m_energy(float(f))
                x = K.evaluate(v, run['dt'], r['t'], run['h'], r['d1'], run['c'], r['clear'], R, K.air_factor(m, R, run['c'], run['dt']), n_extra=1, guards=H.ALL)
                for q in ('edt', 'ts'):
                    key = (q, 'accepted' if x[q]['ok'] else x[q]['why'])
                    cnt[key] += 1; per[name][key] += 1
out = ['W1G at the 1 ms step, twelve real 1 ms datasets: ' + ', '.join(f'{q} {w} {n}' for (q, w), n in sorted(cnt.items()))]
for name, c in per.items():
    out.append(f'  {name}: ' + ', '.join(f'{q} {w} {n}' for (q, w), n in sorted(c.items())))
print('\n'.join(out)); open(os.path.join(HERE, 'at_1ms.txt'), 'w').write('\n'.join(out) + '\n')
